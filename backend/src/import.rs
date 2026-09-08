use crate::{
    genre::normalize_genres,
    models::{DataState, ImportPreview, ImportPreviewRequest, ImportedTrack, MetadataStatus},
};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

const TITLE_HEADERS: &[&str] = &[
    "title",
    "name",
    "track",
    "track name",
    "song",
    "song name",
    "歌曲",
    "歌曲名",
    "歌曲名称",
    "音乐标题",
];
const ARTIST_HEADERS: &[&str] = &[
    "artist",
    "artists",
    "artist name",
    "artist name(s)",
    "track artist",
    "track artist(s)",
    "歌手",
    "歌手名",
    "艺术家",
];
const ALBUM_HEADERS: &[&str] = &["album", "album name", "专辑", "专辑名"];
const DATE_HEADERS: &[&str] = &[
    "year",
    "release date",
    "album release date",
    "发行日期",
    "年份",
];
const GENRE_HEADERS: &[&str] = &["genre", "genres", "流派", "曲风"];
const DURATION_HEADERS: &[&str] = &["duration_ms", "duration ms", "duration (ms)", "时长毫秒"];
const TIME_HEADERS: &[&str] = &["time", "时间", "时长"];
const ENERGY_HEADERS: &[&str] = &["energy", "energy_score", "energy score", "能量"];

#[derive(Debug, Clone)]
pub struct StoredImport {
    pub id: String,
    pub name: String,
    pub file_name: Option<String>,
    pub data_state: DataState,
    pub source_label: String,
    pub total_rows: usize,
    pub invalid_count: usize,
    pub detected_fields: Vec<String>,
    pub requires_column_confirmation: bool,
    pub text_order: String,
    pub questions: Vec<String>,
    pub tracks: Vec<ImportedTrack>,
}

impl StoredImport {
    /// Server-created public metadata, never client-supplied platform API content.
    pub fn from_netease(result: &crate::models::PlaylistLinkInspection) -> Option<Self> {
        if result.platform.as_deref() != Some("netease")
            || !result.can_analyze
            || result.preview_tracks.is_empty()
        {
            return None;
        }
        let total = result.track_count?;
        let tracks: Vec<_> = result
            .preview_tracks
            .iter()
            .map(|track| ImportedTrack {
                title: track.title.clone(),
                artists: track.artists.clone(),
                album: track.album.clone(),
                release_date: None,
                genres: vec![],
                duration_ms: track.duration_ms,
                energy_score: None,
                source: "netease".into(),
                source_url: track.platform_url.clone(),
                original_row: format!("{} - {}", track.artists.join(" / "), track.title),
                metadata_status: MetadataStatus::Partial,
                metadata_confidence: track.metadata_confidence,
                warnings: vec![],
            })
            .collect();
        Some(Self {
            id: Uuid::new_v4().to_string(),
            name: result
                .playlist_name
                .clone()
                .unwrap_or_else(|| "网易云公开歌单".into()),
            file_name: None,
            data_state: DataState::RealPublicLink,
            source_label: format!(
                "网易云公开歌单 · Imported {} / {} tracks",
                tracks.len(),
                total
            ),
            total_rows: total,
            invalid_count: total.saturating_sub(tracks.len()),
            detected_fields: vec![
                "title".into(),
                "artist".into(),
                "album".into(),
                "duration_ms".into(),
                "source_platform".into(),
                "source_url".into(),
            ],
            requires_column_confirmation: false,
            text_order: "artist_title".into(),
            questions: vec![result.message.clone()],
            tracks,
        })
    }
    pub fn preview(&self) -> ImportPreview {
        ImportPreview {
            id: self.id.clone(),
            name: self.name.clone(),
            file_name: self.file_name.clone(),
            data_state: self.data_state.clone(),
            source_label: self.source_label.clone(),
            total_rows: self.total_rows,
            parsed_count: self.tracks.len(),
            warning_count: self.tracks.iter().map(|track| track.warnings.len()).sum(),
            invalid_count: self.invalid_count,
            detected_fields: self.detected_fields.clone(),
            preview_tracks: self.tracks.iter().take(20).cloned().collect(),
            requires_column_confirmation: self.requires_column_confirmation,
            text_order: self.text_order.clone(),
            questions: self.questions.clone(),
        }
    }
}

pub fn parse_import(mut request: ImportPreviewRequest) -> Result<StoredImport, String> {
    if !matches!(
        request.data_state,
        DataState::RealFile | DataState::RealText
    ) {
        return Err("真实导入只接受 REAL_FILE 或 REAL_TEXT 数据状态".into());
    }
    request.content = request.content.trim_start_matches('\u{feff}').to_string();
    if request.content.trim().is_empty() {
        return Err("输入内容为空".into());
    }
    let format = request.format.trim().trim_start_matches('.').to_lowercase();
    let source_label = match request.data_state {
        DataState::RealFile => format!(
            "真实文件 · {}",
            request.file_name.as_deref().unwrap_or("未命名文件")
        ),
        DataState::RealText => "真实批量文本".into(),
        _ => unreachable!(),
    };
    let source = request
        .file_name
        .clone()
        .unwrap_or_else(|| "批量文本".into());
    let text_order = request
        .text_order
        .as_deref()
        .unwrap_or("artist_title")
        .to_string();
    let mut parsed = match format.as_str() {
        "csv" => parse_delimited(&request.content, b',', &source)?,
        "tsv" => parse_delimited(&request.content, b'\t', &source)?,
        "json" => parse_json(&request.content, &source)?,
        "m3u" | "m3u8" => parse_m3u(&request.content, &source, &text_order)?,
        "txt" | "text" if has_tabular_headers(&request.content) => {
            parse_delimited(&request.content, b'\t', &source)?
        }
        "txt" | "text" => parse_text(&request.content, &source, &text_order)?,
        _ => return Err(format!("不支持的导入格式：{format}")),
    };
    if request.text_order.is_some() {
        for track in &mut parsed.tracks {
            track.warnings.retain(|warning| !warning.contains("顺序"));
        }
    }
    if parsed.tracks.is_empty() {
        return Err(format!(
            "未解析到有效歌曲；共有 {} 行无法识别",
            parsed.invalid_count
        ));
    }
    let requires_column_confirmation = matches!(format.as_str(), "txt" | "text")
        && parsed.saw_ambiguous_order
        && request.text_order.is_none();
    let questions = if requires_column_confirmation {
        vec!["文本中的分隔符无法可靠判断歌手和歌名顺序，请确认列含义后再分析。".into()]
    } else {
        Vec::new()
    };
    Ok(StoredImport {
        id: Uuid::new_v4().to_string(),
        name: request.name.unwrap_or_else(|| {
            request
                .file_name
                .as_deref()
                .unwrap_or("真实导入歌单")
                .split('.')
                .next()
                .unwrap_or("真实导入歌单")
                .to_string()
        }),
        file_name: request.file_name,
        data_state: request.data_state,
        source_label,
        total_rows: parsed.total_rows,
        invalid_count: parsed.invalid_count,
        detected_fields: parsed.detected_fields,
        requires_column_confirmation,
        text_order,
        questions,
        tracks: parsed.tracks,
    })
}

struct ParseResult {
    tracks: Vec<ImportedTrack>,
    total_rows: usize,
    invalid_count: usize,
    detected_fields: Vec<String>,
    saw_ambiguous_order: bool,
}

fn has_tabular_headers(content: &str) -> bool {
    let headers: Vec<_> = content
        .lines()
        .next()
        .unwrap_or_default()
        .split('\t')
        .map(normalize_header)
        .collect();
    find_header(&headers, TITLE_HEADERS).is_some()
        && find_header(&headers, ARTIST_HEADERS).is_some()
}

fn clock_duration(raw: &str) -> Option<u32> {
    let parts: Vec<_> = raw
        .trim()
        .split(':')
        .map(str::parse::<u32>)
        .collect::<Result<_, _>>()
        .ok()?;
    let seconds = match parts.as_slice() {
        [minutes, seconds] if *seconds < 60 => minutes.checked_mul(60)?.checked_add(*seconds)?,
        [hours, minutes, seconds] if *minutes < 60 && *seconds < 60 => hours
            .checked_mul(3600)?
            .checked_add(minutes.checked_mul(60)?)?
            .checked_add(*seconds)?,
        _ => return None,
    };
    seconds.checked_mul(1000).filter(|value| *value > 0)
}

fn parse_delimited(content: &str, delimiter: u8, source: &str) -> Result<ParseResult, String> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .trim(csv::Trim::All)
        .from_reader(content.trim_start_matches('\u{feff}').as_bytes());
    let headers = reader
        .headers()
        .map_err(|error| format!("表头解析失败：{error}"))?
        .clone();
    let normalized: Vec<String> = headers.iter().map(normalize_header).collect();
    let title = find_header(&normalized, TITLE_HEADERS)
        .ok_or_else(|| "未找到歌曲列；请使用 title/name/Track Name/歌曲名等表头".to_string())?;
    let artist = find_header(&normalized, ARTIST_HEADERS)
        .ok_or_else(|| "未找到歌手列；请使用 artist/Artist Name(s)/歌手等表头".to_string())?;
    let album = find_header(&normalized, ALBUM_HEADERS);
    let date = find_header(&normalized, DATE_HEADERS);
    let genres = find_header(&normalized, GENRE_HEADERS);
    let duration = find_header(&normalized, DURATION_HEADERS);
    let clock_time = find_header(&normalized, TIME_HEADERS);
    let energy = find_header(&normalized, ENERGY_HEADERS);
    let mut tracks = Vec::new();
    let mut invalid = 0;
    let mut total = 0;
    for record in reader.records() {
        total += 1;
        match record {
            Ok(record) => {
                let title_value = field(&record, title);
                let artists_value = field(&record, artist);
                if title_value.is_empty() || artists_value.is_empty() {
                    invalid += 1;
                    continue;
                }
                let album_value = album
                    .map(|index| field(&record, index))
                    .filter(|v| !v.is_empty());
                let date_value = date
                    .map(|index| field(&record, index))
                    .filter(|v| !v.is_empty());
                let genre_values = genres
                    .map(|index| split_values(&field(&record, index)))
                    .unwrap_or_default();
                let duration_value = duration
                    .and_then(|index| field(&record, index).parse::<u32>().ok())
                    .filter(|value| *value > 0)
                    .or_else(|| {
                        clock_time.and_then(|index| clock_duration(&field(&record, index)))
                    });
                let energy_value = energy.and_then(|index| parse_energy(&field(&record, index)));
                tracks.push(imported_track(
                    title_value,
                    split_artists(&artists_value),
                    album_value,
                    date_value,
                    genre_values,
                    duration_value,
                    energy_value,
                    source,
                    record.iter().collect::<Vec<_>>().join("\t"),
                    Vec::new(),
                ));
            }
            Err(error) => {
                invalid += 1;
                if total == 1 {
                    return Err(format!("表格解析失败：{error}"));
                }
            }
        }
    }
    Ok(ParseResult {
        tracks,
        total_rows: total,
        invalid_count: invalid,
        detected_fields: headers.iter().map(str::to_string).collect(),
        saw_ambiguous_order: false,
    })
}

fn parse_json(content: &str, source: &str) -> Result<ParseResult, String> {
    let value: Value =
        serde_json::from_str(content).map_err(|error| format!("JSON 解析失败：{error}"))?;
    let rows = match value {
        Value::Array(rows) => rows,
        Value::Object(mut object) => object
            .remove("tracks")
            .or_else(|| object.remove("songs"))
            .and_then(|value| value.as_array().cloned())
            .ok_or_else(|| "JSON 需要数组，或包含 tracks/songs 数组".to_string())?,
        _ => return Err("JSON 顶层必须是歌曲数组或包含 tracks/songs 数组".into()),
    };
    let total = rows.len();
    let mut tracks = Vec::new();
    let mut invalid = 0;
    let mut fields = Vec::new();
    for row in rows {
        let Some(object) = row.as_object() else {
            invalid += 1;
            continue;
        };
        if fields.is_empty() {
            fields = object.keys().cloned().collect();
        }
        let normalized: HashMap<String, &Value> = object
            .iter()
            .map(|(key, value)| (normalize_header(key), value))
            .collect();
        let title = value_for(&normalized, TITLE_HEADERS)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim();
        let artists_value = value_for(&normalized, ARTIST_HEADERS);
        let artists = match artists_value {
            Some(Value::Array(values)) => values
                .iter()
                .filter_map(Value::as_str)
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
                .collect(),
            Some(Value::String(value)) => split_artists(value),
            _ => Vec::new(),
        };
        if title.is_empty() || artists.is_empty() {
            invalid += 1;
            continue;
        }
        let album = value_for(&normalized, ALBUM_HEADERS)
            .and_then(Value::as_str)
            .map(str::to_string);
        let release_date = value_for(&normalized, DATE_HEADERS).and_then(|value| {
            value
                .as_str()
                .map(str::to_string)
                .or_else(|| value.as_u64().map(|v| v.to_string()))
        });
        let genres = match value_for(&normalized, GENRE_HEADERS) {
            Some(Value::Array(values)) => values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect(),
            Some(Value::String(value)) => split_values(value),
            _ => Vec::new(),
        };
        let duration_ms = value_for(&normalized, DURATION_HEADERS)
            .and_then(|value| {
                value
                    .as_u64()
                    .or_else(|| value.as_str().and_then(|raw| raw.parse().ok()))
            })
            .and_then(|value| u32::try_from(value).ok());
        let energy_score = value_for(&normalized, ENERGY_HEADERS).and_then(|value| {
            value
                .as_f64()
                .map(|score| score as f32)
                .or_else(|| value.as_str().and_then(parse_energy))
                .and_then(normalize_energy)
        });
        tracks.push(imported_track(
            title.to_string(),
            artists,
            album,
            release_date,
            genres,
            duration_ms,
            energy_score,
            source,
            row.to_string(),
            Vec::new(),
        ));
    }
    Ok(ParseResult {
        tracks,
        total_rows: total,
        invalid_count: invalid,
        detected_fields: fields,
        saw_ambiguous_order: false,
    })
}

fn parse_m3u(content: &str, source: &str, order: &str) -> Result<ParseResult, String> {
    let mut metadata = None;
    let mut tracks = Vec::new();
    let mut invalid = 0;
    let mut total = 0;
    for raw in content.lines() {
        let line = raw.trim().trim_start_matches('\u{feff}');
        if line.starts_with("#EXTINF:") {
            metadata = line
                .split_once(',')
                .map(|(_, value)| value.trim().to_string());
        } else if !line.is_empty() && !line.starts_with('#') {
            total += 1;
            let label = metadata.take().unwrap_or_else(|| line.to_string());
            if let Some((title, artists, ambiguous)) = parse_text_fields(&label, order) {
                tracks.push(imported_track(
                    title,
                    artists,
                    None,
                    None,
                    Vec::new(),
                    None,
                    None,
                    source,
                    label,
                    if ambiguous {
                        vec!["歌手与歌名顺序由用户确认".into()]
                    } else {
                        Vec::new()
                    },
                ));
            } else {
                invalid += 1;
            }
        }
    }
    Ok(ParseResult {
        tracks,
        total_rows: total,
        invalid_count: invalid,
        detected_fields: vec!["EXTINF".into(), "path".into()],
        saw_ambiguous_order: false,
    })
}

fn parse_text(content: &str, source: &str, order: &str) -> Result<ParseResult, String> {
    let mut tracks = Vec::new();
    let mut invalid = 0;
    let mut total = 0;
    let mut ambiguous = false;
    for raw in content.lines() {
        let line = clean_numbering(raw);
        if line.is_empty() {
            continue;
        }
        total += 1;
        if let Some((title, artists, is_ambiguous)) = parse_text_fields(line, order) {
            ambiguous |= is_ambiguous;
            tracks.push(imported_track(
                title,
                artists,
                None,
                None,
                Vec::new(),
                None,
                None,
                source,
                raw.to_string(),
                if is_ambiguous {
                    vec!["歌手与歌名顺序需要确认".into()]
                } else {
                    Vec::new()
                },
            ));
        } else {
            invalid += 1;
        }
    }
    Ok(ParseResult {
        tracks,
        total_rows: total,
        invalid_count: invalid,
        detected_fields: vec!["title".into(), "artists".into()],
        saw_ambiguous_order: ambiguous,
    })
}

fn parse_text_fields(line: &str, order: &str) -> Option<(String, Vec<String>, bool)> {
    if let Some((title, artist)) = line.split_once('\t') {
        return non_empty_pair(title, artist)
            .map(|(title, artist)| (title.into(), split_artists(artist), false));
    }
    let pair = line
        .split_once(" — ")
        .or_else(|| line.split_once(" – "))
        .or_else(|| line.split_once(" - "))
        .or_else(|| line.split_once(" | "))?;
    let (left, right) = non_empty_pair(pair.0, pair.1)?;
    let (artist, title) = if order == "title_artist" {
        (right, left)
    } else {
        (left, right)
    };
    Some((title.into(), split_artists(artist), true))
}

fn clean_numbering(raw: &str) -> &str {
    let trimmed = raw.trim().trim_start_matches('\u{feff}');
    let bytes = trimmed.as_bytes();
    let mut index = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
    }
    if index > 0 && index < bytes.len() && matches!(bytes[index], b'.' | b')') {
        trimmed[index + 1..].trim_start()
    } else {
        trimmed.trim_start_matches(['•', '-']).trim_start()
    }
}

fn imported_track(
    title: String,
    artists: Vec<String>,
    album: Option<String>,
    release_date: Option<String>,
    genres: Vec<String>,
    duration_ms: Option<u32>,
    energy_score: Option<f32>,
    source: &str,
    original_row: String,
    warnings: Vec<String>,
) -> ImportedTrack {
    let populated = usize::from(album.is_some())
        + usize::from(release_date.is_some())
        + usize::from(!genres.is_empty())
        + usize::from(duration_ms.is_some());
    let metadata_status = if populated == 4 {
        MetadataStatus::Complete
    } else if populated > 0 {
        MetadataStatus::Partial
    } else {
        MetadataStatus::Missing
    };
    ImportedTrack {
        source_url: None,
        title,
        artists,
        album,
        release_date,
        genres: normalize_genres(genres),
        duration_ms,
        energy_score,
        source: source.into(),
        original_row,
        metadata_status,
        metadata_confidence: if populated == 4 {
            1.0
        } else if populated > 0 {
            0.65
        } else {
            0.25
        },
        warnings,
    }
}

fn normalize_header(value: &str) -> String {
    value.trim().trim_start_matches('\u{feff}').to_lowercase()
}
fn find_header(headers: &[String], aliases: &[&str]) -> Option<usize> {
    headers
        .iter()
        .position(|header| aliases.contains(&header.as_str()))
}
fn field(record: &csv::StringRecord, index: usize) -> String {
    record.get(index).unwrap_or("").trim().to_string()
}
fn value_for<'a>(values: &'a HashMap<String, &Value>, aliases: &[&str]) -> Option<&'a Value> {
    aliases.iter().find_map(|alias| values.get(*alias).copied())
}
fn split_values(value: &str) -> Vec<String> {
    value
        .split([';', ',', '|', '、'])
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_energy(value: &str) -> Option<f32> {
    value.trim().parse::<f32>().ok().and_then(normalize_energy)
}

fn normalize_energy(value: f32) -> Option<f32> {
    if !value.is_finite() || value < 0.0 {
        None
    } else if value <= 1.0 {
        Some(value)
    } else if value <= 100.0 {
        Some(value / 100.0)
    } else {
        None
    }
}
fn split_artists(value: &str) -> Vec<String> {
    value
        .split([';', '、', '&'])
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_string)
        .collect()
}
fn non_empty_pair<'a>(left: &'a str, right: &'a str) -> Option<(&'a str, &'a str)> {
    let left = left.trim();
    let right = right.trim();
    (!left.is_empty() && !right.is_empty()).then_some((left, right))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_tabular_text_reuses_preview_with_real_missing_values() {
        let mut req = request(
            "txt",
            "\u{feff}Name\tArtist\tAlbum\tTime\n中文 日本語 한국어\t艺人\tAlbum\t3:05\n重复\t艺人\t\t\n重复\t艺人\t\t\nMissing artist\t\t\t\n",
        );
        req.text_order = None;
        let parsed = parse_import(req).unwrap();
        assert_eq!(parsed.tracks.len(), 3);
        assert_eq!(parsed.invalid_count, 1);
        assert!(!parsed.requires_column_confirmation);
        assert_eq!(parsed.tracks[0].duration_ms, Some(185000));
        assert_eq!(parsed.tracks[1].duration_ms, None);
        // File duplicates are retained for existing analysis duplicate statistics.
        assert_eq!(parsed.tracks[1].title, parsed.tracks[2].title);
        assert_eq!(parsed.preview().parsed_count, 3);
        assert_eq!(clock_duration("bad"), None);
        assert_eq!(clock_duration("3:99"), None);
        assert_eq!(clock_duration("0:00"), None);
        assert_eq!(clock_duration("1:02:03"), Some(3723000));
    }

    fn request(format: &str, content: &str) -> ImportPreviewRequest {
        ImportPreviewRequest {
            name: Some("test".into()),
            file_name: Some(format!("test.{format}")),
            format: format.into(),
            content: content.into(),
            data_state: DataState::RealFile,
            text_order: Some("artist_title".into()),
        }
    }

    #[test]
    fn parses_exportify_csv_with_quotes_commas_and_multiple_artists() {
        let csv = "Track Name,Artist Name(s),Album Name,Album Release Date,Track URI\n\"Song, Part II\",\"Artist A; Artist B\",\"The \"\"Quoted\"\" Album\",2024-01-02,spotify:track:1";
        let parsed = parse_import(request("csv", csv)).unwrap();
        assert_eq!(parsed.tracks[0].title, "Song, Part II");
        assert_eq!(parsed.tracks[0].artists, ["Artist A", "Artist B"]);
        assert_eq!(
            parsed.tracks[0].album.as_deref(),
            Some("The \"Quoted\" Album")
        );
    }

    #[test]
    fn parses_exportify_genres_duration_and_real_energy() {
        let csv = "Track Name,Artist Name(s),Album Name,Album Release Date,Duration (ms),Genres,Energy\nSong,Artist,Album,2024-01-02,203456,\"pop,bedroom pop\",0.73";
        let parsed = parse_import(request("csv", csv)).unwrap();
        let track = &parsed.tracks[0];
        assert_eq!(track.genres, ["Pop", "Bedroom Pop"]);
        assert_eq!(track.duration_ms, Some(203_456));
        assert_eq!(track.energy_score, Some(0.73));
        assert_eq!(track.metadata_status, MetadataStatus::Complete);
    }

    #[test]
    fn json_genres_use_the_same_canonicalization() {
        let parsed = parse_import(request(
            "json",
            r#"[{"title":"One","artists":["A"],"genres":["POP","indie-pop"],"energy":82}]"#,
        ))
        .unwrap();
        assert_eq!(parsed.tracks[0].genres, ["Pop", "Indie Pop"]);
        assert_eq!(parsed.tracks[0].energy_score, Some(0.82));
    }

    #[test]
    fn parses_chinese_headers_and_utf8_bom() {
        let parsed = parse_import(request(
            "csv",
            "\u{feff}歌曲名,歌手,专辑名,年份\n晴天,周杰伦,叶惠美,2003",
        ))
        .unwrap();
        assert_eq!(parsed.tracks[0].title, "晴天");
        assert_eq!(parsed.tracks[0].artists, ["周杰伦"]);
    }

    #[test]
    fn parses_tsv_with_compatible_headers() {
        let parsed = parse_import(request(
            "tsv",
            "song name\ttrack artist(s)\talbum name\nNight Drive\tArtist A; Artist B\tRoad Album",
        ))
        .unwrap();
        assert_eq!(parsed.tracks[0].title, "Night Drive");
        assert_eq!(parsed.tracks[0].artists, ["Artist A", "Artist B"]);
    }

    #[test]
    fn parses_five_hundred_tracks() {
        let mut csv = String::from("title,artist\n");
        for index in 0..500 {
            csv.push_str(&format!("Song {index},Artist {index}\n"));
        }
        let parsed = parse_import(request("csv", &csv)).unwrap();
        assert_eq!(parsed.tracks.len(), 500);
    }

    #[test]
    fn parses_json_txt_and_m3u8() {
        let json =
            parse_import(request("json", r#"[{"title":"One","artists":["A","B"]}]"#)).unwrap();
        assert_eq!(json.tracks[0].artists.len(), 2);
        let mut text_request = request("txt", "1. Song One - Artist One\nSong Two\tArtist Two");
        text_request.text_order = Some("title_artist".into());
        let text = parse_import(text_request).unwrap();
        assert_eq!(text.tracks[0].title, "Song One");
        let m3u = parse_import(request(
            "m3u8",
            "#EXTM3U\n#EXTINF:123,Artist - Song\n/music/song.mp3",
        ))
        .unwrap();
        assert_eq!(m3u.tracks[0].title, "Song");
    }

    #[test]
    fn rejects_invalid_files() {
        assert!(parse_import(request("csv", "foo,bar\na,b")).is_err());
        assert!(parse_import(request("txt", "unstructured title only")).is_err());
    }
}
