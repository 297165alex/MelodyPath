use super::metadata::{MetadataMatchStatus, MetadataResolver, ResolutionOutcome};
use crate::{
    models::Track,
    normalize::{detect_version, normalize_text, token_similarity},
};
use anyhow::Context;
use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::{Instant, sleep_until};

const DEFAULT_BASE_URL: &str = "https://musicbrainz.org/ws/2";
const MIN_REQUEST_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug)]
pub struct MusicBrainzResolver {
    client: Client,
    base_url: String,
    next_request_at: Mutex<Instant>,
}

impl MusicBrainzResolver {
    pub fn new(client: Client) -> Self {
        Self {
            client,
            base_url: DEFAULT_BASE_URL.into(),
            next_request_at: Mutex::new(Instant::now()),
        }
    }

    #[cfg(test)]
    fn without_network() -> Self {
        Self::new(Client::new())
    }

    async fn wait_for_rate_limit(&self) {
        let mut next = self.next_request_at.lock().await;
        let now = Instant::now();
        if *next > now {
            sleep_until(*next).await;
        }
        *next = Instant::now() + MIN_REQUEST_INTERVAL;
    }

    async fn search(&self, source: &Track) -> anyhow::Result<MusicBrainzResponse> {
        self.wait_for_rate_limit().await;
        let artist = source.artists.first().cloned().unwrap_or_default();
        let query = format!(
            "recording:\"{}\" AND artist:\"{}\"",
            escape_query(&source.title),
            escape_query(&artist)
        );
        self.client
            .get(format!("{}/recording", self.base_url.trim_end_matches('/')))
            .query(&[("query", query.as_str()), ("fmt", "json"), ("limit", "8")])
            .send()
            .await
            .context("MusicBrainz request failed")?
            .error_for_status()
            .context("MusicBrainz returned an error")?
            .json()
            .await
            .context("MusicBrainz response was invalid")
    }

    fn choose(&self, source: &Track, response: MusicBrainzResponse) -> ResolutionOutcome {
        response
            .recordings
            .into_iter()
            .filter_map(|candidate| scored_candidate(source, candidate))
            .max_by(|left, right| left.match_confidence.total_cmp(&right.match_confidence))
            .filter(|outcome| outcome.match_confidence >= 0.68)
            .unwrap_or_else(|| ResolutionOutcome::unmatched(source.clone()))
    }
}

#[async_trait]
impl MetadataResolver for MusicBrainzResolver {
    async fn resolve(&self, track: &Track) -> anyhow::Result<ResolutionOutcome> {
        Ok(self.choose(track, self.search(track).await?))
    }
}

fn escape_query(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn scored_candidate(source: &Track, candidate: MusicBrainzRecording) -> Option<ResolutionOutcome> {
    let artists: Vec<String> = candidate
        .artist_credit
        .iter()
        .filter_map(|credit| {
            credit
                .name
                .clone()
                .or_else(|| credit.artist.as_ref().map(|artist| artist.name.clone()))
        })
        .filter(|artist| !artist.trim().is_empty())
        .collect();
    if artists.is_empty() || candidate.title.trim().is_empty() {
        return None;
    }

    let title_score = token_similarity(&source.title, &candidate.title);
    let artist_score = source
        .artists
        .iter()
        .flat_map(|source_artist| {
            artists
                .iter()
                .map(move |candidate_artist| token_similarity(source_artist, candidate_artist))
        })
        .fold(0.0_f32, f32::max);
    let duration_score = source
        .duration_ms
        .zip(candidate.length)
        .map(|(left, right)| {
            let difference = left.abs_diff(right);
            if difference <= 2_000 {
                1.0
            } else if difference <= 5_000 {
                0.85
            } else if difference <= 12_000 {
                0.55
            } else {
                0.0
            }
        });
    let confidence = duration_score.map_or(title_score * 0.60 + artist_score * 0.40, |duration| {
        title_score * 0.52 + artist_score * 0.36 + duration * 0.12
    });
    if title_score < 0.60 || artist_score < 0.55 {
        return None;
    }

    let release = candidate.releases.first();
    let mut external_ids = source.external_ids.clone();
    external_ids.insert("musicbrainz".into(), candidate.id.clone());
    external_ids.insert("mbid".into(), candidate.id.clone());
    let status = if confidence >= 0.88 {
        MetadataMatchStatus::HighMatch
    } else {
        MetadataMatchStatus::MediumMatch
    };
    let title = candidate.title;
    let track = Track {
        id: source.id.clone(),
        normalized_title: normalize_text(&title),
        version_type: detect_version(&title),
        title,
        artists,
        album: release.map(|item| item.title.clone()),
        genres: source.genres.clone(),
        release_year: release
            .and_then(|item| item.date.as_deref())
            .and_then(|date| date.get(..4))
            .and_then(|year| year.parse().ok()),
        language: source.language.clone(),
        duration_ms: candidate.length.or(source.duration_ms),
        platform: "musicbrainz".into(),
        platform_url: Some(format!(
            "https://musicbrainz.org/recording/{}",
            candidate.id
        )),
        external_ids,
        mood_tags: source.mood_tags.clone(),
        energy_score: source.energy_score,
        popularity: source.popularity,
        metadata_confidence: confidence,
    };
    Some(ResolutionOutcome {
        track,
        status,
        source: Some("MusicBrainz".into()),
        match_confidence: confidence,
    })
}

#[derive(Debug, Deserialize)]
struct MusicBrainzResponse {
    #[serde(default)]
    recordings: Vec<MusicBrainzRecording>,
}

#[derive(Debug, Deserialize)]
struct MusicBrainzRecording {
    id: String,
    title: String,
    length: Option<u32>,
    #[serde(rename = "artist-credit", default)]
    artist_credit: Vec<MusicBrainzArtistCredit>,
    #[serde(default)]
    releases: Vec<MusicBrainzRelease>,
}

#[derive(Debug, Deserialize)]
struct MusicBrainzArtistCredit {
    name: Option<String>,
    artist: Option<MusicBrainzArtist>,
}

#[derive(Debug, Deserialize)]
struct MusicBrainzArtist {
    name: String,
}

#[derive(Debug, Deserialize)]
struct MusicBrainzRelease {
    title: String,
    date: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::VersionType;
    use serde_json::json;
    use std::collections::HashMap;

    fn source(title: &str, artist: &str) -> Track {
        Track {
            id: format!("{artist}-{title}"),
            title: title.into(),
            normalized_title: normalize_text(title),
            artists: vec![artist.into()],
            album: None,
            genres: Vec::new(),
            release_year: None,
            language: None,
            duration_ms: None,
            platform: "manual".into(),
            platform_url: None,
            external_ids: HashMap::new(),
            version_type: VersionType::Original,
            mood_tags: Vec::new(),
            energy_score: None,
            popularity: None,
            metadata_confidence: 0.25,
        }
    }

    fn response(title: &str, artist: &str, album: &str) -> MusicBrainzResponse {
        serde_json::from_value(json!({
            "recordings": [{
                "id": "recording-id",
                "title": title,
                "length": 242000,
                "artist-credit": [{"name": artist, "artist": {"name": artist}}],
                "releases": [{"title": album, "date": "2020-01-01"}]
            }]
        }))
        .unwrap()
    }

    #[test]
    fn resolves_chinese_track() {
        let resolver = MusicBrainzResolver::without_network();
        let outcome = resolver.choose(
            &source("晴天", "周杰伦"),
            response("晴天", "周杰伦", "叶惠美"),
        );
        assert_eq!(outcome.status, MetadataMatchStatus::HighMatch);
        assert_eq!(outcome.track.album.as_deref(), Some("叶惠美"));
    }

    #[test]
    fn resolves_english_track() {
        let resolver = MusicBrainzResolver::without_network();
        let outcome = resolver.choose(
            &source("instagram", "DEAN"),
            response("instagram", "DEAN", "instagram"),
        );
        assert_eq!(outcome.status, MetadataMatchStatus::HighMatch);
    }

    #[test]
    fn resolves_korean_and_japanese_tracks() {
        let resolver = MusicBrainzResolver::without_network();
        let korean = resolver.choose(
            &source("Blueming", "IU"),
            response("Blueming", "IU", "Love poem"),
        );
        let japanese = resolver.choose(
            &source("First Love", "宇多田光"),
            response("First Love", "宇多田光", "First Love"),
        );
        assert_eq!(korean.status, MetadataMatchStatus::HighMatch);
        assert_eq!(japanese.status, MetadataMatchStatus::HighMatch);
    }

    #[test]
    fn nonexistent_track_stays_unmatched_and_unchanged() {
        let resolver = MusicBrainzResolver::without_network();
        let input = source("不存在歌曲 XYZ", "Nobody");
        let outcome = resolver.choose(
            &input,
            MusicBrainzResponse {
                recordings: Vec::new(),
            },
        );
        assert_eq!(outcome.status, MetadataMatchStatus::Unmatched);
        assert_eq!(outcome.track.title, input.title);
        assert_eq!(outcome.track.artists, input.artists);
        assert!(outcome.track.external_ids.is_empty());
    }
}
