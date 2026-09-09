use crate::{
    metadata::infer_language,
    models::{PlaylistImportRow, Track},
    normalize::{detect_version, normalize_text},
};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RawTrack {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub source_platform: String,
    pub source_url: String,
}

#[derive(Debug, Default)]
pub struct PublicPlaylist {
    pub name: Option<String>,
    pub declared_count: Option<usize>,
    pub visible_count: usize,
    pub raw_tracks: Vec<RawTrack>,
    pub rows: Vec<PlaylistImportRow>,
    pub status: &'static str,
}

pub trait ChinaPlatformAdapter {
    fn platform(&self) -> &'static str;
    fn parse_public_metadata(
        &self,
        html: &[u8],
        playlist_id: &str,
        source_url: &str,
    ) -> PublicPlaylist;
}

pub struct PublicJsonLdAdapter {
    platform: &'static str,
}

impl PublicJsonLdAdapter {
    pub fn for_platform(platform: &'static str) -> Option<Self> {
        matches!(platform, "qq_music" | "kugou").then_some(Self { platform })
    }
}

impl ChinaPlatformAdapter for PublicJsonLdAdapter {
    fn platform(&self) -> &'static str {
        self.platform
    }

    fn parse_public_metadata(
        &self,
        html: &[u8],
        playlist_id: &str,
        source_url: &str,
    ) -> PublicPlaylist {
        let Ok(text) = std::str::from_utf8(html) else {
            return PublicPlaylist {
                status: "public_html_invalid_utf8",
                ..Default::default()
            };
        };
        let document = Html::parse_document(text);
        let selector =
            Selector::parse("script[type='application/ld+json']").expect("static selector");
        for script in document.select(&selector) {
            let payload = script.text().collect::<String>();
            let Ok(value) = serde_json::from_str::<Value>(&payload) else {
                continue;
            };
            if let Some(playlist) = find_playlist(&value) {
                return parse_playlist(self.platform(), playlist, playlist_id, source_url);
            }
        }
        PublicPlaylist {
            status: "ACCESSIBILITY_CHECK_ONLY",
            ..Default::default()
        }
    }
}

pub fn into_tracks(raw_tracks: &[RawTrack]) -> Vec<Track> {
    raw_tracks
        .iter()
        .map(|raw| {
            let artists = raw
                .artist
                .split([';', '、', '&'])
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>();
            let mut external_ids = HashMap::new();
            external_ids.insert("source_platform".into(), raw.source_platform.clone());
            external_ids.insert("source_url".into(), raw.source_url.clone());
            Track {
                id: Uuid::new_v4().to_string(),
                title: raw.title.clone(),
                normalized_title: normalize_text(&raw.title),
                artists,
                album: raw.album.clone(),
                genres: Vec::new(),
                release_year: None,
                language: infer_language(&format!("{} {}", raw.title, raw.artist)),
                duration_ms: raw.duration_ms,
                platform: raw.source_platform.clone(),
                platform_url: Some(raw.source_url.clone()),
                external_ids,
                version_type: detect_version(&raw.title),
                mood_tags: Vec::new(),
                energy_score: None,
                popularity: None,
                metadata_confidence: 0.62,
            }
        })
        .collect()
}

fn find_playlist(value: &Value) -> Option<&Value> {
    match value {
        Value::Object(map) => {
            if type_is(value, "MusicPlaylist") {
                return Some(value);
            }
            map.get("@graph")
                .and_then(find_playlist)
                .or_else(|| map.values().find_map(find_playlist))
        }
        Value::Array(values) => values.iter().find_map(find_playlist),
        _ => None,
    }
}

fn type_is(value: &Value, expected: &str) -> bool {
    value.get("@type").is_some_and(|kind| match kind {
        Value::String(kind) => kind.eq_ignore_ascii_case(expected),
        Value::Array(kinds) => kinds.iter().any(|kind| {
            kind.as_str()
                .is_some_and(|kind| kind.eq_ignore_ascii_case(expected))
        }),
        _ => false,
    })
}

fn parse_playlist(
    platform: &str,
    value: &Value,
    playlist_id: &str,
    source_url: &str,
) -> PublicPlaylist {
    let name = value
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let declared_count = value
        .get("numTracks")
        .or_else(|| value.get("numberOfItems"))
        .and_then(number_as_usize)
        .filter(|count| (1..=10_000).contains(count));
    let entries = value
        .get("track")
        .or_else(|| value.get("tracks"))
        .or_else(|| value.get("itemListElement"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let visible_count = entries.len();
    let mut raw_tracks = Vec::new();
    let mut rows = Vec::new();
    for entry in entries {
        let item = entry.get("item").unwrap_or(&entry);
        let title = item
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let artist = artist_name(
            item.get("byArtist")
                .or_else(|| item.get("author"))
                .or_else(|| item.get("artist")),
        );
        let track_url = item
            .get("url")
            .and_then(Value::as_str)
            .filter(|url| url.starts_with("https://"))
            .unwrap_or(source_url);
        let duration_ms = item
            .get("duration")
            .and_then(Value::as_str)
            .and_then(iso_duration_ms);
        let album = item
            .pointer("/inAlbum/name")
            .or_else(|| item.pointer("/album/name"))
            .or_else(|| item.get("album"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let imported = title
            .zip(artist.as_deref())
            .map(|(title, artist)| RawTrack {
                title: title.to_string(),
                artist: artist.to_string(),
                album: album.clone(),
                duration_ms,
                source_platform: platform.into(),
                source_url: track_url.into(),
            });
        rows.push(PlaylistImportRow {
            source_platform: platform.into(),
            playlist_id: playlist_id.into(),
            playlist_name: name.clone(),
            track_title: title.map(str::to_string),
            artist: artist.as_deref().map(|value| vec![value.to_string()]),
            duration_ms,
            source_url: Some(track_url.into()),
            availability: "PUBLIC_METADATA".into(),
            import_status: if imported.is_some() {
                "IMPORTED"
            } else {
                "SKIPPED_UNAVAILABLE_METADATA"
            }
            .into(),
        });
        if let Some(track) = imported {
            raw_tracks.push(track);
        }
    }
    let status = if raw_tracks.is_empty() {
        "ACCESSIBILITY_CHECK_ONLY"
    } else {
        "PUBLIC_HTML_TRACKS_IMPORTED"
    };
    PublicPlaylist {
        name,
        declared_count,
        visible_count,
        raw_tracks,
        rows,
        status,
    }
}

fn artist_name(value: Option<&Value>) -> Option<String> {
    let value = value?;
    match value {
        Value::String(value) => Some(value.trim().to_string()).filter(|value| !value.is_empty()),
        Value::Object(_) => value
            .get("name")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
        Value::Array(values) => {
            let names = values
                .iter()
                .filter_map(|value| artist_name(Some(value)))
                .collect::<Vec<_>>();
            (!names.is_empty()).then(|| names.join("; "))
        }
        _ => None,
    }
}

fn number_as_usize(value: &Value) -> Option<usize> {
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
}

fn iso_duration_ms(value: &str) -> Option<u32> {
    let raw = value.strip_prefix("PT")?;
    let (minutes, seconds) = if let Some((minutes, seconds)) = raw.split_once('M') {
        (
            minutes.parse::<u32>().ok()?,
            seconds.trim_end_matches('S').parse::<u32>().ok()?,
        )
    } else {
        (0, raw.trim_end_matches('S').parse::<u32>().ok()?)
    };
    minutes
        .checked_mul(60)?
        .checked_add(seconds)?
        .checked_mul(1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qq_and_kugou_json_ld_emit_only_real_complete_raw_tracks() {
        let html = r#"<script type="application/ld+json">{"@type":"MusicPlaylist","name":"Public mix","numTracks":3,"track":[{"@type":"MusicRecording","name":"晴天","byArtist":{"name":"周杰伦"},"inAlbum":{"name":"叶惠美"},"duration":"PT4M29S","url":"https://y.qq.com/song/1"},{"@type":"MusicRecording","name":"봄날","byArtist":{"name":"BTS"}},{"@type":"MusicRecording","name":"Missing artist"}]}</script>"#;
        for platform in ["qq_music", "kugou"] {
            let adapter = PublicJsonLdAdapter::for_platform(platform).unwrap();
            let parsed = adapter.parse_public_metadata(
                html.as_bytes(),
                "playlist-1",
                "https://example.invalid/source",
            );
            assert_eq!(parsed.declared_count, Some(3));
            assert_eq!(parsed.visible_count, 3);
            assert_eq!(parsed.raw_tracks.len(), 2);
            assert_eq!(parsed.raw_tracks[0].duration_ms, Some(269_000));
            let tracks = into_tracks(&parsed.raw_tracks);
            assert_eq!(tracks[0].language.as_deref(), Some("zh"));
            assert_eq!(tracks[1].language.as_deref(), Some("ko"));
            assert_eq!(parsed.rows[2].import_status, "SKIPPED_UNAVAILABLE_METADATA");
        }
    }

    #[test]
    fn page_without_public_playlist_metadata_never_fakes_tracks() {
        let parsed = PublicJsonLdAdapter::for_platform("qq_music")
            .unwrap()
            .parse_public_metadata(
                b"<html>shell</html>",
                "1",
                "https://y.qq.com/n/ryqq/playlist/1",
            );
        assert!(parsed.raw_tracks.is_empty());
        assert_eq!(parsed.status, "ACCESSIBILITY_CHECK_ONLY");
    }
}
