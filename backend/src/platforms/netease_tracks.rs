//! Public HTML/JSON-LD only. No hydration payloads, scripts or internal APIs.
use crate::{
    models::{PlaylistImportRow, Track},
    normalize::{detect_version, normalize_text},
};
use reqwest::{Client, Url, redirect::Policy};
use scraper::{Html, Selector};
use serde_json::Value;
use std::{collections::HashSet, future::Future, sync::LazyLock, time::Duration};
use tokio::time::{Instant, timeout_at};

const MAX_DETAILS: usize = 20;
const BUDGET: Duration = Duration::from_secs(20);
static DETAIL_CLIENT: LazyLock<Client> = LazyLock::new(|| {
    Client::builder()
        .redirect(Policy::none())
        .timeout(Duration::from_secs(3))
        .user_agent("MelodyPath/0.2 (public music metadata; no cookies)")
        .build()
        .expect("anonymous detail client")
});

#[derive(Clone, Debug)]
struct Song {
    id: String,
    title: String,
    track: Option<Track>,
}

pub(super) struct ImportedPage {
    pub tracks: Vec<Track>,
    pub rows: Vec<PlaylistImportRow>,
}

fn selector(value: &str) -> Selector {
    Selector::parse(value).expect("static selector")
}
fn typed(value: &Value, kind: &str) -> bool {
    value["@type"].as_str() == Some(kind)
        || value["@type"]
            .as_array()
            .is_some_and(|types| types.iter().any(|t| t.as_str() == Some(kind)))
}
fn documents(html: &str) -> Vec<Value> {
    Html::parse_document(html)
        .select(&selector("script[type='application/ld+json']"))
        .filter_map(|script| serde_json::from_str::<Value>(&script.inner_html()).ok())
        .flat_map(|v| {
            v.as_array()
                .cloned()
                .or_else(|| v["@graph"].as_array().cloned())
                .unwrap_or_else(|| vec![v])
        })
        .collect()
}
fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.chars().count() <= 500)
        .map(str::to_string)
}
fn official_id(raw: &str, path: &str) -> Option<String> {
    let base = Url::parse("https://music.163.com/").ok()?;
    let url = base.join(raw).ok()?;
    if url.scheme() != "https"
        || url.host_str() != Some("music.163.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || url.path() != path
        || url.fragment().is_some()
    {
        return None;
    }
    let query: Vec<_> = url.query_pairs().collect();
    if query.len() != 1 || query[0].0 != "id" || !super::numeric_id(&query[0].1) {
        return None;
    }
    Some(query[0].1.to_string())
}
fn identity(value: &Value, path: &str) -> Option<String> {
    value["mainEntityOfPage"]["@id"]
        .as_str()
        .or_else(|| value["url"].as_str())
        .and_then(|url| official_id(url, path))
}
fn duration(value: &Value) -> Option<u32> {
    static ISO: LazyLock<regex::Regex> = LazyLock::new(|| {
        regex::Regex::new(r"^PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+(?:\.\d{1,3})?)S)?$").unwrap()
    });
    let captures = ISO.captures(value.as_str()?)?;
    let hours: f64 = captures.get(1).map_or("0", |m| m.as_str()).parse().ok()?;
    let minutes: f64 = captures.get(2).map_or("0", |m| m.as_str()).parse().ok()?;
    let seconds: f64 = captures.get(3).map_or("0", |m| m.as_str()).parse().ok()?;
    let ms = (hours * 3600.0 + minutes * 60.0 + seconds) * 1000.0;
    (ms > 0.0 && ms <= u32::MAX as f64).then_some(ms.round() as u32)
}
fn recording(value: &Value, id: &str) -> Option<Track> {
    if !typed(value, "MusicRecording") {
        return None;
    }
    let title = text(&value["name"])?;
    let artist_values = value["byArtist"]
        .as_array()
        .cloned()
        .unwrap_or_else(|| vec![value["byArtist"].clone()]);
    let artists: Option<Vec<_>> = artist_values.iter().map(|v| text(&v["name"])).collect();
    let artists = artists.filter(|a| !a.is_empty())?;
    let album = value["inAlbum"]
        .as_array()
        .and_then(|v| v.first())
        .unwrap_or(&value["inAlbum"]);
    Some(Track {
        id: format!("netease:{id}"),
        normalized_title: normalize_text(&title),
        version_type: detect_version(&title),
        title,
        artists,
        album: text(&album["name"]),
        duration_ms: duration(&value["duration"]),
        platform: "netease".into(),
        platform_url: Some(format!("https://music.163.com/song?id={id}")),
        external_ids: [("netease".into(), id.into())].into(),
        genres: vec![],
        release_year: None,
        language: None,
        mood_tags: vec![],
        energy_score: None,
        popularity: None,
        metadata_confidence: 0.5,
    })
}

fn songs(body: &[u8], playlist_id: &str) -> Result<Vec<Song>, &'static str> {
    let html = std::str::from_utf8(body).map_err(|_| "invalid_public_page_encoding")?;
    let json = documents(html);
    let playlist = json
        .iter()
        .find(|v| {
            typed(v, "MusicPlaylist") && identity(v, "/playlist").as_deref() == Some(playlist_id)
        })
        .ok_or("public_playlist_identity_unverified")?;
    let total = playlist["numTracks"]
        .as_u64()
        .filter(|n| *n > 0 && *n <= 10_000)
        .ok_or("public_track_count_unverified")? as usize;
    let list = playlist["track"]
        .as_array()
        .or_else(|| playlist["track"]["itemListElement"].as_array());
    let schema: Vec<Song> = list
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let entry = if typed(entry, "ListItem") {
                &entry["item"]
            } else {
                entry
            };
            let id = official_id(entry["url"].as_str()?, "/song")?;
            Some(Song {
                title: text(&entry["name"])?,
                track: recording(entry, &id),
                id,
            })
        })
        .collect();
    if schema.len() == total && unique(&schema) {
        return Ok(schema.into_iter().take(MAX_DETAILS).collect());
    }
    let dom = Html::parse_document(html);
    let containers: Vec<_> = dom.select(&selector("#song-list-pre-cache")).collect();
    if containers.len() != 1
        || containers[0].value().attr("data-key")
            != Some(format!("track_playlist-{playlist_id}").as_str())
    {
        return Err("public_track_list_incomplete");
    }
    let mut result = Vec::new();
    for anchor in containers[0].select(&selector("ul.f-hide > li > a")) {
        let id = anchor
            .value()
            .attr("href")
            .and_then(|href| official_id(href, "/song"))
            .ok_or("public_track_list_invalid")?;
        let title = anchor.text().collect::<String>().trim().to_string();
        if title.is_empty() {
            return Err("public_track_list_invalid");
        }
        let track = schema
            .iter()
            .find(|song| song.id == id)
            .and_then(|s| s.track.clone());
        result.push(Song { id, title, track });
    }
    // The public page may expose only a prefix of a large playlist. Import that
    // verifiable prefix only: both public structures must identify the same
    // ordered songs, and the UI keeps the declared total visible.
    if schema.is_empty()
        || result.is_empty()
        || result.len() > total
        || !unique(&result)
        || !schema.iter().zip(&result).all(|(a, b)| a.id == b.id)
    {
        return Err("public_track_list_incomplete");
    }
    Ok(result.into_iter().take(MAX_DETAILS).collect())
}
fn unique(songs: &[Song]) -> bool {
    songs.iter().map(|s| &s.id).collect::<HashSet<_>>().len() == songs.len()
}
async fn detail(id: String) -> Option<Track> {
    let mut response = DETAIL_CLIENT
        .get(format!("https://music.163.com/song?id={id}"))
        .send()
        .await
        .ok()?;
    if response.status() != reqwest::StatusCode::OK
        || !response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| {
                v.split(';')
                    .next()
                    .is_some_and(|v| v.trim().eq_ignore_ascii_case("text/html"))
            })
    {
        return None;
    }
    let body = super::read_public_page(&mut response, 1024 * 1024).await?;
    let html = std::str::from_utf8(&body).ok()?;
    documents(html)
        .iter()
        .find(|v| typed(v, "MusicRecording") && identity(v, "/song").as_deref() == Some(&id))
        .and_then(|v| recording(v, &id))
}
async fn hydrate<F, Fut>(
    songs: Vec<Song>,
    playlist_id: &str,
    name: Option<&str>,
    fetch: F,
    max_details: usize,
    budget: Duration,
) -> ImportedPage
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Option<Track>>,
{
    let deadline = Instant::now() + budget;
    let mut requests = 0;
    let mut output = ImportedPage {
        tracks: vec![],
        rows: vec![],
    };
    for song in songs {
        let mut track = song.track;
        let mut status = "IMPORTED";
        if track.is_none() {
            if requests >= max_details {
                status = "SKIPPED_REQUEST_LIMIT";
            } else if Instant::now() >= deadline {
                status = "SKIPPED_TIME_LIMIT";
            } else {
                requests += 1;
                track = timeout_at(deadline, fetch(song.id.clone()))
                    .await
                    .ok()
                    .flatten();
                if track.is_none() {
                    status = "SKIPPED_DETAIL_UNAVAILABLE";
                }
                if Instant::now() < deadline {
                    let _ =
                        timeout_at(deadline, tokio::time::sleep(Duration::from_millis(150))).await;
                }
            }
        }
        output.rows.push(PlaylistImportRow {
            source_platform: "netease".into(),
            playlist_id: playlist_id.into(),
            playlist_name: name.map(str::to_string),
            track_title: Some(track.as_ref().map_or(song.title, |t| t.title.clone())),
            artist: track.as_ref().map(|t| t.artists.clone()),
            duration_ms: track.as_ref().and_then(|t| t.duration_ms),
            source_url: Some(format!("https://music.163.com/song?id={}", song.id)),
            availability: "UNKNOWN".into(),
            import_status: status.into(),
        });
        if let Some(track) = track {
            output.tracks.push(track);
        }
    }
    output
}
pub(super) async fn import(
    body: &[u8],
    id: &str,
    name: Option<&str>,
) -> Result<ImportedPage, &'static str> {
    let songs = songs(body, id)?;
    Ok(hydrate(songs, id, name, detail, MAX_DETAILS, BUDGET).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn page(total: usize, ids: &[usize]) -> String {
        let playlist = json!({"@type":"MusicPlaylist", "name":"Synthetic playlist", "numTracks":total,
            "mainEntityOfPage":{"@id":"https://music.163.com/playlist?id=123"},
            "track":{"itemListElement":[{"@type":"ListItem", "item":{"@type":"MusicRecording", "name":"A & B", "url":"https://music.163.com/song?id=1"}}]}});
        format!(
            "<script type='application/ld+json'>{playlist}</script><div id='song-list-pre-cache' data-key='track_playlist-123'><ul class='f-hide'>{}</ul></div>",
            ids.iter()
                .map(|id| format!("<li><a href='/song?id={id}'>A &amp; B</a></li>"))
                .collect::<String>()
        )
    }
    fn track(id: &str) -> Track {
        recording(&json!({"@type":"MusicRecording", "name":"A & B", "byArtist":[{"name":"艺人"}], "inAlbum":{"name":"专辑"}, "duration":"PT3M30.123S"}), id).unwrap()
    }
    #[test]
    fn complete_dom_membership_excludes_unrelated_links_and_decodes_entities() {
        let html = page(2, &[1, 2]) + "<a href='/song?id=999'>Recommendation</a>";
        let result = songs(html.as_bytes(), "123").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].title, "A & B");
        assert_eq!(result[1].id, "2");
    }
    #[test]
    fn partial_public_prefix_is_importable_without_fabricating_hidden_members() {
        let result = songs(page(1196, &[1, 2]).as_bytes(), "123").unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].id, "1");
        assert_eq!(result[1].id, "2");
    }

    #[test]
    fn duplicate_conflicting_identity_or_unsafe_urls_never_import() {
        for html in [
            page(2, &[1, 1]),
            page(2, &[2, 1]),
            page(2, &[1, 2]).replace("track_playlist-123", "track_playlist-456"),
            page(2, &[1, 2]).replace("/song?id=2", "https://evil.example/song?id=2"),
            page(2, &[1, 2]).replace("playlist?id=123", "playlist?id=456"),
        ] {
            assert!(songs(html.as_bytes(), "123").is_err());
        }
        for url in [
            "http://music.163.com/song?id=1",
            "https://user@music.163.com/song?id=1",
            "/song?id=1&id=2",
            "/api/song?id=1",
        ] {
            assert!(official_id(url, "/song").is_none());
        }
    }

    #[test]
    fn public_candidates_are_capped_at_twenty_before_detail_requests() {
        let ids: Vec<_> = (1..=30).collect();
        let result = songs(page(30, &ids).as_bytes(), "123").unwrap();
        assert_eq!(result.len(), 20);
        assert_eq!(result.first().unwrap().id, "1");
        assert_eq!(result.last().unwrap().id, "20");
    }
    #[test]
    fn inline_complete_json_ld_needs_no_detail_fetch_and_missing_optional_fields_stay_null() {
        let v = json!({"@type":"MusicPlaylist", "numTracks":1, "mainEntityOfPage":{"@id":"https://music.163.com/playlist?id=123"},
            "track":[{"@type":"MusicRecording", "url":"https://music.163.com/song?id=1", "name":"Synthetic", "byArtist":{"name":"Artist"}}]});
        let html = format!("<script type='application/ld+json'>{v}</script>");
        let result = songs(html.as_bytes(), "123").unwrap();
        let track = result[0].track.as_ref().unwrap();
        assert!(track.album.is_none() && track.duration_ms.is_none());
        assert_eq!(track.platform, "netease");
        assert_eq!(super::tests::track("1").duration_ms, Some(210123));
        for value in ["PT", "210", "PT0S", "PT999999999H"] {
            assert!(duration(&json!(value)).is_none());
        }
    }
    #[tokio::test]
    async fn failed_details_do_not_discard_success_and_budget_is_reported() {
        let list = songs(page(3, &[1, 2, 3]).as_bytes(), "123").unwrap();
        let output = hydrate(
            list,
            "123",
            None,
            |id| async move { (id == "2").then(|| track(&id)) },
            2,
            BUDGET,
        )
        .await;
        assert_eq!(output.tracks.len(), 1);
        assert_eq!(output.rows.len(), 3);
        assert_eq!(output.rows[0].import_status, "SKIPPED_DETAIL_UNAVAILABLE");
        assert_eq!(output.rows[1].import_status, "IMPORTED");
        assert_eq!(output.rows[2].import_status, "SKIPPED_REQUEST_LIMIT");
    }
    #[tokio::test]
    async fn deadline_preserves_prior_inline_success() {
        let mut list = songs(page(3, &[1, 2, 3]).as_bytes(), "123").unwrap();
        list[0].track = Some(track("1"));
        let output = hydrate(
            list,
            "123",
            None,
            |_| std::future::pending(),
            20,
            Duration::from_millis(10),
        )
        .await;
        assert_eq!(output.tracks.len(), 1);
        assert_eq!(output.rows[2].import_status, "SKIPPED_TIME_LIMIT");
    }
}
