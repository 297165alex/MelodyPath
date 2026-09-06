use crate::{
    models::{
        MatchCandidate, Track, TransferCandidate, TransferMatch, TransferMatchStatus,
        TransferPreview, TransferResult, TransferTrack, TransferTrackResult, VersionType,
    },
    normalize::{is_alternate_version, normalize_text, parse_track_version, token_similarity},
    writers::{
        CreatedPlaylist, PlaylistWriter, spotify::SpotifyPlaylistWriter,
        youtube::YoutubePlaylistWriter,
    },
};
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use uuid::Uuid;

impl From<&Track> for TransferTrack {
    fn from(track: &Track) -> Self {
        Self {
            title: track.title.clone(),
            artists: track.artists.clone(),
            album: track.album.clone(),
            duration_ms: track.duration_ms,
            isrc: track.external_ids.get("isrc").cloned(),
            source_platform: track.platform.clone(),
            source_track_id: track.id.clone(),
            source_url: track.platform_url.clone(),
            normalized_title: parse_track_version(&track.title).base_title,
            normalized_artists: track
                .artists
                .iter()
                .map(|artist| normalize_text(artist))
                .filter(|artist| !artist.is_empty())
                .collect(),
            version_type: track.version_type.clone(),
        }
    }
}

#[async_trait]
pub trait SourceConnector: Send + Sync {
    async fn read_playlist(&self, playlist_id: &str) -> Result<(String, Vec<TransferTrack>)>;
}

#[async_trait]
pub trait DestinationConnector: Send + Sync {
    fn provider_label(&self) -> &'static str;
    fn is_mock(&self) -> bool;
    async fn search(&self, track: &TransferTrack) -> Result<Vec<TransferCandidate>>;
    async fn create_private_playlist(&self, name: &str) -> Result<CreatedPlaylist>;
    async fn add_track(&self, playlist_id: &str, target_id: &str) -> Result<()>;
}

#[derive(Clone)]
pub struct YoutubeDestinationConnector {
    writer: YoutubePlaylistWriter,
    auth_session: Option<String>,
}

impl YoutubeDestinationConnector {
    pub fn new(writer: YoutubePlaylistWriter, auth_session: Option<String>) -> Self {
        Self {
            writer,
            auth_session,
        }
    }
}

#[async_trait]
impl DestinationConnector for YoutubeDestinationConnector {
    fn provider_label(&self) -> &'static str {
        "YouTube Data API v3"
    }

    fn is_mock(&self) -> bool {
        false
    }

    async fn search(&self, track: &TransferTrack) -> Result<Vec<TransferCandidate>> {
        let source = track_to_model(track);
        let result = self
            .writer
            .search_track(&source, self.auth_session.as_deref())
            .await?;
        Ok(result
            .candidates
            .into_iter()
            .map(candidate_from_platform_match)
            .collect())
    }

    async fn create_private_playlist(&self, name: &str) -> Result<CreatedPlaylist> {
        self.writer
            .create_playlist(name, self.auth_session.as_deref())
            .await
    }

    async fn add_track(&self, playlist_id: &str, target_id: &str) -> Result<()> {
        let outcome = self
            .writer
            .add_tracks(
                playlist_id,
                &[target_id.to_string()],
                self.auth_session.as_deref(),
            )
            .await?;
        if let Some(error) = outcome.failures.get(target_id) {
            bail!("{error}");
        }
        if !outcome.added_ids.iter().any(|id| id == target_id) {
            bail!("YouTube 未确认该视频已写入");
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct SpotifyDestinationConnector {
    writer: SpotifyPlaylistWriter,
    auth_session: Option<String>,
}

impl SpotifyDestinationConnector {
    pub fn new(writer: SpotifyPlaylistWriter, auth_session: Option<String>) -> Self {
        Self {
            writer,
            auth_session,
        }
    }
}

#[async_trait]
impl DestinationConnector for SpotifyDestinationConnector {
    fn provider_label(&self) -> &'static str {
        "Spotify Web API"
    }
    fn is_mock(&self) -> bool {
        false
    }

    async fn search(&self, track: &TransferTrack) -> Result<Vec<TransferCandidate>> {
        let result = self
            .writer
            .search_track(&track_to_model(track), self.auth_session.as_deref())
            .await?;
        Ok(result
            .candidates
            .into_iter()
            .map(candidate_from_platform_match)
            .collect())
    }

    async fn create_private_playlist(&self, name: &str) -> Result<CreatedPlaylist> {
        self.writer
            .create_playlist(name, self.auth_session.as_deref())
            .await
    }

    async fn add_track(&self, playlist_id: &str, target_id: &str) -> Result<()> {
        let outcome = self
            .writer
            .add_tracks(
                playlist_id,
                &[target_id.to_string()],
                self.auth_session.as_deref(),
            )
            .await?;
        if let Some(error) = outcome.failures.get(target_id) {
            bail!("{error}");
        }
        if !outcome.added_ids.iter().any(|id| id == target_id) {
            bail!("Spotify 未确认该曲目已写入");
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct MockDestinationConnector {
    pub fail_target_ids: Arc<HashSet<String>>,
    pub added_target_ids: Arc<tokio::sync::RwLock<Vec<String>>>,
}

#[async_trait]
impl DestinationConnector for MockDestinationConnector {
    fn provider_label(&self) -> &'static str {
        "Mock YouTube connector"
    }

    fn is_mock(&self) -> bool {
        true
    }

    async fn search(&self, track: &TransferTrack) -> Result<Vec<TransferCandidate>> {
        Ok(vec![
            TransferCandidate {
                target_id: format!("mock:{}:original", track.source_track_id),
                title: track.title.clone(),
                artists: track.artists.clone(),
                duration_ms: track.duration_ms,
                source_url: Some("mock://youtube/original".into()),
                channel_name: Some(format!("{} - Topic", track.artists.join(" "))),
                official_status: "topic_channel".into(),
                version_type: track.version_type.clone(),
                title_score: 0.0,
                artist_score: 0.0,
                duration_score: None,
                version_score: 0.0,
                score: 0.0,
                reason: String::new(),
            },
            TransferCandidate {
                target_id: format!("mock:{}:live", track.source_track_id),
                title: format!("{} (Live)", track.title),
                artists: track.artists.clone(),
                duration_ms: track.duration_ms.map(|duration| duration + 12_000),
                source_url: Some("mock://youtube/live".into()),
                channel_name: Some(format!("{} Official", track.artists.join(" "))),
                official_status: "official_channel".into(),
                version_type: VersionType::Live,
                title_score: 0.0,
                artist_score: 0.0,
                duration_score: None,
                version_score: 0.0,
                score: 0.0,
                reason: String::new(),
            },
        ])
    }

    async fn create_private_playlist(&self, _name: &str) -> Result<CreatedPlaylist> {
        Ok(CreatedPlaylist {
            id: "mock-playlist".into(),
            url: Some("mock://youtube/playlist/mock-playlist".into()),
        })
    }

    async fn add_track(&self, _playlist_id: &str, target_id: &str) -> Result<()> {
        if self.fail_target_ids.contains(target_id) {
            bail!("Mock 单曲写入失败");
        }
        self.added_target_ids.write().await.push(target_id.into());
        Ok(())
    }
}

pub async fn build_preview(
    connector: &dyn DestinationConnector,
    playlist_name: String,
    tracks: &[Track],
    allow_alternate_versions: bool,
) -> TransferPreview {
    let mut matches = Vec::new();
    let mut provider_errors = 0;
    for source_track in tracks {
        let transfer_track = TransferTrack::from(source_track);
        let candidates = match connector.search(&transfer_track).await {
            Ok(candidates) => candidates,
            Err(_) => {
                provider_errors += 1;
                Vec::new()
            }
        };
        matches.push(rank_match(
            transfer_track,
            candidates,
            allow_alternate_versions,
        ));
    }
    let high_confidence_count = matches
        .iter()
        .filter(|item| item.status == TransferMatchStatus::MatchedHigh)
        .count();
    let ambiguous_count = matches
        .iter()
        .filter(|item| item.status == TransferMatchStatus::MatchedAmbiguous)
        .count();
    let unmatched_count = matches
        .iter()
        .filter(|item| item.status == TransferMatchStatus::Unmatched)
        .count();
    let alternate_fallback_count = matches
        .iter()
        .filter(|item| item.alternate_version_fallback)
        .count();
    TransferPreview {
        preview_id: Uuid::new_v4().to_string(),
        playlist_name,
        source_count: tracks.len(),
        high_confidence_count,
        ambiguous_count,
        unmatched_count,
        alternate_fallback_count,
        matches,
        provider: connector.provider_label().into(),
        destination_platform: if connector.provider_label().starts_with("Spotify") {
            "spotify"
        } else {
            "youtube"
        }
        .into(),
        status: if provider_errors == tracks.len() && !tracks.is_empty() {
            "PROVIDER_ERROR"
        } else if provider_errors > 0 {
            "PARTIAL"
        } else if connector.is_mock() {
            "MOCK_VERIFIED"
        } else {
            "READY"
        }
        .into(),
        message: if connector.is_mock() {
            "Mock connector 只验证匹配、确认和逐首写入流程；不代表真实 YouTube 连接。"
        } else if provider_errors > 0 {
            "部分目标平台搜索失败；失败歌曲保留为未匹配，没有使用 Mock 补齐。"
        } else {
            "候选来自目标平台官方搜索；写入前必须确认所有歧义和版本回退。"
        }
        .into(),
        requires_explicit_confirmation: true,
        source_was_modified: false,
        is_mock: connector.is_mock(),
    }
}

pub fn rank_match(
    source: TransferTrack,
    candidates: Vec<TransferCandidate>,
    allow_alternate_versions: bool,
) -> TransferMatch {
    let mut ranked: Vec<_> = candidates
        .into_iter()
        .map(|candidate| score_candidate(&source, candidate, allow_alternate_versions))
        .collect();
    ranked.sort_by(|left, right| right.score.total_cmp(&left.score));
    ranked.truncate(5);
    let best = ranked.first();
    let fallback = best.is_some_and(|candidate| {
        source.version_type == VersionType::Original
            && is_alternate_version(&candidate.version_type)
            && allow_alternate_versions
    });
    let (status, selected_target_id, score, reason, requires_confirmation) = match best {
        Some(candidate) if candidate.score >= 0.82 && !fallback => (
            TransferMatchStatus::MatchedHigh,
            Some(candidate.target_id.clone()),
            candidate.score,
            candidate.reason.clone(),
            false,
        ),
        Some(candidate) if candidate.score >= 0.58 || fallback => (
            TransferMatchStatus::MatchedAmbiguous,
            None,
            candidate.score,
            if fallback {
                format!("原版未被选中；候选为明确版本回退。{}", candidate.reason)
            } else {
                format!("需要用户确认。{}", candidate.reason)
            },
            true,
        ),
        Some(candidate) => (
            TransferMatchStatus::Unmatched,
            None,
            candidate.score,
            "候选低于最低匹配阈值".into(),
            false,
        ),
        None => (
            TransferMatchStatus::Unmatched,
            None,
            0.0,
            "YouTube 没有返回候选或 Provider 请求失败".into(),
            false,
        ),
    };
    TransferMatch {
        source_track: source,
        candidates: ranked,
        selected_target_id,
        status,
        score,
        reason,
        requires_confirmation,
        alternate_version_fallback: fallback,
    }
}

pub fn score_candidate(
    source: &TransferTrack,
    mut candidate: TransferCandidate,
    allow_alternate_versions: bool,
) -> TransferCandidate {
    let source_base = parse_track_version(&source.title);
    let candidate_base = parse_track_version(&candidate.title);
    candidate.title_score = token_similarity(&source_base.base_title, &candidate_base.base_title);
    candidate.artist_score = token_similarity(
        &source.normalized_artists.join(" "),
        &candidate.artists.join(" "),
    );
    candidate.duration_score =
        source
            .duration_ms
            .zip(candidate.duration_ms)
            .map(|(left, right)| {
                let delta = left.abs_diff(right) as f32;
                (1.0 - delta / 60_000.0).clamp(0.0, 1.0)
            });
    candidate.version_type = candidate_base.version_type;
    let version_consistent = candidate.version_type == source.version_type;
    let alternate = source.version_type == VersionType::Original
        && is_alternate_version(&candidate.version_type);
    candidate.version_score = if version_consistent {
        1.0
    } else if alternate && allow_alternate_versions {
        0.62
    } else {
        0.0
    };
    let official_score = if candidate.official_status.contains("official")
        || candidate.official_status.contains("topic")
    {
        1.0
    } else {
        0.35
    };
    let mut weighted = candidate.title_score * 0.45
        + candidate.artist_score * 0.30
        + candidate.version_score * 0.12
        + official_score * 0.08;
    let mut weight = 0.95;
    if let Some(duration) = candidate.duration_score {
        weighted += duration * 0.05;
        weight += 0.05;
    }
    candidate.score = (weighted / weight).clamp(0.0, 0.99);
    if alternate && !allow_alternate_versions {
        candidate.score *= 0.35;
    }
    let duration_reason = candidate
        .duration_score
        .map(|score| format!(" · 时长 {:.0}%", score * 100.0))
        .unwrap_or_else(|| " · 时长未知（不按 0 处理）".into());
    candidate.reason = format!(
        "歌名 {:.0}% · 艺人 {:.0}%{} · 版本 {:.0}% · {}",
        candidate.title_score * 100.0,
        candidate.artist_score * 100.0,
        duration_reason,
        candidate.version_score * 100.0,
        candidate.official_status
    );
    candidate
}

pub async fn execute_transfer(
    connector: &dyn DestinationConnector,
    preview: &TransferPreview,
    confirmed: bool,
    selections: &HashMap<String, String>,
    cancelled: &AtomicBool,
) -> Result<TransferResult> {
    execute_transfer_with_progress(connector, preview, confirmed, selections, cancelled, None).await
}

pub async fn execute_transfer_with_progress(
    connector: &dyn DestinationConnector,
    preview: &TransferPreview,
    confirmed: bool,
    selections: &HashMap<String, String>,
    cancelled: &AtomicBool,
    on_progress: Option<&(dyn Fn(&TransferResult) + Send + Sync)>,
) -> Result<TransferResult> {
    if !confirmed {
        bail!("写入前必须明确确认复制预览");
    }
    let selected_ids = selected_targets(preview, selections)?;
    let playlist = connector
        .create_private_playlist(&format!(
            "{} · Transferred by MelodyPath",
            preview.playlist_name
        ))
        .await?;
    execute_into_playlist(
        connector,
        preview,
        selected_ids,
        playlist,
        Vec::new(),
        cancelled,
        on_progress,
    )
    .await
}

pub async fn resume_transfer(
    connector: &dyn DestinationConnector,
    preview: &TransferPreview,
    previous: &TransferResult,
    confirmed: bool,
    selections: &HashMap<String, String>,
    cancelled: &AtomicBool,
) -> Result<TransferResult> {
    resume_transfer_with_progress(
        connector, preview, previous, confirmed, selections, cancelled, None,
    )
    .await
}

pub async fn resume_transfer_with_progress(
    connector: &dyn DestinationConnector,
    preview: &TransferPreview,
    previous: &TransferResult,
    confirmed: bool,
    selections: &HashMap<String, String>,
    cancelled: &AtomicBool,
    on_progress: Option<&(dyn Fn(&TransferResult) + Send + Sync)>,
) -> Result<TransferResult> {
    if !confirmed {
        bail!("恢复写入前必须再次确认复制预览");
    }
    let playlist_id = previous
        .playlist_id
        .clone()
        .context("上一次执行没有可恢复的目标播放列表")?;
    let selected_ids = selected_targets(preview, selections)?;
    let completed_results = previous
        .results
        .iter()
        .filter(|item| item.status != "FAILED")
        .cloned()
        .collect();
    execute_into_playlist(
        connector,
        preview,
        selected_ids,
        CreatedPlaylist {
            id: playlist_id,
            url: previous.playlist_url.clone(),
        },
        completed_results,
        cancelled,
        on_progress,
    )
    .await
}

fn selected_targets(
    preview: &TransferPreview,
    selections: &HashMap<String, String>,
) -> Result<HashMap<String, String>> {
    let mut selected_ids = HashMap::new();
    for item in &preview.matches {
        match item.status {
            TransferMatchStatus::MatchedHigh => {
                if let Some(target) = &item.selected_target_id {
                    selected_ids.insert(item.source_track.source_track_id.clone(), target.clone());
                }
            }
            TransferMatchStatus::MatchedAmbiguous => {
                let selection = selections
                    .get(&item.source_track.source_track_id)
                    .context("所有歧义或版本回退歌曲都必须确认、改选或跳过")?;
                if selection != "SKIP"
                    && !item
                        .candidates
                        .iter()
                        .any(|candidate| candidate.target_id == *selection)
                {
                    bail!("歧义选择不属于该歌曲的候选列表");
                }
                selected_ids.insert(item.source_track.source_track_id.clone(), selection.clone());
            }
            TransferMatchStatus::Unmatched | TransferMatchStatus::Skipped => {}
        }
    }
    Ok(selected_ids)
}

pub fn validate_transfer_execution(
    preview: &TransferPreview,
    confirmed: bool,
    selections: &HashMap<String, String>,
) -> Result<()> {
    if !confirmed {
        bail!("写入前必须明确确认复制预览");
    }
    selected_targets(preview, selections).map(|_| ())
}

async fn execute_into_playlist(
    connector: &dyn DestinationConnector,
    preview: &TransferPreview,
    selected_ids: HashMap<String, String>,
    playlist: CreatedPlaylist,
    mut results: Vec<TransferTrackResult>,
    cancelled: &AtomicBool,
    on_progress: Option<&(dyn Fn(&TransferResult) + Send + Sync)>,
) -> Result<TransferResult> {
    let completed_source_ids: HashSet<_> = results
        .iter()
        .map(|item| item.source_track.source_track_id.clone())
        .collect();
    for item in &preview.matches {
        if completed_source_ids.contains(&item.source_track.source_track_id) {
            continue;
        }
        if cancelled.load(Ordering::SeqCst) {
            let result = summarize_result(
                preview,
                Some(playlist.id),
                playlist.url,
                results,
                "CANCELLED",
                connector.is_mock(),
            );
            if let Some(callback) = on_progress {
                callback(&result);
            }
            return Ok(result);
        }
        let Some(target_id) = selected_ids.get(&item.source_track.source_track_id) else {
            results.push(TransferTrackResult {
                source_track: item.source_track.clone(),
                target_id: None,
                status: if item.status == TransferMatchStatus::Unmatched {
                    "UNMATCHED"
                } else {
                    "SKIPPED"
                }
                .into(),
                error: None,
            });
            notify_progress(
                on_progress,
                preview,
                &playlist,
                &results,
                connector.is_mock(),
            );
            continue;
        };
        if target_id == "SKIP" {
            results.push(TransferTrackResult {
                source_track: item.source_track.clone(),
                target_id: None,
                status: "SKIPPED".into(),
                error: None,
            });
            notify_progress(
                on_progress,
                preview,
                &playlist,
                &results,
                connector.is_mock(),
            );
            continue;
        }
        match connector.add_track(&playlist.id, target_id).await {
            Ok(()) => results.push(TransferTrackResult {
                source_track: item.source_track.clone(),
                target_id: Some(target_id.clone()),
                status: "WRITTEN".into(),
                error: None,
            }),
            Err(error) => results.push(TransferTrackResult {
                source_track: item.source_track.clone(),
                target_id: Some(target_id.clone()),
                status: "FAILED".into(),
                error: Some(error.to_string()),
            }),
        }
        notify_progress(
            on_progress,
            preview,
            &playlist,
            &results,
            connector.is_mock(),
        );
    }
    Ok(summarize_result(
        preview,
        Some(playlist.id),
        playlist.url,
        results,
        "COMPLETED",
        connector.is_mock(),
    ))
}

fn notify_progress(
    callback: Option<&(dyn Fn(&TransferResult) + Send + Sync)>,
    preview: &TransferPreview,
    playlist: &CreatedPlaylist,
    results: &[TransferTrackResult],
    is_mock: bool,
) {
    if let Some(callback) = callback {
        let snapshot = summarize_result(
            preview,
            Some(playlist.id.clone()),
            playlist.url.clone(),
            results.to_vec(),
            "RUNNING",
            is_mock,
        );
        callback(&snapshot);
    }
}

fn summarize_result(
    preview: &TransferPreview,
    playlist_id: Option<String>,
    playlist_url: Option<String>,
    results: Vec<TransferTrackResult>,
    status: &str,
    is_mock: bool,
) -> TransferResult {
    let written_count = results
        .iter()
        .filter(|item| item.status == "WRITTEN")
        .count();
    let failed_count = results
        .iter()
        .filter(|item| item.status == "FAILED")
        .count();
    let skipped_count = results
        .iter()
        .filter(|item| item.status == "SKIPPED")
        .count();
    let unmatched_count = results
        .iter()
        .filter(|item| item.status == "UNMATCHED")
        .count();
    TransferResult {
        run_id: Uuid::new_v4().to_string(),
        preview_id: preview.preview_id.clone(),
        status: status.into(),
        source_count: preview.source_count,
        matched_count: preview.high_confidence_count + preview.ambiguous_count,
        written_count,
        failed_count,
        skipped_count,
        unmatched_count,
        progress: results.len() as f32 / preview.source_count.max(1) as f32,
        playlist_id,
        playlist_url,
        report_csv_url: None,
        report_json_url: None,
        results,
        source_was_modified: false,
        is_mock,
    }
}

fn candidate_from_platform_match(candidate: MatchCandidate) -> TransferCandidate {
    TransferCandidate {
        target_id: candidate.target_track_id,
        title: candidate.title,
        artists: candidate.artists,
        duration_ms: candidate.duration_ms,
        source_url: candidate.target_url,
        channel_name: candidate.channel_name,
        official_status: candidate.official_status,
        version_type: candidate.version_type,
        title_score: 0.0,
        artist_score: 0.0,
        duration_score: None,
        version_score: 0.0,
        score: candidate.confidence,
        reason: candidate.match_reason,
    }
}

fn track_to_model(track: &TransferTrack) -> Track {
    Track {
        id: track.source_track_id.clone(),
        title: track.title.clone(),
        normalized_title: track.normalized_title.clone(),
        artists: track.artists.clone(),
        album: track.album.clone(),
        genres: Vec::new(),
        release_year: None,
        language: None,
        duration_ms: track.duration_ms,
        platform: track.source_platform.clone(),
        platform_url: track.source_url.clone(),
        external_ids: track
            .isrc
            .as_ref()
            .map(|isrc| HashMap::from([("isrc".into(), isrc.clone())]))
            .unwrap_or_default(),
        version_type: track.version_type.clone(),
        mood_tags: Vec::new(),
        energy_score: None,
        popularity: None,
        metadata_confidence: 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::detect_version;
    use tokio::sync::RwLock;

    fn track(id: &str, title: &str, artist: &str, duration_ms: Option<u32>) -> Track {
        Track {
            id: id.into(),
            title: title.into(),
            normalized_title: normalize_text(title),
            artists: vec![artist.into()],
            album: None,
            genres: Vec::new(),
            release_year: None,
            language: None,
            duration_ms,
            platform: "spotify".into(),
            platform_url: None,
            external_ids: HashMap::new(),
            version_type: detect_version(title),
            mood_tags: Vec::new(),
            energy_score: None,
            popularity: None,
            metadata_confidence: 1.0,
        }
    }

    fn candidate(
        id: &str,
        title: &str,
        artist: &str,
        duration_ms: Option<u32>,
    ) -> TransferCandidate {
        TransferCandidate {
            target_id: id.into(),
            title: title.into(),
            artists: vec![artist.into()],
            duration_ms,
            source_url: None,
            channel_name: Some(format!("{artist} - Topic")),
            official_status: "topic_channel".into(),
            version_type: detect_version(title),
            title_score: 0.0,
            artist_score: 0.0,
            duration_score: None,
            version_score: 0.0,
            score: 0.0,
            reason: String::new(),
        }
    }

    #[derive(Clone)]
    struct MockSource {
        pages: Vec<Vec<TransferTrack>>,
        source_mutations: Arc<RwLock<usize>>,
    }

    #[async_trait]
    impl SourceConnector for MockSource {
        async fn read_playlist(&self, _playlist_id: &str) -> Result<(String, Vec<TransferTrack>)> {
            Ok((
                "Paged Spotify source".into(),
                self.pages.iter().flatten().cloned().collect(),
            ))
        }
    }

    #[tokio::test]
    async fn source_connector_reads_every_page_without_modifying_source() {
        let source = MockSource {
            pages: vec![
                vec![TransferTrack::from(&track("1", "One", "Artist", None))],
                vec![TransferTrack::from(&track("2", "Two", "Artist", None))],
                vec![TransferTrack::from(&track("3", "Three", "Artist", None))],
            ],
            source_mutations: Arc::new(RwLock::new(0)),
        };
        let (_, tracks) = source.read_playlist("playlist").await.unwrap();
        assert_eq!(tracks.len(), 3);
        assert_eq!(*source.source_mutations.read().await, 0);
    }

    #[test]
    fn scoring_uses_title_artist_duration_official_and_version() {
        let source = TransferTrack::from(&track("1", "Levitating", "Dua Lipa", Some(203_000)));
        let exact = score_candidate(
            &source,
            candidate("exact", "Levitating", "Dua Lipa", Some(204_000)),
            false,
        );
        let live = score_candidate(
            &source,
            candidate("live", "Levitating (Live)", "Dua Lipa", Some(280_000)),
            false,
        );
        assert!(exact.score > live.score);
        assert!(exact.duration_score.unwrap() > 0.9);
        assert_eq!(live.version_score, 0.0);
    }

    #[test]
    fn preview_keeps_at_most_five_candidates_and_marks_ambiguity() {
        let source = TransferTrack::from(&track("1", "Hello", "Adele", None));
        let candidates = (0..8)
            .map(|index| candidate(&index.to_string(), "Hello", "Adele", None))
            .collect();
        let matched = rank_match(source, candidates, false);
        assert_eq!(matched.candidates.len(), 5);
        assert_eq!(matched.status, TransferMatchStatus::MatchedHigh);
    }

    #[test]
    fn alternate_fallback_requires_opt_in_and_confirmation() {
        let source = TransferTrack::from(&track("1", "Hello", "Adele", None));
        let live = candidate("live", "Hello (Live)", "Adele", None);
        let without = rank_match(source.clone(), vec![live.clone()], false);
        let with = rank_match(source, vec![live], true);
        assert_eq!(without.status, TransferMatchStatus::Unmatched);
        assert_eq!(with.status, TransferMatchStatus::MatchedAmbiguous);
        assert!(with.alternate_version_fallback && with.requires_confirmation);
    }

    #[tokio::test]
    async fn confirmation_is_required_before_any_write() {
        let connector = MockDestinationConnector::default();
        let source = vec![track("1", "One", "Artist", None)];
        let preview = build_preview(&connector, "Test".into(), &source, false).await;
        let error = execute_transfer(
            &connector,
            &preview,
            false,
            &HashMap::new(),
            &AtomicBool::new(false),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("明确确认"));
        assert!(connector.added_target_ids.read().await.is_empty());
    }

    #[tokio::test]
    async fn one_write_failure_does_not_stop_remaining_tracks() {
        let connector = MockDestinationConnector {
            fail_target_ids: Arc::new(HashSet::from(["mock:2:original".into()])),
            added_target_ids: Arc::new(RwLock::new(Vec::new())),
        };
        let source = vec![
            track("1", "One", "Artist", None),
            track("2", "Two", "Artist", None),
            track("3", "Three", "Artist", None),
        ];
        let preview = build_preview(&connector, "Test".into(), &source, false).await;
        let result = execute_transfer(
            &connector,
            &preview,
            true,
            &HashMap::new(),
            &AtomicBool::new(false),
        )
        .await
        .unwrap();
        assert_eq!(result.written_count, 2);
        assert_eq!(result.failed_count, 1);
        assert!(!result.source_was_modified);
    }

    #[tokio::test]
    async fn cancellation_stops_before_writing_and_is_reported() {
        let connector = MockDestinationConnector::default();
        let source = vec![track("1", "One", "Artist", None)];
        let preview = build_preview(&connector, "Test".into(), &source, false).await;
        let cancelled = AtomicBool::new(true);
        let result = execute_transfer(&connector, &preview, true, &HashMap::new(), &cancelled)
            .await
            .unwrap();
        assert_eq!(result.status, "CANCELLED");
        assert_eq!(result.written_count, 0);
    }

    #[derive(Clone)]
    struct CancellingConnector {
        inner: MockDestinationConnector,
        cancel: Arc<AtomicBool>,
    }

    #[async_trait]
    impl DestinationConnector for CancellingConnector {
        fn provider_label(&self) -> &'static str {
            self.inner.provider_label()
        }

        fn is_mock(&self) -> bool {
            true
        }

        async fn search(&self, track: &TransferTrack) -> Result<Vec<TransferCandidate>> {
            self.inner.search(track).await
        }

        async fn create_private_playlist(&self, name: &str) -> Result<CreatedPlaylist> {
            self.inner.create_private_playlist(name).await
        }

        async fn add_track(&self, playlist_id: &str, target_id: &str) -> Result<()> {
            self.inner.add_track(playlist_id, target_id).await?;
            self.cancel.store(true, Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn cancelled_transfer_resumes_without_rewriting_completed_tracks() {
        let cancel = Arc::new(AtomicBool::new(false));
        let inner = MockDestinationConnector::default();
        let cancelling = CancellingConnector {
            inner: inner.clone(),
            cancel: cancel.clone(),
        };
        let source = vec![
            track("1", "One", "Artist", None),
            track("2", "Two", "Artist", None),
            track("3", "Three", "Artist", None),
        ];
        let preview = build_preview(&inner, "Test".into(), &source, false).await;
        let interrupted = execute_transfer(
            &cancelling,
            &preview,
            true,
            &HashMap::new(),
            cancel.as_ref(),
        )
        .await
        .unwrap();
        assert_eq!(interrupted.status, "CANCELLED");
        assert_eq!(interrupted.written_count, 1);
        cancel.store(false, Ordering::SeqCst);
        let resumed = resume_transfer(
            &inner,
            &preview,
            &interrupted,
            true,
            &HashMap::new(),
            cancel.as_ref(),
        )
        .await
        .unwrap();
        assert_eq!(resumed.status, "COMPLETED");
        assert_eq!(resumed.written_count, 3);
        let added = inner.added_target_ids.read().await;
        assert_eq!(added.len(), 3);
        assert_eq!(
            added.iter().filter(|id| *id == "mock:1:original").count(),
            1
        );
    }

    #[test]
    fn unicode_titles_and_artists_score_without_loss() {
        let source = TransferTrack::from(&track("cjk", "晴天", "周杰倫", Some(269_000)));
        let scored = score_candidate(
            &source,
            candidate("target", "晴天", "周杰倫", Some(270_000)),
            false,
        );
        assert!(scored.score > 0.9);
        assert_eq!(scored.title_score, 1.0);
        assert_eq!(scored.artist_score, 1.0);
    }

    #[tokio::test]
    async fn large_mock_playlist_keeps_every_source_track() {
        let connector = MockDestinationConnector::default();
        let source: Vec<_> = (0..500)
            .map(|index| track(&index.to_string(), &format!("Song {index}"), "Artist", None))
            .collect();
        let preview = build_preview(&connector, "Large".into(), &source, false).await;
        assert_eq!(preview.source_count, 500);
        assert_eq!(preview.matches.len(), 500);
        assert_eq!(preview.high_confidence_count, 500);
    }
}
