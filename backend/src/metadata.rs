use crate::{
    engine,
    genre::{canonicalize_genre, normalize_genres},
    models::{
        DataState, ImportAnalysisSummary, ImportedTrack, MetadataStatus, PersonalDemo, Playlist,
        Track,
    },
    normalize::{normalize_text, token_similarity},
    recommendation::{
        LastFmRecommendationProvider, RecommendationProvider, build_real_recommendations,
    },
};
use reqwest::Client;
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc, time::Duration};
use tokio::{
    sync::RwLock,
    time::{sleep, timeout},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct MetadataService {
    client: Client,
    storefront: String,
    max_tracks: usize,
    cache: Arc<RwLock<HashMap<String, Option<ItunesTrack>>>>,
    recommendation_provider: Arc<dyn RecommendationProvider>,
}

impl MetadataService {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(7))
            .user_agent("MelodyPath/0.1 (local music analysis course project)")
            .build()
            .unwrap_or_default();
        let storefront = std::env::var("ITUNES_STOREFRONT").unwrap_or_else(|_| "CN".into());
        let recommendation_provider =
            Arc::new(LastFmRecommendationProvider::from_env(client.clone()));
        Self {
            client,
            storefront,
            max_tracks: std::env::var("LOCAL_ANALYSIS_MAX_TRACKS")
                .ok()
                .and_then(|value| value.parse().ok())
                .unwrap_or(12),
            cache: Arc::new(RwLock::new(HashMap::new())),
            recommendation_provider,
        }
    }

    #[cfg(test)]
    pub fn with_recommendation_provider(
        recommendation_provider: Arc<dyn RecommendationProvider>,
    ) -> Self {
        let mut service = Self::new();
        service.recommendation_provider = recommendation_provider;
        service
    }

    /// 无需音乐平台账号：先匹配 Apple 公开目录，失败时使用本地艺术家知识库。
    /// 网络不可用时也会返回基础统计，不会让整个分析失败。
    pub async fn analyze(&self, mut playlist: Playlist) -> PersonalDemo {
        let enrich_count = playlist.tracks.len().min(self.max_tracks);
        for track in playlist.tracks.iter_mut().take(enrich_count) {
            let matched =
                match timeout(Duration::from_secs(8), self.lookup_itunes_cached(track)).await {
                    Ok(Ok(Some(candidate))) => {
                        apply_itunes_candidate(track, candidate);
                        true
                    }
                    _ => false,
                };
            if !matched {
                apply_local_artist_profile(track);
            }
        }

        if playlist.tracks.len() > enrich_count {
            for track in playlist.tracks.iter_mut().skip(enrich_count) {
                apply_local_artist_profile(track);
            }
        }

        let report = engine::analyze_playlist(&playlist);
        let (recommendations, recommendation_summary, route) =
            build_real_recommendations(self.recommendation_provider.as_ref(), &playlist, &report)
                .await;
        PersonalDemo {
            analysis_id: Uuid::new_v4().to_string(),
            playlist,
            report,
            recommendations,
            route,
            recommendation_summary,
            import_summary: None,
            unmatched_tracks: Vec::new(),
        }
    }

    pub async fn analyze_imported(
        &self,
        name: String,
        source_label: String,
        data_state: DataState,
        input_count: usize,
        mut imported: Vec<ImportedTrack>,
    ) -> PersonalDemo {
        let mut provider_requests = 0;
        for imported_track in &mut imported {
            if imported_track.metadata_status == MetadataStatus::Complete {
                continue;
            }
            if provider_requests >= self.max_tracks {
                mark_missing(imported_track, "超过本次联网补全上限，仍参与基础分析");
                continue;
            }
            provider_requests += 1;
            let mut track = imported_to_track(imported_track);
            match timeout(Duration::from_secs(8), self.lookup_itunes_cached(&track)).await {
                Ok(Ok(Some(candidate))) => {
                    apply_itunes_candidate(&mut track, candidate);
                    apply_track_metadata(imported_track, &track);
                }
                Ok(Ok(None)) => mark_missing(imported_track, "公开目录暂未匹配到该歌曲"),
                Ok(Err(_)) => mark_missing(imported_track, "元数据查询失败，已保留原始歌曲"),
                Err(_) => mark_missing(imported_track, "元数据查询超时，已保留原始歌曲"),
            }
            sleep(Duration::from_millis(40)).await;
        }

        let playlist = Playlist {
            id: Uuid::new_v4().to_string(),
            name,
            owner_label: "当前用户".into(),
            source: source_label.clone(),
            is_demo: false,
            tracks: imported.iter().map(imported_to_track).collect(),
        };
        let report = engine::analyze_playlist(&playlist);
        let (recommendations, recommendation_summary, route) =
            build_real_recommendations(self.recommendation_provider.as_ref(), &playlist, &report)
                .await;
        let complete_metadata_count = imported
            .iter()
            .filter(|track| track.metadata_status == MetadataStatus::Complete)
            .count();
        let partial_metadata_count = imported
            .iter()
            .filter(|track| track.metadata_status == MetadataStatus::Partial)
            .count();
        let unmatched_tracks: Vec<_> = imported
            .iter()
            .filter(|track| track.metadata_status == MetadataStatus::Missing)
            .cloned()
            .collect();
        let analyzed_count = playlist.tracks.len();
        PersonalDemo {
            analysis_id: Uuid::new_v4().to_string(),
            playlist,
            report: report.clone(),
            recommendations,
            route,
            recommendation_summary,
            import_summary: Some(ImportAnalysisSummary {
                data_state,
                source_label,
                input_count,
                parsed_count: imported.len(),
                analyzed_count,
                complete_metadata_count,
                partial_metadata_count,
                unmatched_count: unmatched_tracks.len(),
                genre_matched_count: report.genre_matched_count,
                genre_coverage: report.genre_coverage,
                energy_matched_count: report.energy_matched_count,
                energy_coverage: report.energy_coverage,
            }),
            unmatched_tracks,
        }
    }

    async fn lookup_itunes(&self, source: &Track) -> Result<Option<ItunesTrack>, reqwest::Error> {
        let artist = source.artists.first().cloned().unwrap_or_default();
        let term = format!("{} {}", artist, source.title);
        let response = self
            .client
            .get("https://itunes.apple.com/search")
            .query(&[
                ("term", term.as_str()),
                ("media", "music"),
                ("entity", "song"),
                ("limit", "8"),
                ("country", self.storefront.as_str()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<ItunesResponse>()
            .await?;

        Ok(response
            .results
            .into_iter()
            .filter(|item| item.track_name.is_some() && item.artist_name.is_some())
            .map(|item| {
                let title_score = token_similarity(
                    &source.title,
                    item.track_name.as_deref().unwrap_or_default(),
                );
                let artist_score = source
                    .artists
                    .iter()
                    .map(|artist| {
                        token_similarity(artist, item.artist_name.as_deref().unwrap_or_default())
                    })
                    .fold(0.0_f32, f32::max);
                let score = title_score * 0.62 + artist_score * 0.38;
                (score, item)
            })
            .filter(|(score, _)| *score >= 0.52)
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .map(|(_, item)| item))
    }

    async fn lookup_itunes_cached(
        &self,
        source: &Track,
    ) -> Result<Option<ItunesTrack>, reqwest::Error> {
        let key = format!(
            "{}|{}",
            normalize_text(&source.artists.join(" ")),
            normalize_text(&source.title)
        );
        if let Some(cached) = self.cache.read().await.get(&key).cloned() {
            return Ok(cached);
        }
        let result = self.lookup_itunes(source).await?;
        self.cache.write().await.insert(key, result.clone());
        Ok(result)
    }
}

fn imported_to_track(imported: &ImportedTrack) -> Track {
    Track {
        id: Uuid::new_v4().to_string(),
        title: imported.title.clone(),
        normalized_title: normalize_text(&imported.title),
        artists: imported.artists.clone(),
        album: imported.album.clone(),
        genres: normalize_genres(imported.genres.iter()),
        release_year: imported
            .release_date
            .as_deref()
            .and_then(|date| date.get(..4))
            .and_then(|year| year.parse().ok()),
        language: None,
        duration_ms: imported.duration_ms,
        platform: imported.source.clone(),
        platform_url: None,
        external_ids: Default::default(),
        version_type: crate::normalize::detect_version(&imported.title),
        mood_tags: Vec::new(),
        energy_score: imported.energy_score,
        popularity: None,
        metadata_confidence: imported.metadata_confidence,
    }
}

fn apply_track_metadata(imported: &mut ImportedTrack, track: &Track) {
    imported.album = track.album.clone().or_else(|| imported.album.clone());
    imported.genres = if track.genres.is_empty() {
        normalize_genres(imported.genres.iter())
    } else {
        normalize_genres(track.genres.iter())
    };
    imported.release_date = track
        .release_year
        .map(|year| year.to_string())
        .or_else(|| imported.release_date.clone());
    imported.duration_ms = track.duration_ms.or(imported.duration_ms);
    imported.energy_score = track.energy_score.or(imported.energy_score);
    imported.metadata_confidence = track.metadata_confidence;
    imported.metadata_status = if imported.album.is_some()
        && imported.release_date.is_some()
        && !imported.genres.is_empty()
        && imported.duration_ms.is_some()
    {
        MetadataStatus::Complete
    } else {
        MetadataStatus::Partial
    };
}

fn mark_missing(imported: &mut ImportedTrack, warning: &str) {
    if imported.album.is_some()
        || imported.release_date.is_some()
        || !imported.genres.is_empty()
        || imported.duration_ms.is_some()
    {
        imported.metadata_status = MetadataStatus::Partial;
    } else {
        imported.metadata_status = MetadataStatus::Missing;
    }
    imported.warnings.push(warning.into());
}

#[derive(Debug, Deserialize)]
struct ItunesResponse {
    #[serde(default)]
    results: Vec<ItunesTrack>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ItunesTrack {
    track_id: Option<u64>,
    track_name: Option<String>,
    artist_name: Option<String>,
    collection_name: Option<String>,
    primary_genre_name: Option<String>,
    release_date: Option<String>,
    track_time_millis: Option<u32>,
    track_view_url: Option<String>,
    preview_url: Option<String>,
}

fn apply_itunes_candidate(track: &mut Track, candidate: ItunesTrack) {
    if let Some(title) = candidate.track_name {
        track.title = title.clone();
        track.normalized_title = normalize_text(&title);
    }
    if let Some(artist) = candidate.artist_name {
        track.artists = vec![artist];
    }
    track.album = candidate.collection_name;
    let genre = canonicalize_genre(candidate.primary_genre_name.as_deref().unwrap_or("其他"));
    track.genres = vec![genre.clone()];
    track.release_year = candidate
        .release_date
        .as_deref()
        .and_then(|value| value.get(..4))
        .and_then(|value| value.parse().ok());
    track.duration_ms = candidate.track_time_millis;
    track.platform = "public_catalog".into();
    track.platform_url = candidate.track_view_url;
    if let Some(id) = candidate.track_id {
        track.external_ids.insert("itunes".into(), id.to_string());
    }
    if let Some(preview) = candidate.preview_url {
        track.external_ids.insert("preview_url".into(), preview);
    }
    let (moods, _) = genre_features(&genre);
    track.mood_tags = moods;
    track.language = infer_language(&format!("{} {}", track.title, track.artists.join(" ")));
    track.metadata_confidence = 0.88;
}

fn apply_local_artist_profile(track: &mut Track) {
    let artist = normalize_text(
        track
            .artists
            .first()
            .map(String::as_str)
            .unwrap_or_default(),
    );
    let profile = artist_profile(&artist);
    if let Some((genres, language, _energy, moods)) = profile {
        track.genres = normalize_genres(genres.iter());
        track.language = Some(language.into());
        track.mood_tags = moods.iter().map(|value| (*value).to_string()).collect();
        track.metadata_confidence = 0.68;
    } else {
        track.genres = vec!["元数据待补全".into()];
        track.language = infer_language(&format!("{} {}", track.title, track.artists.join(" ")));
        track.metadata_confidence = 0.35;
    }
}

fn artist_profile(
    artist: &str,
) -> Option<(
    &'static [&'static str],
    &'static str,
    f32,
    &'static [&'static str],
)> {
    let profile = match artist {
        "bibi" | "dean" | "crush" | "heize" | "colde" | "keshi" => (
            &["Korean R&B", "Alternative R&B"][..],
            "韩语/英语",
            0.52,
            &["夜色", "细腻"][..],
        ),
        "newjeans" | "ive" | "le sserafim" | "aespa" | "blackpink" | "bts" | "iu" => (
            &["K-Pop", "Dance Pop"][..],
            "韩语",
            0.72,
            &["明亮", "律动"][..],
        ),
        "周杰伦" | "jay chou" | "陈奕迅" | "eason chan" | "邓紫棋" | "g e m" | "五月天"
        | "毛不易" => (
            &["Mandopop", "C-Pop"][..],
            "中文",
            0.58,
            &["叙事", "怀旧"][..],
        ),
        "宇多田光" | "hikaru utada" | "mariya takeuchi" | "竹内まりや" | "藤井风"
        | "fujii kaze" | "yoasobi" | "aimer" => (
            &["J-Pop", "Japanese R&B"][..],
            "日语",
            0.56,
            &["细腻", "都市"][..],
        ),
        "frank ocean" | "daniel caesar" | "sza" | "the weeknd" | "solange" | "masego" => (
            &["R&B / Soul", "Alternative R&B"][..],
            "英语",
            0.55,
            &["夜色", "柔和"][..],
        ),
        "arctic monkeys" | "the 1975" | "radiohead" | "coldplay" | "the cranberries"
        | "tame impala" => (
            &["Alternative Rock", "Indie Rock"][..],
            "英语",
            0.69,
            &["张力", "迷幻"][..],
        ),
        "taylor swift" | "dua lipa" | "billie eilish" | "lana del rey" | "olivia rodrigo" => (
            &["Pop", "Singer/Songwriter"][..],
            "英语",
            0.62,
            &["流行", "叙事"][..],
        ),
        "laufey" => (
            &["Jazz Pop", "Singer/Songwriter"][..],
            "英语",
            0.39,
            &["温暖", "复古"][..],
        ),
        _ => return None,
    };
    Some(profile)
}

fn genre_features(genre: &str) -> (Vec<String>, f32) {
    let lower = genre.to_lowercase();
    let (moods, energy): (&[&str], f32) = if lower.contains("rock") {
        (&["张力", "热烈"], 0.78)
    } else if lower.contains("hip-hop") || lower.contains("rap") {
        (&["节奏", "自信"], 0.75)
    } else if lower.contains("electronic") || lower.contains("dance") {
        (&["律动", "明亮"], 0.82)
    } else if lower.contains("r&b") || lower.contains("soul") {
        (&["细腻", "夜色"], 0.52)
    } else if lower.contains("jazz") {
        (&["松弛", "复古"], 0.43)
    } else if lower.contains("classical") {
        (&["平静", "专注"], 0.28)
    } else if lower.contains("folk") || lower.contains("singer") {
        (&["叙事", "温暖"], 0.42)
    } else if lower.contains("alternative") || lower.contains("indie") {
        (&["独立", "迷幻"], 0.61)
    } else {
        (&["流行", "明亮"], 0.64)
    };
    (
        moods.iter().map(|value| (*value).to_string()).collect(),
        energy,
    )
}

fn infer_language(text: &str) -> Option<String> {
    let mut has_hangul = false;
    let mut has_kana = false;
    let mut has_han = false;
    for ch in text.chars() {
        let code = ch as u32;
        has_hangul |= (0xAC00..=0xD7AF).contains(&code);
        has_kana |= (0x3040..=0x30FF).contains(&code);
        has_han |= (0x4E00..=0x9FFF).contains(&code);
    }
    if has_hangul {
        Some("韩语".into())
    } else if has_kana {
        Some("日语".into())
    } else if has_han {
        Some("中文/日语".into())
    } else {
        Some("英语/其他".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        models::{RecommendationQueryStats, VersionType},
        recommendation::{
            CandidateRelation, RecommendationCandidate, RecommendationProfile,
            RecommendationProvider, RecommendationProviderError, RecommendationProviderResult,
        },
    };
    use async_trait::async_trait;

    struct MockRecommendationProvider;

    #[async_trait]
    impl RecommendationProvider for MockRecommendationProvider {
        fn source_label(&self) -> &str {
            "Mock Last.fm"
        }

        async fn generate(
            &self,
            _seeds: &[crate::models::RecommendationSeed],
            _profile: &RecommendationProfile,
        ) -> Result<RecommendationProviderResult, RecommendationProviderError> {
            let candidate = RecommendationCandidate {
                track: Track {
                    id: "mock-candidate".into(),
                    title: "A Real Provider Result".into(),
                    normalized_title: "a real provider result".into(),
                    artists: vec!["Provider Artist".into()],
                    album: Some("Provider Album".into()),
                    genres: vec!["Pop".into()],
                    release_year: Some(2026),
                    language: None,
                    duration_ms: Some(200_000),
                    platform: "lastfm".into(),
                    platform_url: None,
                    external_ids: Default::default(),
                    version_type: VersionType::Original,
                    mood_tags: Vec::new(),
                    energy_score: None,
                    popularity: None,
                    metadata_confidence: 0.9,
                },
                mbid: None,
                source_url: None,
                source_provider: "Mock Last.fm".into(),
                source_endpoint: "track.getSimilar".into(),
                seed_track: Some("Source Pop Song".into()),
                seed_artist: Some("Source Artist".into()),
                lastfm_similarity: Some(0.9),
                tags: vec!["Pop".into()],
                normalized_genres: vec!["Pop".into()],
                popularity: None,
                reason: "Last.fm mock similar track".into(),
                confidence: 0.9,
                relation: CandidateRelation::TrackSimilar,
                relaxation_level: 1,
            };
            Ok(RecommendationProviderResult {
                candidates: vec![candidate],
                stats: RecommendationQueryStats {
                    successful_seed_count: 1,
                    raw_track_similar_count: 1,
                    raw_candidate_count: 1,
                    ..Default::default()
                },
                successful_requests: 1,
                failed_requests: 0,
            })
        }
    }

    #[test]
    fn local_profile_enriches_known_artist() {
        let mut track = Track {
            id: "1".into(),
            title: "instagram".into(),
            normalized_title: "instagram".into(),
            artists: vec!["DEAN".into()],
            album: None,
            genres: vec!["待补全".into()],
            release_year: None,
            language: None,
            duration_ms: None,
            platform: "manual".into(),
            platform_url: None,
            external_ids: Default::default(),
            version_type: VersionType::Original,
            mood_tags: vec![],
            energy_score: None,
            popularity: None,
            metadata_confidence: 0.4,
        };
        apply_local_artist_profile(&mut track);
        assert!(track.genres.iter().any(|genre| genre.contains("R&B")));
        assert_eq!(track.energy_score, None, "本地 Genre 画像不得编造 Energy");
    }

    fn imported(title: &str, artist: &str) -> ImportedTrack {
        ImportedTrack {
            title: title.into(),
            artists: vec![artist.into()],
            album: None,
            release_date: None,
            genres: Vec::new(),
            duration_ms: None,
            energy_score: None,
            source: "test.txt".into(),
            original_row: format!("{artist} - {title}"),
            metadata_status: MetadataStatus::Missing,
            metadata_confidence: 0.25,
            warnings: Vec::new(),
        }
    }

    #[tokio::test]
    async fn unmatched_real_import_never_falls_back_to_demo() {
        let mut service = MetadataService::new();
        service.max_tracks = 0;
        let result = service
            .analyze_imported(
                "新歌测试".into(),
                "真实批量文本".into(),
                DataState::RealText,
                1,
                vec![imported("尚未发布到目录的新歌 XYZ", "独立音乐人")],
            )
            .await;

        assert!(!result.playlist.is_demo);
        assert!(!result.report.is_demo);
        assert_eq!(result.playlist.tracks[0].title, "尚未发布到目录的新歌 XYZ");
        assert_eq!(result.unmatched_tracks.len(), 1);
        assert_eq!(result.import_summary.unwrap().analyzed_count, 1);
    }

    #[tokio::test]
    async fn partial_metadata_failure_keeps_every_track_and_reports_counts() {
        let mut service = MetadataService::new();
        service.max_tracks = 0;
        let mut partial = imported("Known Fields", "Artist A");
        partial.album = Some("Album A".into());
        partial.release_date = Some("2025".into());
        partial.metadata_status = MetadataStatus::Partial;
        let result = service
            .analyze_imported(
                "部分失败".into(),
                "真实文件 · partial.csv".into(),
                DataState::RealFile,
                2,
                vec![partial, imported("Missing Fields", "Artist B")],
            )
            .await;
        let summary = result.import_summary.unwrap();

        assert_eq!(result.playlist.tracks.len(), 2);
        assert_eq!(summary.analyzed_count, 2);
        assert_eq!(summary.partial_metadata_count, 1);
        assert_eq!(summary.unmatched_count, 1);
    }

    #[tokio::test]
    async fn different_real_playlists_produce_different_statistics() {
        let mut service = MetadataService::new();
        service.max_tracks = 0;
        let first = service
            .analyze_imported(
                "A".into(),
                "真实批量文本".into(),
                DataState::RealText,
                2,
                vec![
                    imported("One", "Same Artist"),
                    imported("Two", "Same Artist"),
                ],
            )
            .await;
        let second = service
            .analyze_imported(
                "B".into(),
                "真实批量文本".into(),
                DataState::RealText,
                3,
                vec![
                    imported("One", "A"),
                    imported("Two", "B"),
                    imported("Three", "C"),
                ],
            )
            .await;

        assert_ne!(first.report.track_count, second.report.track_count);
        assert_ne!(
            first.report.artist_distribution,
            second.report.artist_distribution
        );
    }

    #[tokio::test]
    async fn real_import_integration_uses_injected_provider_without_demo_fallback() {
        let service =
            MetadataService::with_recommendation_provider(Arc::new(MockRecommendationProvider));
        let mut source = imported("Source Pop Song", "Source Artist");
        source.album = Some("Source Album".into());
        source.release_date = Some("2025".into());
        source.genres = vec!["POP".into()];
        source.duration_ms = Some(180_000);
        source.energy_score = Some(0.7);
        source.metadata_status = MetadataStatus::Complete;
        let result = service
            .analyze_imported(
                "真实 Pop 歌单".into(),
                "真实文件 · pop.csv".into(),
                DataState::RealFile,
                1,
                vec![source],
            )
            .await;

        assert!(!result.playlist.is_demo);
        assert_eq!(result.report.energy_matched_count, 1);
        assert_eq!(result.recommendations.len(), 1);
        assert_eq!(result.recommendations[0].zone, "舒适区");
        assert_eq!(result.recommendation_summary.source_label, "Mock Last.fm");
        assert!(result.recommendations.iter().all(|item| !matches!(
            item.track.title.as_str(),
            "Clair de Lune" | "Blue in Green" | "First Love"
        )));
    }
}
