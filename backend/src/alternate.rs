use crate::{
    models::{AlternateVersionCandidate, AlternateVersionSearchResult, Track, VersionType},
    normalize::{is_alternate_version, parse_track_version, token_similarity},
    writers::youtube::normalize_youtube_metadata,
};
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRadarProgress {
    total: usize,
    cursor: usize,
    batch_size: usize,
    cancelled: bool,
}

impl VersionRadarProgress {
    pub fn new(source_count: usize, batch_size: usize, request_limit: usize) -> Self {
        Self {
            total: source_count.min(request_limit),
            cursor: 0,
            batch_size: batch_size.max(1),
            cancelled: false,
        }
    }

    pub fn next_batch(&mut self) -> Vec<usize> {
        if self.cancelled || self.cursor >= self.total {
            return Vec::new();
        }
        let end = (self.cursor + self.batch_size).min(self.total);
        let batch = (self.cursor..end).collect();
        self.cursor = end;
        batch
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn is_complete(&self) -> bool {
        self.cancelled || self.cursor >= self.total
    }
}

pub fn alternate_add_allowed(candidate_selected: bool, user_confirmed: bool) -> bool {
    candidate_selected && user_confirmed
}

pub fn default_version_types() -> Vec<VersionType> {
    vec![
        VersionType::Live,
        VersionType::Concert,
        VersionType::Remix,
        VersionType::Acoustic,
        VersionType::Unplugged,
        VersionType::Remastered,
        VersionType::Instrumental,
        VersionType::Cover,
    ]
}

pub fn version_query_term(version: &VersionType) -> &'static str {
    match version {
        VersionType::Original => "official audio",
        VersionType::Live => "live",
        VersionType::Concert => "concert",
        VersionType::Remix => "remix",
        VersionType::Remastered => "remastered",
        VersionType::Acoustic => "acoustic",
        VersionType::Unplugged => "unplugged",
        VersionType::Instrumental => "instrumental",
        VersionType::Cover => "cover",
        VersionType::SpedUp => "sped up",
        VersionType::Slowed => "slowed",
        VersionType::RadioEdit => "radio edit",
        VersionType::Karaoke => "karaoke",
        VersionType::BonusTrack => "bonus track",
        VersionType::Reaction => "reaction",
        VersionType::Nightcore => "nightcore",
        VersionType::Other | VersionType::Unknown => "alternate version",
    }
}

#[allow(clippy::too_many_arguments)]
pub fn score_youtube_version_candidate(
    source: &Track,
    raw_title: &str,
    channel: &str,
    source_url: &str,
    duration_ms: Option<u32>,
    requested: &HashSet<VersionType>,
) -> Option<AlternateVersionCandidate> {
    let (title, artists) = normalize_youtube_metadata(raw_title, channel);
    let source_version = parse_track_version(&source.title);
    let target_version = parse_track_version(&title);
    let base_title_similarity =
        token_similarity(&source_version.base_title, &target_version.base_title);
    let artist_similarity = token_similarity(&source.artists.join(" "), &artists.join(" "));
    if base_title_similarity < 0.55 || artist_similarity < 0.25 {
        return None;
    }
    if !requested.contains(&target_version.version_type) {
        return None;
    }
    let channel_key = channel.to_lowercase();
    let official = channel_key.contains("topic")
        || channel_key.contains("official")
        || channel_key.contains("vevo");
    let version_score = if target_version.version_type == source_version.version_type {
        0.65
    } else if is_alternate_version(&target_version.version_type) {
        1.0
    } else {
        0.45
    };
    let confidence = (base_title_similarity * 0.55
        + artist_similarity * 0.25
        + version_score * 0.12
        + if official { 0.08 } else { 0.02 })
    .clamp(0.0, 0.99);
    Some(AlternateVersionCandidate {
        title,
        artists,
        platform: "youtube".into(),
        version_type: target_version.version_type.clone(),
        duration_ms,
        official_status: if official {
            "official_or_topic_channel"
        } else {
            "unverified_channel"
        }
        .into(),
        match_confidence: confidence,
        source_url: source_url.into(),
        reason: format!(
            "基础歌名 {:.0}% · 艺人 {:.0}% · 版本 {} · {}",
            base_title_similarity * 100.0,
            artist_similarity * 100.0,
            version_query_term(&target_version.version_type),
            if official {
                "官方/Topic 频道"
            } else {
                "频道未验证"
            }
        ),
        is_alternate_version: is_alternate_version(&target_version.version_type),
    })
}

pub fn mock_version_search(
    source: Track,
    requested: Vec<VersionType>,
) -> AlternateVersionSearchResult {
    let requested = if requested.is_empty() {
        default_version_types()
    } else {
        requested
    };
    let candidates = requested
        .into_iter()
        .take(4)
        .map(|version| AlternateVersionCandidate {
            title: format!("{} ({})", source.title, version_query_term(&version)),
            artists: source.artists.clone(),
            platform: "mock".into(),
            version_type: version,
            duration_ms: source.duration_ms,
            official_status: "mock_connector".into(),
            match_confidence: 0.8,
            source_url: "mock://alternate-version".into(),
            reason: "Mock connector：只用于验证版本分类、确认和界面流程，不代表真实平台结果。"
                .into(),
            is_alternate_version: true,
        })
        .collect();
    AlternateVersionSearchResult {
        source_track: source,
        provider: "Mock alternate-version connector".into(),
        status: "MOCK_VERIFIED".into(),
        message: "这是明确标注的 Mock 结果；没有调用或冒充 YouTube。".into(),
        candidates,
        is_mock: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::{detect_version, normalize_text};
    use std::collections::HashMap;

    fn track(title: &str, artist: &str) -> Track {
        Track {
            id: "source".into(),
            title: title.into(),
            normalized_title: normalize_text(title),
            artists: vec![artist.into()],
            album: None,
            genres: Vec::new(),
            release_year: None,
            language: None,
            duration_ms: Some(220_000),
            platform: "file".into(),
            platform_url: None,
            external_ids: HashMap::new(),
            version_type: detect_version(title),
            mood_tags: Vec::new(),
            energy_score: None,
            popularity: None,
            metadata_confidence: 1.0,
        }
    }

    #[test]
    fn original_can_find_live_and_remix_when_explicitly_requested() {
        let source = track("Blinding Lights", "The Weeknd");
        let requested = HashSet::from([VersionType::Live, VersionType::Remix]);
        let live = score_youtube_version_candidate(
            &source,
            "The Weeknd - Blinding Lights (Live)",
            "The Weeknd VEVO",
            "https://youtube.test/live",
            None,
            &requested,
        )
        .unwrap();
        let remix = score_youtube_version_candidate(
            &source,
            "The Weeknd - Blinding Lights (Remix)",
            "The Weeknd - Topic",
            "https://youtube.test/remix",
            None,
            &requested,
        )
        .unwrap();
        assert_eq!(live.version_type, VersionType::Live);
        assert_eq!(remix.version_type, VersionType::Remix);
        assert!(live.is_alternate_version && remix.is_alternate_version);
    }

    #[test]
    fn false_remix_keyword_is_not_misclassified() {
        assert_eq!(
            parse_track_version("Remix to Ignition").version_type,
            VersionType::Original
        );
    }

    #[test]
    fn unrelated_same_title_artist_is_rejected() {
        let source = track("Hello", "Adele");
        let requested = HashSet::from([VersionType::Live]);
        assert!(
            score_youtube_version_candidate(
                &source,
                "Lionel Richie - Hello (Live)",
                "Lionel Richie Official",
                "https://youtube.test/wrong",
                None,
                &requested,
            )
            .is_none()
        );
    }

    #[test]
    fn version_radar_uses_bounded_batches() {
        let mut progress = VersionRadarProgress::new(17, 4, 10);
        assert_eq!(progress.next_batch(), vec![0, 1, 2, 3]);
        assert_eq!(progress.next_batch(), vec![4, 5, 6, 7]);
        assert_eq!(progress.next_batch(), vec![8, 9]);
        assert!(progress.is_complete());
        assert!(progress.next_batch().is_empty());
    }

    #[test]
    fn version_radar_cancellation_stops_future_batches() {
        let mut progress = VersionRadarProgress::new(20, 5, 20);
        assert_eq!(progress.next_batch().len(), 5);
        progress.cancel();
        assert!(progress.is_complete());
        assert!(progress.next_batch().is_empty());
    }

    #[test]
    fn alternate_version_add_requires_preview_confirmation() {
        assert!(!alternate_add_allowed(true, false));
        assert!(!alternate_add_allowed(false, true));
        assert!(alternate_add_allowed(true, true));
    }
}
