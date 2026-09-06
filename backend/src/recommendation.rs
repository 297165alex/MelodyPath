use crate::{
    genre::{canonicalize_genre, genre_distance, genre_key, normalize_genres},
    identity::{
        artist_identity_keys, canonical_artist_name, normalized_track_key, primary_artist_key,
        same_recording,
    },
    models::{
        Playlist, Recommendation, RecommendationQueryStats, RecommendationSeed,
        RecommendationSummary, RecommendationZoneSummary, RouteStep, TagCandidateTelemetry,
        TasteReport, Track,
    },
    normalize::{detect_version, is_alternate_version, normalize_text},
};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    fmt::{Display, Formatter},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{Mutex, RwLock, Semaphore},
    task::JoinSet,
    time::{Instant, sleep, timeout},
};
use uuid::Uuid;

const MAX_SEEDS: usize = 10;
const MAX_PER_ARTIST_PER_ZONE: usize = 2;
const TARGET_PER_ZONE: usize = 4;
const LASTFM_TRACK_SIMILAR_LIMIT: usize = 20;
const LASTFM_MAX_REQUESTS: usize = 48;
const LASTFM_MAX_CONCURRENCY: usize = 4;
const LASTFM_REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
const LASTFM_REQUEST_INTERVAL: Duration = Duration::from_millis(80);

#[derive(Debug, Clone)]
pub struct RecommendationProviderError(pub String);

impl Display for RecommendationProviderError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for RecommendationProviderError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CandidateRelation {
    TrackSimilar,
    SimilarArtistTopTrack,
    CoreTagTopTrack,
    AdjacentTagTopTrack,
    DistantTagTopTrack,
    GenreBridgeTopTrack,
    SecondHopSimilarArtist,
}

#[derive(Debug, Clone)]
pub struct RecommendationCandidate {
    pub track: Track,
    pub mbid: Option<String>,
    pub source_url: Option<String>,
    pub source_provider: String,
    pub source_endpoint: String,
    pub seed_track: Option<String>,
    pub seed_artist: Option<String>,
    pub lastfm_similarity: Option<f32>,
    pub tags: Vec<String>,
    pub normalized_genres: Vec<String>,
    pub popularity: Option<f32>,
    pub reason: String,
    pub confidence: f32,
    pub relation: CandidateRelation,
    pub relaxation_level: u8,
}

#[derive(Debug, Clone, Default)]
pub struct RecommendationProviderResult {
    pub candidates: Vec<RecommendationCandidate>,
    pub stats: RecommendationQueryStats,
    pub successful_requests: usize,
    pub failed_requests: usize,
}

#[derive(Debug, Clone)]
pub struct RecommendationProfile {
    pub core_genres: Vec<String>,
    pub adjacent_genres: Vec<String>,
    pub exploration_genres: Vec<String>,
    pub main_artists: Vec<String>,
}

impl RecommendationProfile {
    pub fn from_report(report: &TasteReport) -> Self {
        Self {
            core_genres: report.core_preferences.clone(),
            adjacent_genres: report.adjacent_preferences.clone(),
            exploration_genres: report.unexplored_preferences.clone(),
            main_artists: report
                .artist_distribution
                .iter()
                .take(6)
                .map(|(artist, _)| artist.clone())
                .collect(),
        }
    }
}

#[async_trait]
pub trait RecommendationProvider: Send + Sync {
    fn source_label(&self) -> &str;

    fn is_configured(&self) -> bool {
        true
    }

    async fn generate(
        &self,
        seeds: &[RecommendationSeed],
        profile: &RecommendationProfile,
    ) -> Result<RecommendationProviderResult, RecommendationProviderError>;
}

#[derive(Clone)]
pub struct LastFmRecommendationProvider {
    client: Client,
    api_key: Option<Arc<str>>,
    cache: Arc<RwLock<HashMap<String, Value>>>,
    request_gate: Arc<Mutex<Option<Instant>>>,
    concurrency: Arc<Semaphore>,
}

impl LastFmRecommendationProvider {
    pub fn from_env(client: Client) -> Self {
        let api_key = std::env::var("LASTFM_API_KEY")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .map(Arc::<str>::from);
        Self {
            client,
            api_key,
            cache: Arc::new(RwLock::new(HashMap::new())),
            request_gate: Arc::new(Mutex::new(None)),
            concurrency: Arc::new(Semaphore::new(LASTFM_MAX_CONCURRENCY)),
        }
    }

    #[cfg(test)]
    fn without_key(client: Client) -> Self {
        Self {
            client,
            api_key: None,
            cache: Arc::new(RwLock::new(HashMap::new())),
            request_gate: Arc::new(Mutex::new(None)),
            concurrency: Arc::new(Semaphore::new(LASTFM_MAX_CONCURRENCY)),
        }
    }

    async fn request_json(
        &self,
        method: &str,
        params: Vec<(String, String)>,
        budget: Arc<AtomicUsize>,
        retries: Arc<AtomicUsize>,
    ) -> Result<Value, RecommendationProviderError> {
        let Some(api_key) = self.api_key.as_deref() else {
            return Err(RecommendationProviderError("未配置Last.fm推荐服务".into()));
        };
        let cache_key = format!(
            "{}?{}",
            method.to_ascii_lowercase(),
            params
                .iter()
                .map(|(key, value)| format!("{}={}", key, normalize_text(value)))
                .collect::<Vec<_>>()
                .join("&")
        );
        if let Some(cached) = self.cache.read().await.get(&cache_key).cloned() {
            return Ok(cached);
        }

        // The budget counts logical API requests. A retry belongs to the same request and must
        // not starve the later tag exploration path.
        if budget.fetch_add(1, Ordering::Relaxed) >= LASTFM_MAX_REQUESTS {
            return Err(RecommendationProviderError(
                "已达到本次 Last.fm 请求数量上限".into(),
            ));
        }
        for attempt in 0..=1 {
            let _permit = self
                .concurrency
                .acquire()
                .await
                .map_err(|_| RecommendationProviderError("Last.fm 并发控制不可用".into()))?;
            {
                let mut previous = self.request_gate.lock().await;
                if let Some(last) = *previous {
                    let elapsed = last.elapsed();
                    if elapsed < LASTFM_REQUEST_INTERVAL {
                        sleep(LASTFM_REQUEST_INTERVAL - elapsed).await;
                    }
                }
                *previous = Some(Instant::now());
            }

            let mut query = vec![
                ("method".to_string(), method.to_string()),
                ("api_key".to_string(), api_key.to_string()),
                ("format".to_string(), "json".to_string()),
                ("autocorrect".to_string(), "1".to_string()),
            ];
            query.extend(params.clone());
            let response = timeout(
                LASTFM_REQUEST_TIMEOUT,
                self.client
                    .get("https://ws.audioscrobbler.com/2.0/")
                    .query(&query)
                    .send(),
            )
            .await;
            let response = match response {
                Ok(Ok(response)) => response,
                _ if attempt == 0 => {
                    retries.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                _ => {
                    return Err(RecommendationProviderError(
                        "Last.fm 请求超时或网络不可用".into(),
                    ));
                }
            };
            let status = response.status();
            if !status.is_success() {
                if attempt == 0 && (status.as_u16() == 429 || status.is_server_error()) {
                    retries.fetch_add(1, Ordering::Relaxed);
                    sleep(Duration::from_millis(250)).await;
                    continue;
                }
                return Err(RecommendationProviderError(format!(
                    "Last.fm 返回 HTTP {}",
                    status.as_u16()
                )));
            }
            let body = match response.json::<Value>().await {
                Ok(body) => body,
                Err(_) if attempt == 0 => {
                    retries.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
                Err(_) => {
                    return Err(RecommendationProviderError(
                        "Last.fm 返回了无法解析的响应".into(),
                    ));
                }
            };
            if let Some(code) = body.get("error").and_then(Value::as_u64) {
                return Err(RecommendationProviderError(format!(
                    "Last.fm API 错误 {code}"
                )));
            }
            self.cache.write().await.insert(cache_key, body.clone());
            return Ok(body);
        }
        Err(RecommendationProviderError("Last.fm 请求失败".into()))
    }

    async fn seed_queries(
        &self,
        seed: RecommendationSeed,
        budget: Arc<AtomicUsize>,
        retries: Arc<AtomicUsize>,
    ) -> SeedQueryResult {
        let artist = seed.artists.first().cloned().unwrap_or_default();
        let common = vec![
            ("artist".into(), artist),
            ("track".into(), seed.title.clone()),
        ];
        let mut similar_params = common.clone();
        similar_params.push(("limit".into(), LASTFM_TRACK_SIMILAR_LIMIT.to_string()));
        let similar = self
            .request_json(
                "track.getSimilar",
                similar_params,
                budget.clone(),
                retries.clone(),
            )
            .await;
        let tags = self
            .request_json("track.getTopTags", common, budget, retries)
            .await;
        SeedQueryResult {
            seed,
            similar,
            tags,
        }
    }
}

#[async_trait]
impl RecommendationProvider for LastFmRecommendationProvider {
    fn source_label(&self) -> &str {
        "Last.fm Music Discovery API"
    }

    fn is_configured(&self) -> bool {
        self.api_key.is_some()
    }

    async fn generate(
        &self,
        seeds: &[RecommendationSeed],
        profile: &RecommendationProfile,
    ) -> Result<RecommendationProviderResult, RecommendationProviderError> {
        if !self.is_configured() {
            return Err(RecommendationProviderError("未配置Last.fm推荐服务".into()));
        }
        let budget = Arc::new(AtomicUsize::new(0));
        let retries = Arc::new(AtomicUsize::new(0));
        let mut result = RecommendationProviderResult::default();
        let mut seed_tags = Vec::new();
        let mut seed_tasks = JoinSet::new();
        for seed in seeds.iter().cloned() {
            let provider = self.clone();
            let task_budget = budget.clone();
            let task_retries = retries.clone();
            seed_tasks
                .spawn(async move { provider.seed_queries(seed, task_budget, task_retries).await });
        }
        while let Some(joined) = seed_tasks.join_next().await {
            let Ok(seed_result) = joined else {
                result.failed_requests += 1;
                continue;
            };
            match seed_result.similar {
                Ok(value) => {
                    result.successful_requests += 1;
                    result.stats.successful_seed_count += 1;
                    let candidates = parse_track_similar(&value, &seed_result.seed);
                    result.stats.raw_track_similar_count += candidates.len();
                    result.candidates.extend(candidates);
                }
                Err(_) => {
                    result.failed_requests += 1;
                    result.stats.failed_seed_count += 1;
                }
            }
            match seed_result.tags {
                Ok(value) => {
                    result.successful_requests += 1;
                    seed_tags.extend(parse_tags(&value, "/toptags/tag"));
                }
                Err(_) => result.failed_requests += 1,
            }
        }

        let mut main_artists = Vec::new();
        let mut seen_artists = HashSet::new();
        for artist in profile
            .main_artists
            .iter()
            .chain(seeds.iter().flat_map(|seed| seed.artists.iter()))
        {
            let key = normalize_text(artist);
            if !key.is_empty() && seen_artists.insert(key) {
                main_artists.push(artist.clone());
            }
        }
        main_artists.truncate(4);
        let mut artist_tasks = JoinSet::new();
        for artist in main_artists {
            let provider = self.clone();
            let task_budget = budget.clone();
            let task_retries = retries.clone();
            artist_tasks.spawn(async move {
                let response = provider
                    .request_json(
                        "artist.getSimilar",
                        vec![
                            ("artist".into(), artist.clone()),
                            ("limit".into(), "8".into()),
                        ],
                        task_budget,
                        task_retries,
                    )
                    .await;
                (artist, response)
            });
        }
        let mut similar_artists = Vec::new();
        while let Some(joined) = artist_tasks.join_next().await {
            let Ok((seed_artist, response)) = joined else {
                result.failed_requests += 1;
                continue;
            };
            match response {
                Ok(value) => {
                    result.successful_requests += 1;
                    let parsed = parse_similar_artists(&value, &seed_artist);
                    result.stats.raw_artist_similar_count += parsed.len();
                    similar_artists.extend(parsed);
                }
                Err(_) => result.failed_requests += 1,
            }
        }
        similar_artists.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
        similar_artists
            .dedup_by(|left, right| normalize_text(&left.name) == normalize_text(&right.name));
        similar_artists.truncate(8);

        let mut top_track_tasks = JoinSet::new();
        for similar_artist in similar_artists.iter().cloned() {
            let provider = self.clone();
            let task_budget = budget.clone();
            let task_retries = retries.clone();
            top_track_tasks.spawn(async move {
                let response = provider
                    .request_json(
                        "artist.getTopTracks",
                        vec![
                            ("artist".into(), similar_artist.name.clone()),
                            ("limit".into(), "5".into()),
                        ],
                        task_budget,
                        task_retries,
                    )
                    .await;
                (similar_artist, response)
            });
        }
        while let Some(joined) = top_track_tasks.join_next().await {
            let Ok((similar_artist, response)) = joined else {
                result.failed_requests += 1;
                continue;
            };
            match response {
                Ok(value) => {
                    result.successful_requests += 1;
                    let candidates = parse_artist_top_tracks(
                        &value,
                        &similar_artist,
                        CandidateRelation::SimilarArtistTopTrack,
                    );
                    result.stats.raw_artist_top_tracks_count += candidates.len();
                    result.candidates.extend(candidates);
                }
                Err(_) => result.failed_requests += 1,
            }
        }

        // A second artist hop is a controlled-serendipity source: it is still anchored to a
        // real seed artist, but sufficiently novel to qualify for the surprise pool.
        let first_hop_for_second: Vec<_> = similar_artists.iter().take(2).cloned().collect();
        let mut second_artist_tasks = JoinSet::new();
        for first_hop in first_hop_for_second {
            let provider = self.clone();
            let task_budget = budget.clone();
            let task_retries = retries.clone();
            second_artist_tasks.spawn(async move {
                let response = provider
                    .request_json(
                        "artist.getSimilar",
                        vec![
                            ("artist".into(), first_hop.name.clone()),
                            ("limit".into(), "3".into()),
                        ],
                        task_budget,
                        task_retries,
                    )
                    .await;
                (first_hop, response)
            });
        }
        let mut second_hop_artists = Vec::new();
        while let Some(joined) = second_artist_tasks.join_next().await {
            let Ok((first_hop, response)) = joined else {
                result.failed_requests += 1;
                continue;
            };
            match response {
                Ok(value) => {
                    result.successful_requests += 1;
                    let parsed = parse_similar_artists(&value, &first_hop.name)
                        .into_iter()
                        .map(|mut artist| {
                            artist.seed_artist =
                                format!("{} → {}", first_hop.seed_artist, first_hop.name);
                            artist.similarity *= first_hop.similarity;
                            artist
                        });
                    second_hop_artists.extend(parsed);
                }
                Err(_) => result.failed_requests += 1,
            }
        }
        second_hop_artists.sort_by(|left, right| right.similarity.total_cmp(&left.similarity));
        second_hop_artists
            .dedup_by(|left, right| normalize_text(&left.name) == normalize_text(&right.name));
        second_hop_artists.truncate(3);
        let mut second_top_track_tasks = JoinSet::new();
        for artist in second_hop_artists {
            let provider = self.clone();
            let task_budget = budget.clone();
            let task_retries = retries.clone();
            second_top_track_tasks.spawn(async move {
                let response = provider
                    .request_json(
                        "artist.getTopTracks",
                        vec![
                            ("artist".into(), artist.name.clone()),
                            ("limit".into(), "4".into()),
                        ],
                        task_budget,
                        task_retries,
                    )
                    .await;
                (artist, response)
            });
        }
        while let Some(joined) = second_top_track_tasks.join_next().await {
            let Ok((artist, response)) = joined else {
                result.failed_requests += 1;
                continue;
            };
            match response {
                Ok(value) => {
                    result.successful_requests += 1;
                    let candidates = parse_artist_top_tracks(
                        &value,
                        &artist,
                        CandidateRelation::SecondHopSimilarArtist,
                    );
                    result.stats.raw_artist_top_tracks_count += candidates.len();
                    result.candidates.extend(candidates);
                }
                Err(_) => result.failed_requests += 1,
            }
        }

        let mut core_tags = ranked_profile_tags(profile, seed_tags);
        core_tags.truncate(2);
        result.stats.core_tags = core_tags.clone();
        let mut core_tag_tasks = JoinSet::new();
        for tag in core_tags {
            let provider = self.clone();
            let task_budget = budget.clone();
            let task_retries = retries.clone();
            core_tag_tasks.spawn(async move {
                let top_tracks = provider
                    .request_json(
                        "tag.getTopTracks",
                        vec![("tag".into(), tag.clone()), ("limit".into(), "10".into())],
                        task_budget.clone(),
                        task_retries.clone(),
                    )
                    .await;
                let similar = provider
                    .request_json(
                        "tag.getSimilar",
                        vec![("tag".into(), tag.clone())],
                        task_budget,
                        task_retries,
                    )
                    .await;
                (tag, top_tracks, similar)
            });
        }
        let mut adjacent_tags = Vec::new();
        while let Some(joined) = core_tag_tasks.join_next().await {
            let Ok((root_tag, top_tracks, similar)) = joined else {
                result.failed_requests += 1;
                continue;
            };
            match top_tracks {
                Ok(value) => {
                    result.successful_requests += 1;
                    let candidates = parse_tag_top_tracks(
                        &value,
                        &root_tag,
                        &root_tag,
                        CandidateRelation::CoreTagTopTrack,
                        2,
                    );
                    result.stats.raw_tag_top_tracks_count += candidates.len();
                    result.candidates.extend(candidates);
                }
                Err(_) => result.failed_requests += 1,
            }
            match similar {
                Ok(value) => {
                    result.successful_requests += 1;
                    result.stats.tag_similar_success_count += 1;
                    let parsed = parse_tags(&value, "/similartags/tag");
                    result.stats.similar_tag_count += parsed.len();
                    adjacent_tags.extend(parsed.into_iter().take(2).map(|tag| TagPath {
                        root: root_tag.clone(),
                        previous: root_tag.clone(),
                        tag,
                    }));
                }
                Err(_) => {
                    result.failed_requests += 1;
                    result.stats.tag_similar_failure_count += 1;
                }
            }
        }
        dedupe_tag_paths(&mut adjacent_tags);
        adjacent_tags.truncate(4);
        result.stats.layer1_tags = adjacent_tags.iter().map(|path| path.tag.clone()).collect();

        let mut adjacent_tasks = JoinSet::new();
        for path in adjacent_tags {
            let provider = self.clone();
            let task_budget = budget.clone();
            let task_retries = retries.clone();
            adjacent_tasks.spawn(async move {
                let top_tracks = provider
                    .request_json(
                        "tag.getTopTracks",
                        vec![
                            ("tag".into(), path.tag.clone()),
                            ("limit".into(), "10".into()),
                        ],
                        task_budget.clone(),
                        task_retries.clone(),
                    )
                    .await;
                let similar = provider
                    .request_json(
                        "tag.getSimilar",
                        vec![("tag".into(), path.tag.clone())],
                        task_budget,
                        task_retries,
                    )
                    .await;
                (path, top_tracks, similar)
            });
        }
        let mut distant_tags = Vec::new();
        while let Some(joined) = adjacent_tasks.join_next().await {
            let Ok((path, top_tracks, similar)) = joined else {
                result.failed_requests += 1;
                continue;
            };
            match top_tracks {
                Ok(value) => {
                    result.successful_requests += 1;
                    let candidates = parse_tag_top_tracks(
                        &value,
                        &path.tag,
                        &path.root,
                        CandidateRelation::AdjacentTagTopTrack,
                        2,
                    );
                    result.stats.raw_tag_top_tracks_count += candidates.len();
                    result.stats.tag_layer1_candidate_count += candidates.len();
                    result
                        .stats
                        .tag_top_track_counts
                        .push(TagCandidateTelemetry {
                            tag: path.tag.clone(),
                            layer: 1,
                            candidate_count: candidates.len(),
                            source: "tag.getSimilar depth 1".into(),
                        });
                    result.candidates.extend(candidates);
                }
                Err(_) => result.failed_requests += 1,
            }
            match similar {
                Ok(value) => {
                    result.successful_requests += 1;
                    result.stats.tag_similar_success_count += 1;
                    let parsed = parse_tags(&value, "/similartags/tag");
                    result.stats.similar_tag_count += parsed.len();
                    distant_tags.extend(
                        parsed
                            .into_iter()
                            .filter(|tag| normalize_text(tag) != normalize_text(&path.root))
                            .take(1)
                            .map(|tag| TagPath {
                                root: path.root.clone(),
                                previous: path.tag.clone(),
                                tag,
                            }),
                    );
                }
                Err(_) => {
                    result.failed_requests += 1;
                    result.stats.tag_similar_failure_count += 1;
                }
            }
        }
        dedupe_tag_paths(&mut distant_tags);
        distant_tags.truncate(2);
        let mut relation = CandidateRelation::DistantTagTopTrack;
        if distant_tags.is_empty() {
            relation = CandidateRelation::GenreBridgeTopTrack;
            distant_tags.extend(profile.exploration_genres.iter().take(2).map(|tag| {
                TagPath {
                    root: profile
                        .core_genres
                        .first()
                        .cloned()
                        .unwrap_or_else(|| "core preference".into()),
                    previous: "Genre Graph two-hop bridge".into(),
                    tag: tag.clone(),
                }
            }));
            dedupe_tag_paths(&mut distant_tags);
        }
        result.stats.layer2_tags = distant_tags.iter().map(|path| path.tag.clone()).collect();
        let mut distant_tasks = JoinSet::new();
        for path in distant_tags {
            let provider = self.clone();
            let task_budget = budget.clone();
            let task_retries = retries.clone();
            let task_relation = relation.clone();
            distant_tasks.spawn(async move {
                let response = provider
                    .request_json(
                        "tag.getTopTracks",
                        vec![
                            ("tag".into(), path.tag.clone()),
                            ("limit".into(), "10".into()),
                        ],
                        task_budget,
                        task_retries,
                    )
                    .await;
                (path, task_relation, response)
            });
        }
        while let Some(joined) = distant_tasks.join_next().await {
            let Ok((path, candidate_relation, response)) = joined else {
                result.failed_requests += 1;
                continue;
            };
            match response {
                Ok(value) => {
                    result.successful_requests += 1;
                    let candidates = parse_tag_top_tracks(
                        &value,
                        &path.tag,
                        &format!("{} → {}", path.root, path.previous),
                        candidate_relation.clone(),
                        2,
                    );
                    result.stats.raw_tag_top_tracks_count += candidates.len();
                    result.stats.tag_layer2_candidate_count += candidates.len();
                    result
                        .stats
                        .tag_top_track_counts
                        .push(TagCandidateTelemetry {
                            tag: path.tag.clone(),
                            layer: 2,
                            candidate_count: candidates.len(),
                            source: if candidate_relation == CandidateRelation::GenreBridgeTopTrack
                            {
                                "Genre Graph two-hop → tag.getTopTracks".into()
                            } else {
                                "tag.getSimilar depth 2".into()
                            },
                        });
                    result.candidates.extend(candidates);
                }
                Err(_) => result.failed_requests += 1,
            }
        }
        let requests = budget.load(Ordering::Relaxed);
        result.stats.request_budget_used_count = requests.min(LASTFM_MAX_REQUESTS);
        result.stats.request_budget_exhausted_count = requests.saturating_sub(LASTFM_MAX_REQUESTS);
        result.stats.retry_count = retries.load(Ordering::Relaxed);
        result.stats.genre_bridge_candidate_count = result
            .candidates
            .iter()
            .filter(|candidate| candidate.relation == CandidateRelation::GenreBridgeTopTrack)
            .count();
        result.stats.second_hop_artist_candidate_count = result
            .candidates
            .iter()
            .filter(|candidate| candidate.relation == CandidateRelation::SecondHopSimilarArtist)
            .count();
        result.stats.raw_candidate_count = result.candidates.len();
        Ok(result)
    }
}

pub async fn build_real_recommendations(
    provider: &dyn RecommendationProvider,
    playlist: &Playlist,
    report: &TasteReport,
) -> (Vec<Recommendation>, RecommendationSummary, Vec<RouteStep>) {
    let seeds = select_recommendation_seeds(playlist, report);
    if seeds.is_empty() {
        let summary = empty_summary(
            provider.source_label(),
            "no_valid_seeds",
            "没有可用于相似歌曲查询的真实歌名与歌手。",
            seeds,
            RecommendationQueryStats::default(),
        );
        return (Vec::new(), summary, Vec::new());
    }
    if !provider.is_configured() {
        let summary = empty_summary(
            provider.source_label(),
            "not_configured",
            "未配置Last.fm推荐服务；未调用 Apple 关键词搜索，也未回退 Demo。",
            seeds,
            RecommendationQueryStats::default(),
        );
        return (Vec::new(), summary, Vec::new());
    }

    let profile = RecommendationProfile::from_report(report);
    let provider_result = match provider.generate(&seeds, &profile).await {
        Ok(result) => result,
        Err(error) => {
            let summary = empty_summary(
                provider.source_label(),
                "unavailable",
                &format!("推荐服务暂不可用；未回退 Demo。{error}"),
                seeds,
                RecommendationQueryStats::default(),
            );
            return (Vec::new(), summary, Vec::new());
        }
    };

    let successful_requests = provider_result.successful_requests;
    let failed_requests = provider_result.failed_requests;
    let mut stats = provider_result.stats;
    let raw_candidates = provider_result.candidates;
    stats.raw_candidate_count = raw_candidates.len();
    let version_filtered: Vec<_> = raw_candidates
        .into_iter()
        .filter(|candidate| !is_alternate_version(&detect_version(&candidate.track.title)))
        .collect();
    stats.after_version_filter_count = version_filtered.len();

    let normalized: Vec<_> = version_filtered
        .into_iter()
        .map(|mut candidate| {
            candidate.track.normalized_title = normalize_text(&candidate.track.title);
            candidate.track.version_type = detect_version(&candidate.track.title);
            candidate
        })
        .collect();
    stats.after_normalization_count = normalized.len();

    let mut seen = HashSet::new();
    let mut deduplicated = Vec::new();
    for candidate in normalized {
        let key = normalized_track_key(&candidate.track);
        if seen.insert(key) {
            deduplicated.push(candidate);
        }
    }
    stats.after_deduplication_count = deduplicated.len();

    let filtered: Vec<_> = deduplicated
        .into_iter()
        .filter(|candidate| {
            !playlist
                .tracks
                .iter()
                .any(|source| same_recording(source, &candidate.track))
        })
        .collect();
    stats.after_source_exclusion_count = filtered.len();
    stats.deduplicated_candidate_count = filtered.len();
    let average_energy = average_real_energy(playlist);
    let source_artists: HashSet<_> = playlist
        .tracks
        .iter()
        .flat_map(artist_identity_keys)
        .collect();
    let profile_tags = profile_tag_keys(&profile);
    let seed_rank: HashMap<_, _> = seeds
        .iter()
        .enumerate()
        .map(|(index, seed)| (seed_key(seed), index))
        .collect();

    let mut evaluated: Vec<_> = filtered
        .into_iter()
        .map(|candidate| {
            evaluate_candidate(
                candidate,
                &profile,
                &profile_tags,
                &source_artists,
                &seed_rank,
                average_energy,
            )
        })
        .collect();
    evaluated.sort_by(|left, right| {
        zone_rank(&left.zone)
            .cmp(&zone_rank(&right.zone))
            .then_with(|| left.relaxation_level.cmp(&right.relaxation_level))
            .then_with(|| right.match_score.total_cmp(&left.match_score))
    });
    let artist_limited = apply_artist_cap(evaluated);
    stats.after_artist_cap_count = artist_limited.len();
    let comfort_pool: Vec<_> = artist_limited
        .iter()
        .filter(|item| item.zone == "舒适区")
        .cloned()
        .collect();
    let expansion_pool: Vec<_> = artist_limited
        .iter()
        .filter(|item| item.zone == "拓展区")
        .cloned()
        .collect();
    let surprise_pool: Vec<_> = artist_limited
        .iter()
        .filter(|item| item.zone == "惊喜区")
        .cloned()
        .collect();
    stats.comfort_candidate_count = comfort_pool.len();
    stats.expansion_candidate_count = expansion_pool.len();
    stats.surprise_candidate_count = surprise_pool.len();
    let retained_tag_layer1 = artist_limited
        .iter()
        .filter(|item| {
            item.source_endpoint.contains("tag.getTopTracks")
                && item.source_endpoint.contains("depth 1")
        })
        .count();
    let retained_tag_layer2 = artist_limited
        .iter()
        .filter(|item| {
            item.source_endpoint.contains("tag.getTopTracks")
                && (item.source_endpoint.contains("depth 2")
                    || item.source_endpoint.contains("Genre Graph two-hop"))
        })
        .count();
    stats.tag_layer1_rejected_count = stats
        .tag_layer1_candidate_count
        .saturating_sub(retained_tag_layer1);
    stats.tag_layer2_rejected_count = stats
        .tag_layer2_candidate_count
        .saturating_sub(retained_tag_layer2);
    let recommendations = take_zone_limits(artist_limited);
    let status = if recommendations.is_empty() && successful_requests == 0 {
        "unavailable"
    } else if failed_requests > 0 {
        "partial"
    } else if recommendations.is_empty() {
        "no_candidates"
    } else {
        "ready"
    };
    let message = match status {
        "unavailable" => "Last.fm 请求均失败；未回退 Apple 关键词搜索或 Demo。".into(),
        "partial" => format!(
            "部分 Last.fm 查询失败，仍保留 {} 首可解释候选；未使用其他推荐兜底。",
            recommendations.len()
        ),
        "no_candidates" => "Last.fm 的渐进式候选步骤均已耗尽，没有可安全展示的候选。".into(),
        _ => format!(
            "Last.fm 生成 {} 首原始候选，版本过滤后 {} 首，规范化去重后 {} 首，排除原歌单后 {} 首，艺术家上限后 {} 首，最终展示 {} 首。",
            stats.raw_candidate_count,
            stats.after_version_filter_count,
            stats.after_deduplication_count,
            stats.after_source_exclusion_count,
            stats.after_artist_cap_count,
            recommendations.len()
        ),
    };
    let summary = RecommendationSummary {
        source_label: provider.source_label().into(),
        status: status.into(),
        message,
        candidate_count: stats.deduplicated_candidate_count,
        zones: zone_summaries(&recommendations, status, provider.source_label()),
        seeds,
        query_stats: stats,
        comfort_pool,
        expansion_pool,
        surprise_pool,
    };
    let route = build_route(report, &recommendations);
    (recommendations, summary, route)
}

pub fn select_recommendation_seeds(
    playlist: &Playlist,
    report: &TasteReport,
) -> Vec<RecommendationSeed> {
    let artist_counts: HashMap<_, _> = report
        .artist_distribution
        .iter()
        .map(|(artist, count)| (canonical_artist_name(artist), *count))
        .collect();
    let genre_counts: HashMap<_, _> = report
        .genre_distribution
        .iter()
        .map(|(genre, count)| (genre_key(genre), *count))
        .collect();
    let mut tracks: Vec<_> = playlist
        .tracks
        .iter()
        .filter(|track| {
            !track.title.trim().is_empty()
                && track
                    .artists
                    .first()
                    .is_some_and(|artist| !artist.trim().is_empty())
        })
        .collect();
    tracks.sort_by(|left, right| {
        let score = |track: &Track| {
            let artist = track
                .artists
                .first()
                .map(|artist| canonical_artist_name(artist))
                .unwrap_or_default();
            let artist_score = artist_counts.get(&artist).copied().unwrap_or_default() as f32;
            let genre_score = track
                .genres
                .iter()
                .map(|genre| {
                    genre_counts
                        .get(&genre_key(genre))
                        .copied()
                        .unwrap_or_default()
                })
                .max()
                .unwrap_or_default() as f32;
            artist_score * 2.0 + genre_score + track.metadata_confidence
        };
        score(right)
            .total_cmp(&score(left))
            .then_with(|| left.title.cmp(&right.title))
    });

    let mut selected = Vec::new();
    let mut artist_limits: HashMap<String, usize> = HashMap::new();
    let mut seen_tracks = HashSet::new();
    let mut add_track = |track: &Track| {
        if track.artists.is_empty() {
            return;
        }
        let artist_keys: Vec<_> = track
            .artists
            .iter()
            .map(|artist| canonical_artist_name(artist))
            .filter(|artist| !artist.is_empty())
            .collect();
        let track_key = normalized_track_key(track);
        if selected.len() >= MAX_SEEDS
            || artist_keys
                .iter()
                .any(|artist| artist_limits.get(artist).copied().unwrap_or_default() >= 2)
            || !seen_tracks.insert(track_key)
        {
            return;
        }
        for artist in artist_keys {
            *artist_limits.entry(artist).or_default() += 1;
        }
        selected.push(RecommendationSeed {
            title: track.title.clone(),
            artists: track.artists.clone(),
        });
    };

    for core in &report.core_preferences {
        if let Some(track) = tracks.iter().copied().find(|track| {
            track
                .genres
                .iter()
                .any(|genre| genre_key(genre) == genre_key(core))
        }) {
            add_track(track);
        }
    }
    for main_artist in report.artist_distribution.iter().map(|(artist, _)| artist) {
        if let Some(track) = tracks.iter().copied().find(|track| {
            track
                .artists
                .iter()
                .any(|artist| canonical_artist_name(artist) == canonical_artist_name(main_artist))
        }) {
            add_track(track);
        }
    }
    for track in tracks {
        add_track(track);
    }
    selected
}

pub fn build_route(report: &TasteReport, recommendations: &[Recommendation]) -> Vec<RouteStep> {
    let mut route = Vec::new();
    let mut used = HashSet::new();
    for (zone, preferred, explanation) in [
        (
            "舒适区",
            report.core_preferences.as_slice(),
            "从真实歌单的核心偏好与高相似歌曲出发",
        ),
        (
            "拓展区",
            report.adjacent_preferences.as_slice(),
            "沿相似艺术家和第一层相邻标签增加探索距离",
        ),
        (
            "惊喜区",
            report.unexplored_preferences.as_slice(),
            "沿第二层关联标签进入更远但仍可解释的声音",
        ),
    ] {
        let zone_items: Vec<_> = recommendations
            .iter()
            .filter(|item| item.zone == zone)
            .collect();
        if zone_items.is_empty() {
            continue;
        }
        let genre = preferred
            .iter()
            .map(|genre| canonicalize_genre(genre))
            .chain(
                zone_items
                    .iter()
                    .flat_map(|item| item.track.genres.iter())
                    .map(|genre| canonicalize_genre(genre)),
            )
            .find(|genre| !genre.is_empty() && used.insert(genre_key(genre)));
        let Some(genre) = genre else { continue };
        route.push(RouteStep {
            genre,
            explanation: explanation.into(),
            tracks: zone_items
                .into_iter()
                .take(3)
                .map(|item| item.track.clone())
                .collect(),
        });
    }
    route
}

fn evaluate_candidate(
    candidate: RecommendationCandidate,
    profile: &RecommendationProfile,
    profile_tags: &HashSet<String>,
    source_artists: &HashSet<String>,
    seed_rank: &HashMap<String, usize>,
    average_energy: Option<f32>,
) -> Recommendation {
    let zone = match candidate.relation {
        CandidateRelation::TrackSimilar | CandidateRelation::CoreTagTopTrack => "舒适区",
        CandidateRelation::SimilarArtistTopTrack | CandidateRelation::AdjacentTagTopTrack => {
            "拓展区"
        }
        CandidateRelation::DistantTagTopTrack
        | CandidateRelation::GenreBridgeTopTrack
        | CandidateRelation::SecondHopSimilarArtist => "惊喜区",
    };
    let overlap = candidate
        .tags
        .iter()
        .filter(|tag| profile_tags.contains(&genre_key(tag)))
        .count();
    let genre_distance_score = candidate
        .normalized_genres
        .iter()
        .flat_map(|genre| {
            profile
                .core_genres
                .iter()
                .map(move |core| genre_distance(core, genre))
        })
        .min();
    let candidate_artists = artist_identity_keys(&candidate.track);
    let artist_is_new = !candidate_artists
        .iter()
        .any(|artist| source_artists.contains(artist));
    let seed_position = RecommendationSeed {
        title: candidate.seed_track.clone().unwrap_or_default(),
        artists: candidate
            .seed_artist
            .clone()
            .into_iter()
            .collect::<Vec<_>>(),
    };
    let representative = seed_rank
        .get(&seed_key(&seed_position))
        .map(|index| 1.0 - (*index as f32 / MAX_SEEDS as f32))
        .unwrap_or(0.5);
    let similarity = candidate.lastfm_similarity.unwrap_or(0.0).clamp(0.0, 1.0);
    let tag_score = (overlap as f32 / 3.0).min(1.0);
    let genre_score = match genre_distance_score {
        Some(0) => 1.0,
        Some(1) => 0.8,
        Some(2) => 0.55,
        Some(_) => 0.2,
        None => 0.0,
    };
    let relation_score = match candidate.relation {
        CandidateRelation::TrackSimilar => similarity,
        CandidateRelation::SimilarArtistTopTrack => similarity.max(0.55),
        CandidateRelation::CoreTagTopTrack => 0.58,
        CandidateRelation::AdjacentTagTopTrack => 0.64,
        CandidateRelation::DistantTagTopTrack => 0.62,
        CandidateRelation::GenreBridgeTopTrack => 0.6,
        CandidateRelation::SecondHopSimilarArtist => similarity.max(0.48),
    };
    let novelty = if artist_is_new { 1.0 } else { 0.25 };
    let mut score = match zone {
        "舒适区" => {
            relation_score * 0.48 + tag_score * 0.18 + genre_score * 0.14 + representative * 0.2
        }
        "拓展区" => {
            relation_score * 0.38 + tag_score * 0.18 + genre_score * 0.18 + novelty * 0.26
        }
        _ => relation_score * 0.34 + tag_score * 0.2 + genre_score * 0.18 + novelty * 0.28,
    };
    if let (Some(user), Some(candidate_energy)) = (average_energy, candidate.track.energy_score) {
        score += (1.0 - (user - candidate_energy).abs()).max(0.0) * 0.05;
    }
    let connection = match candidate.relation {
        CandidateRelation::TrackSimilar => format!(
            "Last.fm 听众相似关系 {:.0}%{}",
            similarity * 100.0,
            candidate
                .seed_track
                .as_deref()
                .map(|seed| format!("，来自种子《{seed}》"))
                .unwrap_or_default()
        ),
        CandidateRelation::SimilarArtistTopTrack => format!(
            "相似艺术家网络，起点为 {}",
            candidate.seed_artist.as_deref().unwrap_or("歌单主要艺术家")
        ),
        CandidateRelation::CoreTagTopTrack => "用户核心标签的真实热门曲目".into(),
        CandidateRelation::AdjacentTagTopTrack => "核心标签的第一层相邻标签曲目".into(),
        CandidateRelation::DistantTagTopTrack => "核心标签的第二层相邻标签曲目".into(),
        CandidateRelation::GenreBridgeTopTrack => format!(
            "Genre Graph 两跳桥梁，仍连接核心偏好 {}",
            candidate.seed_artist.as_deref().unwrap_or("核心 Genre")
        ),
        CandidateRelation::SecondHopSimilarArtist => format!(
            "第二层相似艺术家网络，桥梁路径为 {}",
            candidate.seed_artist.as_deref().unwrap_or("歌单主要艺术家")
        ),
    };
    let expansion = format!(
        "Last.fm {} · 渐进式第 {} 级",
        candidate.source_endpoint, candidate.relaxation_level
    );
    let mut track = candidate.track;
    if track.platform_url.is_none() {
        track.platform_url = candidate.source_url.clone();
    }
    if track.popularity.is_none() {
        track.popularity = candidate.popularity;
    }
    track.genres = candidate.normalized_genres.clone();
    track.mood_tags = candidate.tags.clone();
    let ui_confidence = (candidate.confidence * 0.72 + score * 0.28).clamp(0.0, 0.99);
    Recommendation {
        track,
        zone: zone.into(),
        reason: candidate.reason,
        connection,
        expansion,
        match_score: score.clamp(0.0, 0.98),
        novelty_score: match zone {
            "舒适区" => 0.3,
            "拓展区" => 0.66,
            _ => 0.88,
        },
        candidate_source: candidate.source_provider,
        match_confidence: ui_confidence,
        source_endpoint: candidate.source_endpoint,
        seed_track: candidate.seed_track,
        seed_artist: candidate.seed_artist,
        lastfm_similarity: candidate.lastfm_similarity,
        tags: candidate.tags,
        relaxation_level: candidate.relaxation_level,
        already_in_source_playlist: false,
    }
}

fn apply_artist_cap(items: Vec<Recommendation>) -> Vec<Recommendation> {
    let mut result: Vec<Recommendation> = Vec::new();
    let mut used = HashSet::new();
    for zone in ["舒适区", "拓展区", "惊喜区"] {
        let mut artist_counts: HashMap<String, usize> = HashMap::new();
        for item in items.iter().filter(|item| item.zone == zone) {
            let key = recommendation_identity(item);
            if !used.insert(key) {
                continue;
            }
            let artist = primary_artist_key(&item.track);
            if artist_counts.get(&artist).copied().unwrap_or_default() >= MAX_PER_ARTIST_PER_ZONE {
                continue;
            }
            *artist_counts.entry(artist).or_default() += 1;
            result.push(item.clone());
        }
    }
    result
}

fn take_zone_limits(items: Vec<Recommendation>) -> Vec<Recommendation> {
    let mut result = Vec::new();
    for zone in ["舒适区", "拓展区", "惊喜区"] {
        result.extend(
            items
                .iter()
                .filter(|item| item.zone == zone)
                .take(TARGET_PER_ZONE)
                .cloned(),
        );
    }
    result
}

#[derive(Debug, Clone)]
pub struct RecommendationBatch {
    pub items: Vec<Recommendation>,
    pub exhausted: bool,
}

/// Returns the next non-overlapping page from an already-fetched pool. It never calls a provider.
pub fn next_recommendation_batch(
    pool: &[Recommendation],
    already_shown: &[String],
    batch_size: usize,
) -> RecommendationBatch {
    let shown: HashSet<_> = already_shown.iter().cloned().collect();
    let items: Vec<_> = pool
        .iter()
        .filter(|item| !shown.contains(&recommendation_identity(item)))
        .take(batch_size)
        .cloned()
        .collect();
    let returned: HashSet<_> = items.iter().map(recommendation_identity).collect();
    let exhausted = pool.iter().all(|item| {
        let id = recommendation_identity(item);
        shown.contains(&id) || returned.contains(&id)
    });
    RecommendationBatch { items, exhausted }
}

fn zone_summaries(
    recommendations: &[Recommendation],
    status: &str,
    source: &str,
) -> Vec<RecommendationZoneSummary> {
    ["舒适区", "拓展区", "惊喜区"]
        .into_iter()
        .map(|zone| {
            let count = recommendations
                .iter()
                .filter(|item| item.zone == zone)
                .count();
            let message = if count >= TARGET_PER_ZONE {
                format!("{count} 首真实候选 · {source}")
            } else if count > 0 {
                format!(
                    "仅找到 {count}/{TARGET_PER_ZONE} 首；所有 Last.fm 渐进候选经去重、原歌单排除和艺术家上限后已耗尽"
                )
            } else if status == "not_configured" {
                "未配置Last.fm推荐服务".into()
            } else if status == "unavailable" {
                "Last.fm 请求失败，未回退 Apple 或 Demo".into()
            } else {
                match zone {
                    "舒适区" => "track.getSimilar、共享标签和相似艺术家代表曲均无可用结果".into(),
                    "拓展区" => "artist.getSimilar、artist.getTopTracks 和第一层相邻标签候选已耗尽".into(),
                    _ => "第二层关联标签和较远相似艺术家网络候选已耗尽".into(),
                }
            };
            RecommendationZoneSummary {
                zone: zone.into(),
                count,
                message,
            }
        })
        .collect()
}

fn empty_summary(
    source: &str,
    status: &str,
    message: &str,
    seeds: Vec<RecommendationSeed>,
    query_stats: RecommendationQueryStats,
) -> RecommendationSummary {
    RecommendationSummary {
        source_label: source.into(),
        status: status.into(),
        message: message.into(),
        candidate_count: 0,
        zones: zone_summaries(&[], status, source),
        seeds,
        query_stats,
        comfort_pool: Vec::new(),
        expansion_pool: Vec::new(),
        surprise_pool: Vec::new(),
    }
}

fn average_real_energy(playlist: &Playlist) -> Option<f32> {
    let energies: Vec<_> = playlist
        .tracks
        .iter()
        .filter_map(|track| track.energy_score)
        .collect();
    (!energies.is_empty()).then(|| energies.iter().sum::<f32>() / energies.len() as f32)
}

fn profile_tag_keys(profile: &RecommendationProfile) -> HashSet<String> {
    profile
        .core_genres
        .iter()
        .chain(profile.adjacent_genres.iter())
        .chain(profile.exploration_genres.iter())
        .map(|genre| genre_key(genre))
        .collect()
}

fn ranked_profile_tags(profile: &RecommendationProfile, seed_tags: Vec<String>) -> Vec<String> {
    let mut counts: HashMap<String, (String, usize)> = HashMap::new();
    for tag in seed_tags
        .into_iter()
        .chain(profile.core_genres.iter().cloned())
    {
        let key = genre_key(&tag);
        if key.is_empty() {
            continue;
        }
        let entry = counts.entry(key).or_insert((tag, 0));
        entry.1 += 1;
    }
    let mut ranked: Vec<_> = counts.into_values().collect();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    ranked.into_iter().map(|(tag, _)| tag).collect()
}

fn parse_track_similar(value: &Value, seed: &RecommendationSeed) -> Vec<RecommendationCandidate> {
    value
        .pointer("/similartracks/track")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let similarity = parse_number(item.get("match"))
                .map(normalize_similarity)
                .unwrap_or(0.0);
            let level = if similarity >= 0.5 {
                1
            } else if similarity >= 0.25 {
                2
            } else {
                3
            };
            candidate_from_track_value(
                item,
                Vec::new(),
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                seed.artists.first().cloned(),
                Some(seed.title.clone()),
                Some(similarity),
                format!("与种子《{}》具有 Last.fm 听众相似关系", seed.title),
                similarity.max(0.35),
                level,
            )
        })
        .collect()
}

#[derive(Debug, Clone)]
struct SimilarArtist {
    name: String,
    seed_artist: String,
    similarity: f32,
}

fn parse_similar_artists(value: &Value, seed_artist: &str) -> Vec<SimilarArtist> {
    value
        .pointer("/similarartists/artist")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?.trim().to_string();
            (!name.is_empty()).then(|| SimilarArtist {
                name,
                seed_artist: seed_artist.into(),
                similarity: parse_number(item.get("match"))
                    .map(normalize_similarity)
                    .unwrap_or(0.5),
            })
        })
        .collect()
}

fn parse_artist_top_tracks(
    value: &Value,
    similar_artist: &SimilarArtist,
    relation: CandidateRelation,
) -> Vec<RecommendationCandidate> {
    value
        .pointer("/toptracks/track")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            candidate_from_track_value(
                item,
                Vec::new(),
                relation.clone(),
                if relation == CandidateRelation::SecondHopSimilarArtist {
                    "artist.getSimilar depth 2 → artist.getTopTracks"
                } else {
                    "artist.getSimilar → artist.getTopTracks"
                },
                Some(similar_artist.seed_artist.clone()),
                None,
                Some(similar_artist.similarity),
                format!(
                    "{} 是 {} 的 Last.fm 相似艺术家网络代表曲",
                    similar_artist.name, similar_artist.seed_artist
                ),
                similar_artist.similarity.max(0.5),
                if similar_artist.similarity >= 0.5 {
                    1
                } else {
                    2
                },
            )
        })
        .collect()
}

fn parse_tag_top_tracks(
    value: &Value,
    tag: &str,
    connection: &str,
    relation: CandidateRelation,
    relaxation_level: u8,
) -> Vec<RecommendationCandidate> {
    value
        .pointer("/tracks/track")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            candidate_from_track_value(
                item,
                vec![tag.to_string()],
                relation.clone(),
                match relation {
                    CandidateRelation::CoreTagTopTrack => "tag.getTopTracks (core tag)",
                    CandidateRelation::AdjacentTagTopTrack => {
                        "tag.getSimilar depth 1 → tag.getTopTracks"
                    }
                    CandidateRelation::GenreBridgeTopTrack => {
                        "Genre Graph two-hop → tag.getTopTracks"
                    }
                    _ => "tag.getSimilar depth 2 → tag.getTopTracks",
                },
                Some(connection.to_string()),
                None,
                None,
                format!("Last.fm 标签路径 {connection} → {tag} 的真实热门曲目"),
                match relation {
                    CandidateRelation::CoreTagTopTrack => 0.58,
                    CandidateRelation::AdjacentTagTopTrack => 0.64,
                    _ => 0.6,
                },
                relaxation_level,
            )
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn candidate_from_track_value(
    item: &Value,
    tags: Vec<String>,
    relation: CandidateRelation,
    endpoint: &str,
    seed_artist: Option<String>,
    seed_track: Option<String>,
    similarity: Option<f32>,
    reason: String,
    confidence: f32,
    relaxation_level: u8,
) -> Option<RecommendationCandidate> {
    let title = item.get("name")?.as_str()?.trim().to_string();
    let artist = item
        .pointer("/artist/name")
        .or_else(|| item.pointer("/artist/#text"))
        .or_else(|| item.get("artist"))?
        .as_str()?
        .trim()
        .to_string();
    if title.is_empty() || artist.is_empty() {
        return None;
    }
    let mbid = item
        .get("mbid")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let source_url = item
        .get("url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| value.starts_with("https://") || value.starts_with("http://"))
        .map(str::to_string);
    let normalized_genres = normalize_genres(tags.iter());
    let popularity = parse_number(item.get("listeners").or_else(|| item.get("playcount")))
        .map(|value| ((value + 1.0).ln() / 16.0).clamp(0.0, 1.0));
    let mut external_ids = HashMap::new();
    if let Some(mbid) = &mbid {
        external_ids.insert("mbid".into(), mbid.clone());
    }
    Some(RecommendationCandidate {
        track: Track {
            id: Uuid::new_v4().to_string(),
            title: title.clone(),
            normalized_title: normalize_text(&title),
            artists: vec![artist],
            album: None,
            genres: normalized_genres.clone(),
            release_year: None,
            language: None,
            duration_ms: None,
            platform: "lastfm".into(),
            platform_url: source_url.clone(),
            external_ids,
            version_type: detect_version(&title),
            mood_tags: tags.clone(),
            energy_score: None,
            popularity,
            metadata_confidence: confidence,
        },
        mbid,
        source_url,
        source_provider: "Last.fm Music Discovery API".into(),
        source_endpoint: endpoint.into(),
        seed_track,
        seed_artist,
        lastfm_similarity: similarity,
        tags,
        normalized_genres,
        popularity,
        reason,
        confidence,
        relation,
        relaxation_level,
    })
}

fn parse_tags(value: &Value, pointer: &str) -> Vec<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| item.get("name").and_then(Value::as_str))
        .map(str::trim)
        .filter(|tag| !tag.is_empty())
        .take(8)
        .map(str::to_string)
        .collect()
}

fn parse_number(value: Option<&Value>) -> Option<f32> {
    value.and_then(|value| {
        value
            .as_f64()
            .map(|number| number as f32)
            .or_else(|| value.as_str().and_then(|number| number.parse().ok()))
    })
}

fn normalize_similarity(value: f32) -> f32 {
    if value > 1.0 {
        (value / 100.0).clamp(0.0, 1.0)
    } else {
        value.clamp(0.0, 1.0)
    }
}

#[derive(Debug)]
struct SeedQueryResult {
    seed: RecommendationSeed,
    similar: Result<Value, RecommendationProviderError>,
    tags: Result<Value, RecommendationProviderError>,
}

#[derive(Debug, Clone)]
struct TagPath {
    root: String,
    previous: String,
    tag: String,
}

fn dedupe_tag_paths(paths: &mut Vec<TagPath>) {
    let mut seen = HashSet::new();
    paths.retain(|path| seen.insert(normalize_text(&path.tag)));
}

fn seed_key(seed: &RecommendationSeed) -> String {
    format!(
        "{}|{}",
        normalize_text(&seed.title),
        canonical_artist_name(seed.artists.first().map(String::as_str).unwrap_or_default())
    )
}

fn recommendation_identity(item: &Recommendation) -> String {
    normalized_track_key(&item.track)
}

fn zone_rank(zone: &str) -> u8 {
    match zone {
        "舒适区" => 0,
        "拓展区" => 1,
        _ => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine;

    #[derive(Clone)]
    struct MockLastFmProvider {
        configured: bool,
        fail: bool,
        result: RecommendationProviderResult,
    }

    #[async_trait]
    impl RecommendationProvider for MockLastFmProvider {
        fn source_label(&self) -> &str {
            "Mock Last.fm"
        }

        fn is_configured(&self) -> bool {
            self.configured
        }

        async fn generate(
            &self,
            _seeds: &[RecommendationSeed],
            _profile: &RecommendationProfile,
        ) -> Result<RecommendationProviderResult, RecommendationProviderError> {
            if self.fail {
                Err(RecommendationProviderError("mock Last.fm failure".into()))
            } else {
                Ok(self.result.clone())
            }
        }
    }

    fn track(title: &str, artist: &str, genre: &str, energy: Option<f32>) -> Track {
        Track {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            normalized_title: normalize_text(title),
            artists: vec![artist.into()],
            album: None,
            genres: normalize_genres([genre]),
            release_year: None,
            language: None,
            duration_ms: None,
            platform: "test".into(),
            platform_url: None,
            external_ids: HashMap::new(),
            version_type: detect_version(title),
            mood_tags: Vec::new(),
            energy_score: energy,
            popularity: None,
            metadata_confidence: 1.0,
        }
    }

    fn playlist() -> Playlist {
        Playlist {
            id: "real".into(),
            name: "Real pop".into(),
            owner_label: "test".into(),
            source: "REAL_FILE".into(),
            is_demo: false,
            tracks: vec![
                track("Run Away With Me", "Carly Rae Jepsen", "Pop", Some(0.72)),
                track("Levitating", "Dua Lipa", "Pop", Some(0.82)),
                track("Somebody Else", "The 1975", "Bedroom Pop", None),
            ],
        }
    }

    fn candidate(
        title: &str,
        artist: &str,
        relation: CandidateRelation,
        endpoint: &str,
        similarity: Option<f32>,
        tags: &[&str],
    ) -> RecommendationCandidate {
        let normalized = normalize_genres(tags.iter());
        RecommendationCandidate {
            track: track(
                title,
                artist,
                normalized.first().map(String::as_str).unwrap_or(""),
                None,
            ),
            mbid: None,
            source_url: Some(format!("https://www.last.fm/music/{artist}/_/{title}")),
            source_provider: "Mock Last.fm".into(),
            source_endpoint: endpoint.into(),
            seed_track: Some("Run Away With Me".into()),
            seed_artist: Some("Carly Rae Jepsen".into()),
            lastfm_similarity: similarity,
            tags: tags.iter().map(|tag| (*tag).into()).collect(),
            normalized_genres: normalized,
            popularity: None,
            reason: "mock listening relationship".into(),
            confidence: similarity.unwrap_or(0.65),
            relation,
            relaxation_level: 1,
        }
    }

    fn provider(candidates: Vec<RecommendationCandidate>) -> MockLastFmProvider {
        let tag_layer1_candidate_count = candidates
            .iter()
            .filter(|candidate| candidate.relation == CandidateRelation::AdjacentTagTopTrack)
            .count();
        let tag_layer2_candidate_count = candidates
            .iter()
            .filter(|candidate| {
                matches!(
                    candidate.relation,
                    CandidateRelation::DistantTagTopTrack | CandidateRelation::GenreBridgeTopTrack
                )
            })
            .count();
        MockLastFmProvider {
            configured: true,
            fail: false,
            result: RecommendationProviderResult {
                stats: RecommendationQueryStats {
                    successful_seed_count: 3,
                    raw_track_similar_count: candidates
                        .iter()
                        .filter(|candidate| candidate.relation == CandidateRelation::TrackSimilar)
                        .count(),
                    raw_artist_similar_count: candidates
                        .iter()
                        .filter(|candidate| {
                            candidate.relation == CandidateRelation::SimilarArtistTopTrack
                        })
                        .count(),
                    raw_tag_top_tracks_count: candidates
                        .iter()
                        .filter(|candidate| {
                            matches!(
                                candidate.relation,
                                CandidateRelation::CoreTagTopTrack
                                    | CandidateRelation::AdjacentTagTopTrack
                                    | CandidateRelation::DistantTagTopTrack
                            )
                        })
                        .count(),
                    raw_candidate_count: candidates.len(),
                    tag_layer1_candidate_count,
                    tag_layer2_candidate_count,
                    ..Default::default()
                },
                candidates,
                successful_requests: 3,
                failed_requests: 0,
            },
        }
    }

    #[test]
    fn parses_track_get_similar_and_preserves_similarity() {
        let value = serde_json::json!({"similartracks":{"track":[{"name":"Cut to the Feeling","match":"0.87","url":"https://www.last.fm/music/Carly+Rae+Jepsen/_/Cut+to+the+Feeling","artist":{"name":"Carly Rae Jepsen"}}]}});
        let parsed = parse_track_similar(
            &value,
            &RecommendationSeed {
                title: "Run Away With Me".into(),
                artists: vec!["Carly Rae Jepsen".into()],
            },
        );
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].lastfm_similarity, Some(0.87));
        assert_eq!(parsed[0].source_endpoint, "track.getSimilar");
    }

    #[tokio::test]
    async fn track_similar_enters_comfort_without_requiring_genre() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let provider = provider(vec![candidate(
            "Cut to the Feeling",
            "Carly Rae Jepsen",
            CandidateRelation::TrackSimilar,
            "track.getSimilar",
            Some(0.83),
            &[],
        )]);
        let (items, _, _) = build_real_recommendations(&provider, &playlist, &report).await;
        assert_eq!(items[0].zone, "舒适区");
        assert!(items[0].track.genres.is_empty());
    }

    #[tokio::test]
    async fn empty_track_similar_can_fall_back_to_similar_artist_top_tracks() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let provider = provider(vec![candidate(
            "Want You in My Room",
            "Carly Rae Jepsen",
            CandidateRelation::SimilarArtistTopTrack,
            "artist.getSimilar → artist.getTopTracks",
            Some(0.71),
            &["Pop"],
        )]);
        let (items, summary, _) = build_real_recommendations(&provider, &playlist, &report).await;
        assert_eq!(summary.query_stats.raw_track_similar_count, 0);
        assert_eq!(items[0].zone, "拓展区");
    }

    #[tokio::test]
    async fn distant_connected_tag_enters_surprise() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let provider = provider(vec![candidate(
            "Archie, Marry Me",
            "Alvvays",
            CandidateRelation::DistantTagTopTrack,
            "tag.getSimilar depth 2 → tag.getTopTracks",
            None,
            &["Dream Pop"],
        )]);
        let (items, _, _) = build_real_recommendations(&provider, &playlist, &report).await;
        assert_eq!(items[0].zone, "惊喜区");
    }

    #[tokio::test]
    async fn genre_graph_bridge_fallback_enters_surprise_without_random_data() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let provider = provider(vec![candidate(
            "Sweet Life",
            "Frank Ocean",
            CandidateRelation::GenreBridgeTopTrack,
            "Genre Graph two-hop → tag.getTopTracks",
            None,
            &["Neo Soul"],
        )]);
        let (items, summary, _) = build_real_recommendations(&provider, &playlist, &report).await;
        assert_eq!(items[0].zone, "惊喜区");
        assert!(items[0].source_endpoint.contains("Genre Graph"));
        assert!(!summary.source_label.to_lowercase().contains("demo"));
    }

    #[tokio::test]
    async fn second_hop_artist_is_controlled_surprise() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let provider = provider(vec![candidate(
            "Garden Song",
            "Phoebe Bridgers",
            CandidateRelation::SecondHopSimilarArtist,
            "artist.getSimilar depth 2 → artist.getTopTracks",
            Some(0.48),
            &["Singer/Songwriter"],
        )]);
        let (items, _, _) = build_real_recommendations(&provider, &playlist, &report).await;
        assert_eq!(items[0].zone, "惊喜区");
        assert!(items[0].connection.contains("第二层相似艺术家"));
    }

    #[tokio::test]
    async fn source_tracks_duplicates_and_versions_are_excluded() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let provider = provider(vec![
            candidate(
                "Levitating",
                "Dua Lipa",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.9),
                &["Pop"],
            ),
            candidate(
                "Cut to the Feeling",
                "Carly Rae Jepsen",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.8),
                &["Pop"],
            ),
            candidate(
                "Cut to the Feeling - Live",
                "Carly Rae Jepsen",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.7),
                &["Pop"],
            ),
        ]);
        let (items, summary, _) = build_real_recommendations(&provider, &playlist, &report).await;
        assert_eq!(items.len(), 1);
        assert_eq!(summary.query_stats.deduplicated_candidate_count, 1);
    }

    #[tokio::test]
    async fn same_artist_is_limited_to_two_per_zone() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let candidates = ["Emotion", "Julien", "Surrender", "Boy Problems"]
            .into_iter()
            .map(|title| {
                candidate(
                    title,
                    "Carly Rae Jepsen",
                    CandidateRelation::TrackSimilar,
                    "track.getSimilar",
                    Some(0.8),
                    &["Pop"],
                )
            })
            .collect();
        let (items, _, _) =
            build_real_recommendations(&provider(candidates), &playlist, &report).await;
        assert_eq!(items.len(), 2);
    }

    #[tokio::test]
    async fn provider_failure_never_falls_back_to_demo() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let provider = MockLastFmProvider {
            configured: true,
            fail: true,
            result: Default::default(),
        };
        let (items, summary, _) = build_real_recommendations(&provider, &playlist, &report).await;
        assert!(items.is_empty());
        assert_eq!(summary.status, "unavailable");
        assert!(!summary.message.contains("Apple 关键词搜索生成"));
    }

    #[tokio::test]
    async fn missing_lastfm_key_has_explicit_status_and_no_network_fallback() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let provider = LastFmRecommendationProvider::without_key(Client::new());
        let (items, summary, _) = build_real_recommendations(&provider, &playlist, &report).await;
        assert!(items.is_empty());
        assert_eq!(summary.status, "not_configured");
        assert!(summary.message.contains("未配置Last.fm推荐服务"));
    }

    #[test]
    fn seed_selection_uses_real_tracks_covers_genres_and_limits_artists() {
        let mut playlist = playlist();
        playlist.tracks.extend([
            track("Emotion", "Carly Rae Jepsen", "Pop", None),
            track("Julien", "Carly Rae Jepsen", "Pop", None),
            track("Super Shy", "NewJeans", "K-Pop", None),
            track("Birds of a Feather", "Billie Eilish", "Bedroom Pop", None),
        ]);
        let report = engine::analyze_playlist(&playlist);
        let seeds = select_recommendation_seeds(&playlist, &report);
        let carly = seeds
            .iter()
            .filter(|seed| {
                seed.artists
                    .first()
                    .is_some_and(|artist| artist == "Carly Rae Jepsen")
            })
            .count();
        assert!(seeds.len() <= 10);
        assert!(carly <= 2);
        assert!(seeds.iter().any(|seed| seed.title == "Super Shy"));
    }

    #[tokio::test]
    async fn one_candidate_cannot_appear_in_multiple_zones_and_route_has_no_duplicate_genres() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let duplicate = candidate(
            "About You",
            "The 1975",
            CandidateRelation::TrackSimilar,
            "track.getSimilar",
            Some(0.8),
            &["Bedroom Pop"],
        );
        let mut expansion = duplicate.clone();
        expansion.relation = CandidateRelation::SimilarArtistTopTrack;
        let (items, _, route) =
            build_real_recommendations(&provider(vec![duplicate, expansion]), &playlist, &report)
                .await;
        assert_eq!(items.len(), 1);
        let keys: HashSet<_> = route.iter().map(|step| genre_key(&step.genre)).collect();
        assert_eq!(keys.len(), route.len());
    }

    #[tokio::test]
    async fn missing_energy_is_never_invented_or_treated_as_zero() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let provider = provider(vec![candidate(
            "Physical",
            "Dua Lipa",
            CandidateRelation::TrackSimilar,
            "track.getSimilar",
            Some(0.7),
            &["Pop"],
        )]);
        let (items, _, _) = build_real_recommendations(&provider, &playlist, &report).await;
        assert_eq!(items[0].track.energy_score, None);
        assert!(items[0].match_score > 0.0);
    }

    #[tokio::test]
    async fn source_original_excludes_known_derived_versions() {
        let playlist = Playlist {
            id: "versions".into(),
            name: "Version sources".into(),
            owner_label: "test".into(),
            source: "REAL_FILE".into(),
            is_demo: false,
            tracks: vec![
                track("Dancin", "Aaron Smith", "Pop", None),
                track("Bang Bang", "Jessie J", "Pop", None),
            ],
        };
        let report = engine::analyze_playlist(&playlist);
        let provider = provider(vec![
            candidate(
                "Dancin - Krono Remix",
                "Aaron Smith",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.92),
                &["Pop"],
            ),
            candidate(
                "Bang Bang (Bonus Track)",
                "Jessie J",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.91),
                &["Pop"],
            ),
        ]);
        let (items, summary, _) = build_real_recommendations(&provider, &playlist, &report).await;
        assert!(items.is_empty());
        assert_eq!(summary.query_stats.raw_candidate_count, 2);
        assert_eq!(summary.query_stats.after_version_filter_count, 0);
    }

    #[tokio::test]
    async fn explicit_remix_source_keeps_original_version_semantics_distinct() {
        let playlist = Playlist {
            id: "remix-source".into(),
            name: "Remix source".into(),
            owner_label: "test".into(),
            source: "REAL_FILE".into(),
            is_demo: false,
            tracks: vec![track("Dancin - Krono Remix", "Aaron Smith", "Pop", None)],
        };
        let report = engine::analyze_playlist(&playlist);
        let provider = provider(vec![candidate(
            "Dancin",
            "Aaron Smith",
            CandidateRelation::TrackSimilar,
            "track.getSimilar",
            Some(0.9),
            &["Pop"],
        )]);
        let (items, _, _) = build_real_recommendations(&provider, &playlist, &report).await;
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].track.title, "Dancin");
    }

    #[tokio::test]
    async fn source_aliases_are_excluded_from_real_recommendations() {
        let playlist = Playlist {
            id: "aliases".into(),
            name: "Alias sources".into(),
            owner_label: "test".into(),
            source: "REAL_FILE".into(),
            is_demo: false,
            tracks: vec![
                track("晴天", "Jay Chou", "Mandopop", None),
                track("背對背擁抱", "JJ Lin", "Mandopop", None),
            ],
        };
        let report = engine::analyze_playlist(&playlist);
        let provider = provider(vec![
            candidate(
                "晴天",
                "周杰倫",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.9),
                &["Mandopop"],
            ),
            candidate(
                "背對背擁抱",
                "林俊傑",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.88),
                &["Mandopop"],
            ),
        ]);
        let (items, summary, _) = build_real_recommendations(&provider, &playlist, &report).await;
        assert!(items.is_empty());
        assert_eq!(summary.query_stats.after_source_exclusion_count, 0);
    }

    #[tokio::test]
    async fn pipeline_statistics_record_each_filter_stage() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let provider = provider(vec![
            candidate(
                "About You - Live",
                "The 1975",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.9),
                &["Pop"],
            ),
            candidate(
                "Levitating",
                "Dua Lipa",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.9),
                &["Pop"],
            ),
            candidate(
                "Cut to the Feeling",
                "Carly Rae Jepsen",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.88),
                &["Pop"],
            ),
            candidate(
                "Cut to the Feeling",
                "Carly Rae Jepsen",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.87),
                &["Pop"],
            ),
            candidate(
                "Emotion",
                "Carly Rae Jepsen",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.86),
                &["Pop"],
            ),
            candidate(
                "Julien",
                "Carly Rae Jepsen",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.85),
                &["Pop"],
            ),
            candidate(
                "Surrender",
                "Carly Rae Jepsen",
                CandidateRelation::TrackSimilar,
                "track.getSimilar",
                Some(0.84),
                &["Pop"],
            ),
        ]);
        let (items, summary, _) = build_real_recommendations(&provider, &playlist, &report).await;
        let stats = summary.query_stats;
        assert_eq!(stats.raw_candidate_count, 7);
        assert_eq!(stats.after_version_filter_count, 6);
        assert_eq!(stats.after_normalization_count, 6);
        assert_eq!(stats.after_deduplication_count, 5);
        assert_eq!(stats.after_source_exclusion_count, 4);
        assert_eq!(stats.after_artist_cap_count, 2);
        assert_eq!(stats.comfort_candidate_count, 2);
        assert_eq!(items.len(), 2);
        assert!(items.iter().all(|item| !item.already_in_source_playlist));
    }

    #[tokio::test]
    async fn tag_layer_statistics_separate_fetched_and_rejected_counts() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let provider = provider(vec![
            candidate(
                "Layer One",
                "Artist One",
                CandidateRelation::AdjacentTagTopTrack,
                "tag.getSimilar depth 1 → tag.getTopTracks",
                None,
                &["Indie Pop"],
            ),
            candidate(
                "Layer Two",
                "Artist Two",
                CandidateRelation::DistantTagTopTrack,
                "tag.getSimilar depth 2 → tag.getTopTracks",
                None,
                &["Dream Pop"],
            ),
            candidate(
                "Layer Two Live",
                "Artist Three",
                CandidateRelation::DistantTagTopTrack,
                "tag.getSimilar depth 2 → tag.getTopTracks",
                None,
                &["Dream Pop"],
            ),
            candidate(
                "Second Hop Artist Track",
                "Artist Four",
                CandidateRelation::SecondHopSimilarArtist,
                "artist.getSimilar depth 2 → artist.getTopTracks",
                Some(0.9),
                &["Dream Pop"],
            ),
            candidate(
                "Genre Graph Bridge Track",
                "Artist Five",
                CandidateRelation::GenreBridgeTopTrack,
                "Genre Graph two-hop → tag.getTopTracks",
                None,
                &["Neo Soul"],
            ),
        ]);
        let (_, summary, _) = build_real_recommendations(&provider, &playlist, &report).await;
        let stats = summary.query_stats;
        assert_eq!(stats.tag_layer1_candidate_count, 1);
        assert_eq!(stats.tag_layer2_candidate_count, 3);
        assert_eq!(stats.tag_layer1_rejected_count, 0);
        assert_eq!(stats.tag_layer2_rejected_count, 1);
    }

    #[tokio::test]
    async fn recommendation_batches_do_not_overlap_and_report_exhaustion() {
        let playlist = playlist();
        let report = engine::analyze_playlist(&playlist);
        let candidates = (0..6)
            .map(|index| {
                candidate(
                    &format!("Fresh Candidate {index}"),
                    &format!("Artist {index}"),
                    CandidateRelation::TrackSimilar,
                    "track.getSimilar",
                    Some(0.8 - index as f32 * 0.02),
                    &["Pop"],
                )
            })
            .collect();
        let (_, summary, _) =
            build_real_recommendations(&provider(candidates), &playlist, &report).await;
        let first = next_recommendation_batch(&summary.comfort_pool, &[], 4);
        let shown: Vec<_> = first.items.iter().map(recommendation_identity).collect();
        let second = next_recommendation_batch(&summary.comfort_pool, &shown, 4);
        let first_ids: HashSet<_> = shown.into_iter().collect();
        assert!(
            second
                .items
                .iter()
                .all(|item| !first_ids.contains(&recommendation_identity(item)))
        );
        assert!(second.exhausted);
        let all_shown: Vec<_> = summary
            .comfort_pool
            .iter()
            .map(recommendation_identity)
            .collect();
        let empty = next_recommendation_batch(&summary.comfort_pool, &all_shown, 4);
        assert!(empty.items.is_empty() && empty.exhausted);
    }
}
