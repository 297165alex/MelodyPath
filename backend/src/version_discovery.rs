//! Independent, deterministic discovery; never sends provider metadata to an LLM.
use crate::{
    models::{Track, VersionType},
    normalize::{parse_track_version, token_similarity},
    writers::{PlaylistWriter, spotify::SpotifyPlaylistWriter, youtube::YoutubePlaylistWriter},
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, sync::LazyLock, time::Duration};
use tokio::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VersionCandidate {
    pub title: String,
    pub artist: String,
    pub platform: String,
    pub url: String,
    pub version_type: String,
    pub language: Option<String>,
    pub confidence: f32,
    pub reason: String,
}
#[derive(Deserialize)]
pub struct DiscoveryRequest {
    pub track: Track,
    #[serde(default)]
    pub preferences: Vec<String>,
}
#[derive(Serialize)]
pub struct DiscoveryResult {
    pub source_track: Track,
    pub candidates: Vec<VersionCandidate>,
    pub provider_status: Vec<String>,
    pub status: String,
}
#[async_trait]
pub trait VersionProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn discover(&self, source: &Track) -> anyhow::Result<Vec<VersionCandidate>>;
}
pub struct SpotifyAdapter<'a>(pub &'a SpotifyPlaylistWriter, pub Option<&'a str>);
pub struct YoutubeAdapter<'a>(pub &'a YoutubePlaylistWriter, pub Option<&'a str>);
pub struct MusicBrainzAdapter;

fn classify(title: &str) -> (String, Option<String>) {
    let lower = title.to_lowercase();
    let language = [
        ("korean", "ko"),
        ("한국어", "ko"),
        ("chinese", "zh"),
        ("中文", "zh"),
        ("japanese", "ja"),
        ("日本語", "ja"),
        ("english", "en"),
    ]
    .iter()
    .find(|(word, _)| lower.contains(word))
    .map(|(_, code)| code.to_string());
    let kind = if lower.contains("cover") && language.is_some() {
        "language_cover".into()
    } else if lower.contains("extended") {
        "extended".into()
    } else {
        serde_json::to_value(parse_track_version(title).version_type)
            .unwrap()
            .as_str()
            .unwrap()
            .to_string()
    };
    (kind, language)
}

fn candidate(
    source: &Track,
    title: String,
    artist: String,
    platform: &str,
    url: String,
) -> Option<VersionCandidate> {
    let (kind, language) = classify(&title);
    let source_base = parse_track_version(&source.title).base_title;
    let mut recording_title = title.to_lowercase();
    for artist in &source.artists {
        for separator in [" - ", " — ", " – ", " | "] {
            if let Some(rest) =
                recording_title.strip_prefix(&format!("{}{separator}", artist.to_lowercase()))
            {
                recording_title = rest.to_string();
            }
        }
    }
    let mut target_base = parse_track_version(&recording_title).base_title;
    for artist in &source.artists {
        target_base = target_base.replace(&artist.to_lowercase(), " ");
    }
    for tag in [
        "korean",
        "chinese",
        "japanese",
        "english",
        "cover",
        "extended",
        "한국어",
        "中文",
        "日本語",
    ] {
        target_base = target_base.replace(tag, " ");
    }
    let title_score = token_similarity(&source_base, &target_base);
    let artist_score = token_similarity(&source.artists.join(" "), &artist);
    let cover =
        kind == "cover" || kind == "language_cover" || title.to_lowercase().contains("cover");
    // A cover may change performer, but must explicitly name the source artist.
    let credited = token_similarity(&source.artists.join(" "), &title) >= 0.15;
    if title_score < 0.5 || (artist_score < 0.25 && !(cover && credited)) {
        return None;
    }
    Some(VersionCandidate {
        title,
        artist,
        platform: platform.into(),
        url,
        version_type: kind,
        language,
        confidence: (title_score * 0.7 + artist_score * 0.25 + if credited { 0.05 } else { 0.0 })
            .min(0.99),
        reason: format!(
            "Title overlap {:.0}%; artist overlap {:.0}%; explicit version metadata. Cover identity requires review.",
            title_score * 100.0,
            artist_score * 100.0
        ),
    })
}

#[async_trait]
impl VersionProvider for SpotifyAdapter<'_> {
    fn name(&self) -> &'static str {
        "Spotify"
    }
    async fn discover(&self, source: &Track) -> anyhow::Result<Vec<VersionCandidate>> {
        anyhow::ensure!(
            self.0.connection_status(self.1).await.connected,
            "AUTH_REQUIRED"
        );
        let mut output = Vec::new();
        for suffix in [
            "",
            "Live",
            "Acoustic",
            "Cover",
            "Korean Cover",
            "Chinese Cover",
            "Japanese Cover",
            "Remix",
            "Instrumental",
            "Extended",
        ] {
            let mut query = source.clone();
            query.title = format!(
                "{} {}",
                parse_track_version(&source.title).base_title,
                suffix
            );
            query.platform = "version_discovery".into();
            query.external_ids.clear();
            for item in self.0.search_track(&query, self.1).await?.candidates {
                if let Some(url) = item.target_url
                    && let Some(found) =
                        candidate(source, item.title, item.artists.join(", "), "spotify", url)
                {
                    output.push(found);
                }
            }
        }
        Ok(output)
    }
}
#[async_trait]
impl VersionProvider for YoutubeAdapter<'_> {
    fn name(&self) -> &'static str {
        "YouTube"
    }
    async fn discover(&self, source: &Track) -> anyhow::Result<Vec<VersionCandidate>> {
        anyhow::ensure!(
            self.0.connection_status(self.1).await.connected,
            "AUTH_REQUIRED"
        );
        let mut output = Vec::new();
        for types in [
            vec![
                VersionType::Original,
                VersionType::Live,
                VersionType::Acoustic,
                VersionType::Cover,
            ],
            vec![VersionType::Remix, VersionType::Instrumental],
        ] {
            for item in self
                .0
                .search_alternate_versions(source, types, self.1)
                .await?
                .candidates
            {
                if let Some(found) = candidate(
                    source,
                    item.title,
                    item.artists.join(", "),
                    "youtube",
                    item.source_url,
                ) {
                    output.push(found);
                }
            }
        }
        // Explicit language/extended query plans reuse the connector; no new OAuth or HTTP surface.
        for suffix in [
            "Korean Cover",
            "Chinese Cover",
            "Japanese Cover",
            "Extended",
        ] {
            let mut query = source.clone();
            query.title = format!("{} ({suffix})", source.title);
            for item in self
                .0
                .search_alternate_versions(
                    &query,
                    vec![
                        VersionType::Cover,
                        VersionType::Original,
                        VersionType::Other,
                    ],
                    self.1,
                )
                .await?
                .candidates
            {
                if let Some(found) = candidate(
                    source,
                    item.title,
                    item.artists.join(", "),
                    "youtube",
                    item.source_url,
                ) {
                    output.push(found);
                }
            }
        }
        Ok(output)
    }
}
static MB_RATE: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));
#[async_trait]
impl VersionProvider for MusicBrainzAdapter {
    fn name(&self) -> &'static str {
        "MusicBrainz"
    }
    async fn discover(&self, source: &Track) -> anyhow::Result<Vec<VersionCandidate>> {
        let _guard = MB_RATE.lock().await;
        tokio::time::sleep(Duration::from_secs(1)).await;
        let title = parse_track_version(&source.title)
            .base_title
            .replace('\\', " ")
            .replace('"', " ");
        let response: serde_json::Value = reqwest::Client::new()
            .get("https://musicbrainz.org/ws/2/recording")
            .header(
                "User-Agent",
                "MelodyPath/0.1 (version discovery; https://github.com/MelodyPath)",
            )
            .query(&[
                ("query", format!("recording:\"{title}\"")),
                ("fmt", "json".into()),
                ("limit", "40".into()),
            ])
            .timeout(Duration::from_secs(12))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let mut output = Vec::new();
        if let Some(items) = response["recordings"].as_array() {
            for item in items {
                let (Some(id), Some(title)) = (item["id"].as_str(), item["title"].as_str()) else {
                    continue;
                };
                if uuid::Uuid::parse_str(id).is_err() {
                    continue;
                }
                let artists = item["artist-credit"]
                    .as_array()
                    .map(|credits| {
                        credits
                            .iter()
                            .filter_map(|c| c["name"].as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .unwrap_or_default();
                let annotation = item["disambiguation"].as_str().unwrap_or("");
                if let Some(found) = candidate(
                    source,
                    format!("{title} {annotation}").trim().into(),
                    artists,
                    "musicbrainz",
                    format!("https://musicbrainz.org/recording/{id}"),
                ) {
                    output.push(found);
                }
            }
        }
        Ok(output)
    }
}
pub async fn discover(
    request: DiscoveryRequest,
    providers: &[&dyn VersionProvider],
) -> DiscoveryResult {
    let mut candidates = Vec::new();
    let mut provider_status = Vec::new();
    for provider in providers {
        match provider.discover(&request.track).await {
            Ok(found) => {
                provider_status.push(format!(
                    "{}: OK ({} candidates)",
                    provider.name(),
                    found.len()
                ));
                candidates.extend(found);
            }
            Err(_) => provider_status.push(format!(
                "{}: unavailable — check authorization, configuration or provider availability",
                provider.name()
            )),
        }
    }
    rank(&mut candidates, &request.preferences);
    let status = if candidates.is_empty() {
        "NO_VERIFIED_CANDIDATES"
    } else {
        "READY"
    }
    .into();
    DiscoveryResult {
        source_track: request.track,
        candidates,
        provider_status,
        status,
    }
}
fn rank(candidates: &mut Vec<VersionCandidate>, preferences: &[String]) {
    let mut seen = HashSet::new();
    candidates.retain(|c| seen.insert(c.url.clone()));
    let affinity = |c: &VersionCandidate| {
        preferences
            .iter()
            .any(|p| p == &c.version_type || c.language.as_ref() == Some(p))
    };
    candidates.sort_by(|a, b| {
        (b.confidence + if affinity(b) { 0.15 } else { 0.0 })
            .total_cmp(&(a.confidence + if affinity(a) { 0.15 } else { 0.0 }))
            .then_with(|| a.url.cmp(&b.url))
    });
    for c in candidates.iter_mut() {
        if affinity(c) {
            c.reason.push_str(&format!(
                " Matches your preference for {} arrangements or language.",
                c.version_type
            ));
        }
    }
    candidates.truncate(12);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn language_and_extended_classification() {
        for (title, kind, lang) in [
            ("Song", "original", None),
            ("Song (Live)", "live", None),
            ("Song (Acoustic)", "acoustic", None),
            ("Song (Cover)", "cover", None),
            ("Song (Remix)", "remix", None),
            ("Song (Instrumental)", "instrumental", None),
            ("Song (Korean Cover)", "language_cover", Some("ko")),
            ("Song Chinese Cover", "language_cover", Some("zh")),
            ("Song Japanese Cover", "language_cover", Some("ja")),
            ("Song Extended Mix", "extended", None),
        ] {
            let result = classify(title);
            assert_eq!(result.0, kind);
            assert_eq!(result.1.as_deref(), lang);
        }
    }
    fn source() -> Track {
        serde_json::from_value(serde_json::json!({"id":"synthetic", "title":"Love Story", "normalized_title":"love story", "artists":["Taylor Swift"], "genres":[], "platform":"file", "external_ids":{}, "version_type":"original", "mood_tags":[], "metadata_confidence":1.0})).unwrap()
    }
    #[test]
    fn covers_require_source_credit_and_unrelated_artists_are_rejected() {
        assert!(
            candidate(
                &source(),
                "Love Story (Live)".into(),
                "Unrelated Artist".into(),
                "test",
                "test://1".into()
            )
            .is_none()
        );
        assert!(
            candidate(
                &source(),
                "Taylor Swift - Love Story (Korean Cover)".into(),
                "New Singer".into(),
                "test",
                "test://2".into()
            )
            .is_some()
        );
    }
    struct FixtureProvider(bool);
    #[async_trait]
    impl VersionProvider for FixtureProvider {
        fn name(&self) -> &'static str {
            "Explicit fixture"
        }
        async fn discover(&self, source: &Track) -> anyhow::Result<Vec<VersionCandidate>> {
            anyhow::ensure!(self.0, "synthetic unavailable");
            Ok(vec![
                candidate(
                    source,
                    "Love Story (Live)".into(),
                    "Taylor Swift".into(),
                    "test",
                    "test://live".into(),
                )
                .unwrap(),
            ])
        }
    }
    #[tokio::test]
    async fn provider_failure_isolated_and_empty_is_not_demo() {
        let result = discover(
            DiscoveryRequest {
                track: source(),
                preferences: vec![],
            },
            &[&FixtureProvider(false), &FixtureProvider(true)],
        )
        .await;
        assert_eq!(result.candidates.len(), 1);
        assert!(result.provider_status[0].contains("unavailable"));
        let empty = discover(
            DiscoveryRequest {
                track: source(),
                preferences: vec![],
            },
            &[&FixtureProvider(false)],
        )
        .await;
        assert!(empty.candidates.is_empty());
        assert_eq!(empty.status, "NO_VERIFIED_CANDIDATES");
    }
    #[test]
    fn taste_ranking_deduplicates_without_inflating_confidence() {
        let c = |kind: &str, confidence| VersionCandidate {
            title: "Song".into(),
            artist: "Artist".into(),
            platform: "test".into(),
            url: kind.into(),
            version_type: kind.into(),
            language: None,
            confidence,
            reason: String::new(),
        };
        let mut items = vec![c("live", 0.9), c("acoustic", 0.8), c("acoustic", 0.8)];
        rank(&mut items, &["acoustic".into()]);
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].version_type, "acoustic");
        assert_eq!(items[0].confidence, 0.8);
        assert!(items[0].reason.contains("preference"));
    }
}
