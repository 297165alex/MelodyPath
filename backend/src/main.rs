mod agent;
mod alternate;
mod demo;
mod engine;
mod export;
mod genre;
mod identity;
mod import;
mod metadata;
mod models;
mod normalize;
mod platforms;
mod recommendation;
mod resolver;
mod secure_store;
mod transfer;
mod writers;

use axum::{
    Json, Router,
    body::Body,
    extract::{Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{
        IntoResponse, Redirect, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{get, post},
};
use models::{
    ExportExecuteRequest, ExportPreview, ExportPreviewRequest, ImportPreviewRequest,
    ManualAnalyzeRequest, MatchStatus, PlatformTrackMatch, PlaylistExportResult,
    PlaylistLinkRequest, SpotifyImportRequest, StoredPreview, TrackFailure, WriterStatus,
    YoutubeImportRequest,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    convert::Infallible,
    path::PathBuf,
    sync::{
        Arc, RwLock as StdRwLock,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::{net::TcpListener, sync::RwLock, time::sleep};
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use uuid::Uuid;
use writers::{
    DemoPlaylistWriter, ExportFilePlaylistWriter, PlaylistWriter, UnavailablePlaylistWriter,
    apple::AppleMusicConnector, spotify::SpotifyPlaylistWriter, unavailable_writers,
    youtube::YoutubePlaylistWriter,
};

#[derive(Clone)]
struct AppState {
    previews: Arc<RwLock<HashMap<String, StoredPreview>>>,
    downloads: Arc<RwLock<HashMap<String, DownloadArtifact>>>,
    spotify: SpotifyPlaylistWriter,
    youtube: YoutubePlaylistWriter,
    apple: AppleMusicConnector,
    platforms: platforms::PlatformService,
    agent: agent::AgentService,
    metadata: metadata::MetadataService,
    analyses: agent::AnalysisRegistry,
    imports: Arc<RwLock<HashMap<String, import::StoredImport>>>,
    transfer_previews: Arc<RwLock<HashMap<String, models::TransferPreview>>>,
    transfer_runs: Arc<StdRwLock<HashMap<String, StoredTransferRun>>>,
}

#[derive(Clone)]
struct StoredTransferRun {
    public: models::TransferRun,
    preview: models::TransferPreview,
    request: models::TransferExecuteRequest,
    auth_session: Option<String>,
    cancelled: Arc<AtomicBool>,
}

#[derive(Clone)]
struct DownloadArtifact {
    filename: String,
    content_type: String,
    bytes: Vec<u8>,
}

#[derive(Debug, Serialize)]
struct ApiErrorBody {
    error: String,
    code: &'static str,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            code: "bad_request",
            message: message.into(),
        }
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::UNAUTHORIZED,
            code: "authorization_required",
            message: message.into(),
        }
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            code: "not_found",
            message: message.into(),
        }
    }

    fn internal(error: impl std::fmt::Display) -> Self {
        tracing::error!(error = %error, "request failed");
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            code: "internal_error",
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiErrorBody {
                error: self.message,
                code: self.code,
            }),
        )
            .into_response()
    }
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/api/demo", get(get_demo))
        .route("/api/analyze/manual", post(analyze_manual))
        .route("/api/imports/preview", post(preview_import))
        .route("/api/imports/{id}/analyze", post(analyze_import))
        .route("/api/compare", post(compare_analyses))
        .route(
            "/api/alternate-versions/search",
            post(search_alternate_versions),
        )
        .route("/api/transfers/preview", post(transfer_preview))
        .route("/api/transfers/execute", post(transfer_execute))
        .route("/api/transfers/runs", post(create_transfer_run))
        .route("/api/transfers/runs/{id}", get(get_transfer_run))
        .route("/api/transfers/runs/{id}/events", get(transfer_run_events))
        .route("/api/transfers/runs/{id}/cancel", post(cancel_transfer_run))
        .route("/api/transfers/runs/{id}/resume", post(resume_transfer_run))
        .route("/api/writers/status", get(writer_statuses))
        .route("/api/platforms/capabilities", get(platform_capabilities))
        .route("/api/config/spotify", get(spotify_configuration))
        .route(
            "/api/config/spotify/check",
            post(check_spotify_configuration),
        )
        .route("/api/config/youtube", get(youtube_configuration))
        .route(
            "/api/config/youtube/check",
            post(check_youtube_configuration),
        )
        .route("/api/config/apple", get(apple_configuration))
        .route(
            "/api/apple/musickit/bootstrap",
            get(apple_musickit_bootstrap),
        )
        .route("/api/playlists/inspect-link", post(inspect_playlist_link))
        .route("/api/settings", get(get_settings).put(update_settings))
        .route("/api/tasks", get(list_tasks).post(create_task))
        .route("/api/tasks/{id}", get(get_task))
        .route("/api/tasks/{id}/events", get(task_events))
        .route("/api/tasks/{id}/cancel", post(cancel_task))
        .route("/api/tasks/{id}/resume", post(resume_task))
        .route("/api/spotify/authorize", get(spotify_authorize))
        .route("/api/spotify/callback", get(spotify_callback))
        .route("/api/spotify/me", get(spotify_me))
        .route("/api/spotify/playlists", get(spotify_playlists))
        .route("/api/spotify/import", post(spotify_import))
        .route("/api/spotify/disconnect", post(spotify_disconnect))
        .route("/api/youtube/authorize", get(youtube_authorize))
        .route("/api/youtube/callback", get(youtube_callback))
        .route("/api/youtube/me", get(youtube_me))
        .route("/api/youtube/playlists", get(youtube_playlists))
        .route("/api/youtube/import", post(youtube_import))
        .route("/api/youtube/disconnect", post(youtube_disconnect))
        .route("/api/exports/preview", post(export_preview))
        .route("/api/exports/execute", post(export_execute))
        .route("/api/exports/{id}/download", get(download_export))
        .layer(TraceLayer::new_for_http().make_span_with(|request: &axum::http::Request<Body>| {
            // OAuth codes/state are in the query; headers may contain sessions.
            tracing::info_span!("http_request", method = %request.method(), path = %request.uri().path())
        }))
        .with_state(state)
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "melody_path_api=info,tower_http=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    let database_path = std::env::var("MELODYPATH_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("melody_path.db"));
    let analyses = Arc::new(RwLock::new(HashMap::new()));
    let metadata = metadata::MetadataService::new();
    let state = AppState {
        previews: Arc::new(RwLock::new(HashMap::new())),
        downloads: Arc::new(RwLock::new(HashMap::new())),
        spotify: SpotifyPlaylistWriter::new(),
        youtube: writers::youtube::YoutubePlaylistWriter::new(),
        apple: AppleMusicConnector::new(),
        platforms: platforms::PlatformService::new(),
        agent: agent::AgentService::new(database_path, analyses.clone()).await?,
        metadata,
        analyses,
        imports: Arc::new(RwLock::new(HashMap::new())),
        transfer_previews: Arc::new(RwLock::new(HashMap::new())),
        transfer_runs: Arc::new(StdRwLock::new(HashMap::new())),
    };
    let address = std::env::var("MELODYPATH_BIND").unwrap_or_else(|_| "127.0.0.1:3000".into());
    let listener = TcpListener::bind(&address).await?;
    tracing::info!(%address, "MelodyPath API listening");
    axum::serve(listener, app(state))
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

async fn health() -> Json<serde_json::Value> {
    Json(
        serde_json::json!({ "status": "ok", "service": "melody-path-api", "mode": "offline-ready" }),
    )
}

async fn get_demo() -> Json<models::DemoPayload> {
    Json(demo::demo_payload())
}

async fn analyze_manual(
    State(state): State<AppState>,
    Json(request): Json<ManualAnalyzeRequest>,
) -> Result<Json<models::PersonalDemo>, ApiError> {
    let playlist = engine::parse_manual_playlist(
        request.name.as_deref().unwrap_or("本地输入歌单"),
        &request.text,
    )
    .map_err(ApiError::bad_request)?;
    let analysis = state.metadata.analyze(playlist).await;
    state
        .analyses
        .write()
        .await
        .insert(analysis.analysis_id.clone(), analysis.clone());
    Ok(Json(analysis))
}

async fn preview_import(
    State(state): State<AppState>,
    Json(request): Json<ImportPreviewRequest>,
) -> Result<Json<models::ImportPreview>, ApiError> {
    let stored = import::parse_import(request).map_err(ApiError::bad_request)?;
    let preview = stored.preview();
    state
        .imports
        .write()
        .await
        .insert(stored.id.clone(), stored);
    Ok(Json(preview))
}

async fn analyze_import(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<models::PersonalDemo>, ApiError> {
    let imported = state
        .imports
        .read()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("导入预览不存在或服务已重启，请重新解析"))?;
    let mut analysis = state
        .metadata
        .analyze_imported(
            imported.name,
            imported.source_label,
            imported.data_state,
            imported.total_rows,
            imported.tracks,
        )
        .await;
    analysis.analysis_id = id;
    state
        .analyses
        .write()
        .await
        .insert(analysis.analysis_id.clone(), analysis.clone());
    Ok(Json(analysis))
}

async fn compare_analyses(
    Json(request): Json<models::CompareRequest>,
) -> Result<Json<models::ComparisonReport>, ApiError> {
    if request.analysis_a.playlist.tracks.is_empty()
        || request.analysis_b.playlist.tracks.is_empty()
    {
        return Err(ApiError::bad_request("两份歌单都必须至少包含一首歌曲"));
    }
    if request.save_locally {
        return Err(ApiError::bad_request(
            "当前临时比较不会默认保存；本地保存需要在后续界面中单独明确确认",
        ));
    }
    Ok(Json(engine::compare_analyses(
        &request.analysis_a,
        &request.analysis_b,
    )))
}

async fn search_alternate_versions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<models::AlternateVersionSearchRequest>,
) -> Json<models::AlternateVersionSearchResult> {
    if request.use_mock {
        return Json(alternate::mock_version_search(
            request.track,
            request.version_types,
        ));
    }
    let session = youtube_auth_session(&headers);
    let connection = state.youtube.connection_status(session.as_deref()).await;
    if !connection.connected {
        return Json(models::AlternateVersionSearchResult {
            source_track: request.track,
            provider: "YouTube Data API v3".into(),
            status: "BLOCKED_EXTERNAL_AUTH".into(),
            message: if connection.configured {
                "需要用户在 Google 官方页面完成 YouTube OAuth；没有用 Mock 冒充真实搜索。"
            } else {
                "Google OAuth / YouTube Data API 配置不完整；没有发起搜索，也没有用 Mock 冒充真实搜索。"
            }
            .into(),
            candidates: Vec::new(),
            is_mock: false,
        });
    }
    match state
        .youtube
        .search_alternate_versions(&request.track, request.version_types, session.as_deref())
        .await
    {
        Ok(result) => Json(result),
        Err(_) => Json(models::AlternateVersionSearchResult {
            source_track: request.track,
            provider: "YouTube Data API v3".into(),
            status: "PROVIDER_ERROR".into(),
            message: "YouTube 官方搜索暂时失败；没有记录响应正文，也没有回退到 Mock。".into(),
            candidates: Vec::new(),
            is_mock: false,
        }),
    }
}

async fn transfer_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<models::TransferPreviewRequest>,
) -> Result<Json<models::TransferPreview>, ApiError> {
    if request.tracks.is_empty() {
        return Err(ApiError::bad_request("迁移源歌单不能为空"));
    }
    let preview = if request.use_mock {
        transfer::build_preview(
            &transfer::MockDestinationConnector::default(),
            request.playlist_name,
            &request.tracks,
            request.allow_alternate_versions,
        )
        .await
    } else {
        let (connected, configured, provider) = match request.destination_platform.as_str() {
            "youtube" => {
                let status = state
                    .youtube
                    .connection_status(youtube_auth_session(&headers).as_deref())
                    .await;
                (status.connected, status.configured, "YouTube Data API v3")
            }
            "spotify" => {
                let status = state
                    .spotify
                    .connection_status(auth_session(&headers).as_deref())
                    .await;
                (status.connected, status.configured, "Spotify Web API")
            }
            _ => {
                return Err(ApiError::bad_request(
                    "目标平台没有已验证的官方播放列表 Writer",
                ));
            }
        };
        if !connected {
            models::TransferPreview {
                preview_id: Uuid::new_v4().to_string(),
                playlist_name: request.playlist_name,
                source_count: request.tracks.len(),
                high_confidence_count: 0,
                ambiguous_count: 0,
                unmatched_count: request.tracks.len(),
                alternate_fallback_count: 0,
                matches: Vec::new(),
                provider: provider.into(),
                destination_platform: request.destination_platform,
                status: "BLOCKED_EXTERNAL_AUTH".into(),
                message: if configured {
                    "需要用户在目标平台官方页面完成 OAuth 后才能搜索和写入；没有用 Mock 冒充连接。"
                } else {
                    "目标平台 OAuth 配置不完整；没有搜索、写入或切换到 Mock。"
                }
                .into(),
                requires_explicit_confirmation: true,
                source_was_modified: false,
                is_mock: false,
            }
        } else {
            match request.destination_platform.as_str() {
                "youtube" => {
                    transfer::build_preview(
                        &transfer::YoutubeDestinationConnector::new(
                            state.youtube.clone(),
                            youtube_auth_session(&headers),
                        ),
                        request.playlist_name,
                        &request.tracks,
                        request.allow_alternate_versions,
                    )
                    .await
                }
                "spotify" => {
                    transfer::build_preview(
                        &transfer::SpotifyDestinationConnector::new(
                            state.spotify.clone(),
                            auth_session(&headers),
                        ),
                        request.playlist_name,
                        &request.tracks,
                        request.allow_alternate_versions,
                    )
                    .await
                }
                _ => unreachable!(),
            }
        }
    };
    state
        .transfer_previews
        .write()
        .await
        .insert(preview.preview_id.clone(), preview.clone());
    Ok(Json(preview))
}

async fn transfer_execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<models::TransferExecuteRequest>,
) -> Result<Json<models::TransferResult>, ApiError> {
    if request.privacy != "private" {
        return Err(ApiError::bad_request("当前 MVP 只创建新的私有目标播放列表"));
    }
    let preview = state
        .transfer_previews
        .read()
        .await
        .get(&request.preview_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("迁移预览不存在或服务已重启"))?;
    if preview.status == "BLOCKED_EXTERNAL_AUTH" {
        return Err(ApiError::unauthorized(preview.message));
    }
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let mut result = if preview.is_mock {
        transfer::execute_transfer(
            &transfer::MockDestinationConnector::default(),
            &preview,
            request.confirmed,
            &request.selections,
            &cancelled,
        )
        .await
    } else {
        match preview.destination_platform.as_str() {
            "youtube" => {
                let session = youtube_auth_session(&headers);
                if !state
                    .youtube
                    .connection_status(session.as_deref())
                    .await
                    .connected
                {
                    return Err(ApiError::unauthorized(
                        "YouTube OAuth 会话不存在或已过期；未创建播放列表",
                    ));
                }
                transfer::execute_transfer(
                    &transfer::YoutubeDestinationConnector::new(state.youtube.clone(), session),
                    &preview,
                    request.confirmed,
                    &request.selections,
                    &cancelled,
                )
                .await
            }
            "spotify" => {
                let session = auth_session(&headers);
                if !state
                    .spotify
                    .connection_status(session.as_deref())
                    .await
                    .connected
                {
                    return Err(ApiError::unauthorized(
                        "Spotify OAuth 会话不存在或已过期；未创建播放列表",
                    ));
                }
                transfer::execute_transfer(
                    &transfer::SpotifyDestinationConnector::new(state.spotify.clone(), session),
                    &preview,
                    request.confirmed,
                    &request.selections,
                    &cancelled,
                )
                .await
            }
            _ => return Err(ApiError::bad_request("迁移预览的目标平台无可用 Writer")),
        }
    }
    .map_err(|error| ApiError::bad_request(error.to_string()))?;

    let csv_id = Uuid::new_v4().to_string();
    let json_id = Uuid::new_v4().to_string();
    let csv_bytes = transfer_report_csv(&result).map_err(ApiError::internal)?;
    let json_bytes = serde_json::to_vec_pretty(&result).map_err(ApiError::internal)?;
    let mut downloads = state.downloads.write().await;
    downloads.insert(
        csv_id.clone(),
        DownloadArtifact {
            filename: format!("transfer-report-{}.csv", result.run_id),
            content_type: "text/csv; charset=utf-8".into(),
            bytes: csv_bytes,
        },
    );
    downloads.insert(
        json_id.clone(),
        DownloadArtifact {
            filename: format!("transfer-report-{}.json", result.run_id),
            content_type: "application/json; charset=utf-8".into(),
            bytes: json_bytes,
        },
    );
    result.report_csv_url = Some(format!("/api/exports/{csv_id}/download"));
    result.report_json_url = Some(format!("/api/exports/{json_id}/download"));
    Ok(Json(result))
}

async fn create_transfer_run(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<models::TransferExecuteRequest>,
) -> Result<(StatusCode, Json<models::TransferRun>), ApiError> {
    if request.privacy != "private" {
        return Err(ApiError::bad_request("当前 MVP 只创建新的私有目标播放列表"));
    }
    let preview = state
        .transfer_previews
        .read()
        .await
        .get(&request.preview_id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("复制预览不存在或服务已重启"))?;
    if preview.status == "BLOCKED_EXTERNAL_AUTH" {
        return Err(ApiError::unauthorized(preview.message));
    }
    transfer::validate_transfer_execution(&preview, request.confirmed, &request.selections)
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let auth_session = destination_auth_session(&headers, &preview.destination_platform);
    let connector = transfer_connector(&state, &preview, auth_session.clone()).await?;
    let timestamp = unix_timestamp();
    let id = Uuid::new_v4().to_string();
    let run = models::TransferRun {
        id: id.clone(),
        preview_id: preview.preview_id.clone(),
        destination_platform: preview.destination_platform.clone(),
        status: "QUEUED".into(),
        processed_count: 0,
        source_count: preview.source_count,
        progress: 0.0,
        result: None,
        error: None,
        is_mock: preview.is_mock,
        revision: 1,
        created_at: timestamp,
        updated_at: timestamp,
    };
    state
        .transfer_runs
        .write()
        .map_err(ApiError::internal)?
        .insert(
            id.clone(),
            StoredTransferRun {
                public: run.clone(),
                preview,
                request,
                auth_session,
                cancelled: Arc::new(AtomicBool::new(false)),
            },
        );
    spawn_transfer_execution(state, id, connector, false);
    Ok((StatusCode::ACCEPTED, Json(run)))
}

async fn get_transfer_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<models::TransferRun>, ApiError> {
    let run = state
        .transfer_runs
        .read()
        .map_err(ApiError::internal)?
        .get(&id)
        .map(|stored| stored.public.clone())
        .ok_or_else(|| ApiError::not_found("复制任务不存在或服务已重启"))?;
    Ok(Json(run))
}

async fn cancel_transfer_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<models::TransferRun>, ApiError> {
    let mut runs = state.transfer_runs.write().map_err(ApiError::internal)?;
    let stored = runs
        .get_mut(&id)
        .ok_or_else(|| ApiError::not_found("复制任务不存在或服务已重启"))?;
    if matches!(stored.public.status.as_str(), "COMPLETED" | "CANCELLED") {
        return Ok(Json(stored.public.clone()));
    }
    stored.cancelled.store(true, Ordering::SeqCst);
    stored.public.status = "CANCELLING".into();
    stored.public.updated_at = unix_timestamp();
    stored.public.revision += 1;
    Ok(Json(stored.public.clone()))
}

async fn resume_transfer_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<models::TransferRun>, ApiError> {
    let (preview, auth_session) = {
        let mut runs = state.transfer_runs.write().map_err(ApiError::internal)?;
        let stored = runs
            .get_mut(&id)
            .ok_or_else(|| ApiError::not_found("复制任务不存在或服务已重启"))?;
        if !matches!(stored.public.status.as_str(), "FAILED" | "CANCELLED") {
            return Err(ApiError::bad_request(
                "只有 FAILED 或 CANCELLED 复制任务可以恢复",
            ));
        }
        stored.cancelled = Arc::new(AtomicBool::new(false));
        stored.public.status = "QUEUED".into();
        stored.public.error = None;
        stored.public.updated_at = unix_timestamp();
        stored.public.revision += 1;
        (stored.preview.clone(), stored.auth_session.clone())
    };
    let connector = transfer_connector(&state, &preview, auth_session).await?;
    spawn_transfer_execution(state.clone(), id.clone(), connector, true);
    get_transfer_run(State(state), Path(id)).await
}

async fn transfer_run_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    if !state
        .transfer_runs
        .read()
        .map_err(ApiError::internal)?
        .contains_key(&id)
    {
        return Err(ApiError::not_found("复制任务不存在或服务已重启"));
    }
    let runs = state.transfer_runs.clone();
    let events = async_stream::stream! {
        let mut last_revision = 0_u64;
        loop {
            let current = runs.read().ok().and_then(|items| items.get(&id).map(|stored| stored.public.clone()));
            let Some(run) = current else { break; };
            if run.revision != last_revision {
                last_revision = run.revision;
                let terminal = matches!(run.status.as_str(), "COMPLETED" | "FAILED" | "CANCELLED");
                match Event::default().event("transfer").json_data(&run) {
                    Ok(event) => yield Ok::<Event, Infallible>(event),
                    Err(_) => break,
                }
                if terminal { break; }
            }
            sleep(Duration::from_millis(250)).await;
        }
    };
    Ok(Sse::new(events).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    ))
}

async fn transfer_connector(
    state: &AppState,
    preview: &models::TransferPreview,
    auth_session: Option<String>,
) -> Result<Arc<dyn transfer::DestinationConnector>, ApiError> {
    if preview.is_mock {
        return Ok(Arc::new(transfer::MockDestinationConnector::default()));
    }
    match preview.destination_platform.as_str() {
        "youtube" => {
            if !state
                .youtube
                .connection_status(auth_session.as_deref())
                .await
                .connected
            {
                return Err(ApiError::unauthorized(
                    "YouTube OAuth 会话不存在或已过期；未创建播放列表",
                ));
            }
            Ok(Arc::new(transfer::YoutubeDestinationConnector::new(
                state.youtube.clone(),
                auth_session,
            )))
        }
        "spotify" => {
            if !state
                .spotify
                .connection_status(auth_session.as_deref())
                .await
                .connected
            {
                return Err(ApiError::unauthorized(
                    "Spotify OAuth 会话不存在或已过期；未创建播放列表",
                ));
            }
            Ok(Arc::new(transfer::SpotifyDestinationConnector::new(
                state.spotify.clone(),
                auth_session,
            )))
        }
        _ => Err(ApiError::bad_request("复制预览的目标平台没有已验证 Writer")),
    }
}

fn spawn_transfer_execution(
    state: AppState,
    id: String,
    connector: Arc<dyn transfer::DestinationConnector>,
    resume: bool,
) {
    tokio::spawn(async move {
        let Some((preview, request, cancelled, previous)) =
            state.transfer_runs.write().ok().and_then(|mut runs| {
                let stored = runs.get_mut(&id)?;
                stored.public.status = "RUNNING".into();
                stored.public.updated_at = unix_timestamp();
                stored.public.revision += 1;
                Some((
                    stored.preview.clone(),
                    stored.request.clone(),
                    stored.cancelled.clone(),
                    stored.public.result.clone(),
                ))
            })
        else {
            return;
        };
        if cancelled.load(Ordering::SeqCst) {
            update_transfer_run_terminal(&state, &id, "CANCELLED", None, None);
            return;
        }
        let progress_runs = state.transfer_runs.clone();
        let progress_id = id.clone();
        let on_progress = move |snapshot: &models::TransferResult| {
            if let Ok(mut runs) = progress_runs.write()
                && let Some(stored) = runs.get_mut(&progress_id)
            {
                let mut snapshot = snapshot.clone();
                snapshot.run_id = progress_id.clone();
                stored.public.status = snapshot.status.clone();
                stored.public.processed_count = snapshot.results.len();
                stored.public.progress = snapshot.progress;
                stored.public.result = Some(snapshot);
                stored.public.updated_at = unix_timestamp();
                stored.public.revision += 1;
            }
        };
        let outcome = if resume
            && previous
                .as_ref()
                .and_then(|item| item.playlist_id.as_ref())
                .is_some()
        {
            transfer::resume_transfer_with_progress(
                connector.as_ref(),
                &preview,
                previous.as_ref().expect("checked above"),
                request.confirmed,
                &request.selections,
                cancelled.as_ref(),
                Some(&on_progress),
            )
            .await
        } else {
            transfer::execute_transfer_with_progress(
                connector.as_ref(),
                &preview,
                request.confirmed,
                &request.selections,
                cancelled.as_ref(),
                Some(&on_progress),
            )
            .await
        };
        match outcome {
            Ok(mut result) => {
                result.run_id = id.clone();
                if let Err(error) = attach_transfer_reports(&state, &mut result).await {
                    tracing::warn!(error = %error, "failed to prepare transfer report");
                }
                let status = result.status.clone();
                update_transfer_run_terminal(&state, &id, &status, Some(result), None);
            }
            Err(error) => {
                update_transfer_run_terminal(
                    &state,
                    &id,
                    "FAILED",
                    previous,
                    Some(error.to_string()),
                );
            }
        }
    });
}

fn update_transfer_run_terminal(
    state: &AppState,
    id: &str,
    status: &str,
    result: Option<models::TransferResult>,
    error: Option<String>,
) {
    if let Ok(mut runs) = state.transfer_runs.write()
        && let Some(stored) = runs.get_mut(id)
    {
        stored.public.status = status.into();
        if let Some(result) = result {
            stored.public.processed_count = result.results.len();
            stored.public.progress = result.progress;
            stored.public.result = Some(result);
        }
        stored.public.error = error;
        stored.public.updated_at = unix_timestamp();
        stored.public.revision += 1;
    }
}

async fn attach_transfer_reports(
    state: &AppState,
    result: &mut models::TransferResult,
) -> anyhow::Result<()> {
    let csv_id = Uuid::new_v4().to_string();
    let json_id = Uuid::new_v4().to_string();
    let csv_bytes = transfer_report_csv(result)?;
    let json_bytes = serde_json::to_vec_pretty(result)?;
    let mut downloads = state.downloads.write().await;
    downloads.insert(
        csv_id.clone(),
        DownloadArtifact {
            filename: format!("copy-report-{}.csv", result.run_id),
            content_type: "text/csv; charset=utf-8".into(),
            bytes: csv_bytes,
        },
    );
    downloads.insert(
        json_id.clone(),
        DownloadArtifact {
            filename: format!("copy-report-{}.json", result.run_id),
            content_type: "application/json; charset=utf-8".into(),
            bytes: json_bytes,
        },
    );
    result.report_csv_url = Some(format!("/api/exports/{csv_id}/download"));
    result.report_json_url = Some(format!("/api/exports/{json_id}/download"));
    Ok(())
}

fn destination_auth_session(headers: &HeaderMap, destination: &str) -> Option<String> {
    match destination {
        "youtube" => youtube_auth_session(headers),
        "spotify" => auth_session(headers),
        _ => None,
    }
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn transfer_report_csv(result: &models::TransferResult) -> anyhow::Result<Vec<u8>> {
    let mut writer = csv::Writer::from_writer(Vec::new());
    writer.write_record([
        "source_title",
        "source_artists",
        "target_id",
        "status",
        "error",
    ])?;
    for item in &result.results {
        let artists = item.source_track.artists.join("; ");
        writer.write_record([
            item.source_track.title.as_str(),
            artists.as_str(),
            item.target_id.as_deref().unwrap_or(""),
            item.status.as_str(),
            item.error.as_deref().unwrap_or(""),
        ])?;
    }
    Ok(writer.into_inner()?)
}

async fn get_settings(State(state): State<AppState>) -> Json<models::AgentSettings> {
    Json(state.agent.settings().await)
}

async fn update_settings(
    State(state): State<AppState>,
    Json(settings): Json<models::AgentSettings>,
) -> Result<Json<models::AgentSettings>, ApiError> {
    Ok(Json(state.agent.update_settings(settings).await.map_err(
        |error| ApiError::bad_request(error.to_string()),
    )?))
}

async fn create_task(
    State(state): State<AppState>,
    Json(request): Json<models::CreateAgentTaskRequest>,
) -> Result<(StatusCode, Json<models::AgentTask>), ApiError> {
    let task = state
        .agent
        .create_task(request)
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok((StatusCode::CREATED, Json(task)))
}

async fn list_tasks(
    State(state): State<AppState>,
) -> Result<Json<Vec<models::AgentTask>>, ApiError> {
    Ok(Json(
        state.agent.list_tasks().await.map_err(ApiError::internal)?,
    ))
}

async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<models::AgentTask>, ApiError> {
    let task = state
        .agent
        .get_task(&id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("任务不存在"))?;
    Ok(Json(task))
}

async fn cancel_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<models::AgentTask>, ApiError> {
    Ok(Json(state.agent.cancel(&id).await.map_err(|error| {
        ApiError::bad_request(error.to_string())
    })?))
}

async fn resume_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<models::AgentTask>, ApiError> {
    Ok(Json(state.agent.resume(&id).await.map_err(|error| {
        ApiError::bad_request(error.to_string())
    })?))
}

async fn task_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    if state
        .agent
        .get_task(&id)
        .await
        .map_err(ApiError::internal)?
        .is_none()
    {
        return Err(ApiError::not_found("任务不存在"));
    }
    let agent = state.agent.clone();
    let events = async_stream::stream! {
        let mut last_revision = -1_i64;
        loop {
            match agent.get_task(&id).await {
                Ok(Some(task)) => {
                    if task.revision != last_revision {
                        last_revision = task.revision;
                        let terminal = matches!(task.status.as_str(), "COMPLETED" | "FAILED" | "CANCELLED" | "BLOCKED_EXTERNAL_AUTH");
                        match Event::default().event("task").json_data(&task) {
                            Ok(event) => yield Ok::<Event, Infallible>(event),
                            Err(_) => break,
                        }
                        if terminal { break; }
                    }
                }
                _ => break,
            }
            sleep(Duration::from_millis(350)).await;
        }
    };
    Ok(Sse::new(events).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(10))
            .text("keep-alive"),
    ))
}

fn named_auth_session(headers: &HeaderMap, cookie_name: &str) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|pair| {
            let (key, value) = pair.trim().split_once('=')?;
            (key == cookie_name).then(|| value.to_string())
        })
}

fn auth_session(headers: &HeaderMap) -> Option<String> {
    named_auth_session(headers, "melody_spotify_session")
}

fn youtube_auth_session(headers: &HeaderMap) -> Option<String> {
    named_auth_session(headers, "melody_youtube_session")
}

async fn writer_statuses(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<WriterStatus>>, ApiError> {
    let session = auth_session(&headers);
    let mut statuses = Vec::new();
    statuses.push(
        state
            .spotify
            .authorize(session.as_deref())
            .await
            .map_err(ApiError::internal)?,
    );
    statuses.push(
        state
            .youtube
            .authorize(youtube_auth_session(&headers).as_deref())
            .await
            .map_err(ApiError::internal)?,
    );
    statuses.push(
        ExportFilePlaylistWriter
            .authorize(None)
            .await
            .map_err(ApiError::internal)?,
    );
    statuses.push(
        DemoPlaylistWriter
            .authorize(None)
            .await
            .map_err(ApiError::internal)?,
    );
    for writer in unavailable_writers() {
        statuses.push(writer.authorize(None).await.map_err(ApiError::internal)?);
    }
    Ok(Json(statuses))
}

async fn platform_capabilities(
    State(state): State<AppState>,
) -> Json<Vec<models::PlatformCapability>> {
    Json(state.platforms.capabilities(
        state.spotify.is_configured(),
        state.youtube.is_configured(),
        state.apple.is_configured(),
    ))
}

async fn spotify_configuration(
    State(state): State<AppState>,
) -> Json<models::ProviderConfigurationStatus> {
    Json(state.spotify.configuration_status())
}

async fn check_spotify_configuration(
    State(state): State<AppState>,
) -> Json<models::ProviderConfigurationStatus> {
    Json(state.spotify.validate_configuration().await)
}

async fn youtube_configuration(
    State(state): State<AppState>,
) -> Json<models::ProviderConfigurationStatus> {
    Json(state.youtube.configuration_status())
}

async fn check_youtube_configuration(
    State(state): State<AppState>,
) -> Json<models::ProviderConfigurationStatus> {
    Json(state.youtube.validate_configuration().await)
}

async fn apple_configuration(
    State(state): State<AppState>,
) -> Json<models::ProviderConfigurationStatus> {
    Json(state.apple.configuration_status())
}

async fn apple_musickit_bootstrap(
    State(state): State<AppState>,
) -> Json<models::AppleMusicBootstrap> {
    Json(state.apple.bootstrap())
}

async fn inspect_playlist_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<PlaylistLinkRequest>,
) -> Result<Json<models::PlaylistLinkInspection>, ApiError> {
    use models::PublicLinkCapability;
    if request.url.trim().len() > 2_048 {
        return Err(ApiError::bad_request("链接过长"));
    }
    let mut result = state
        .platforms
        .inspect_link(&request.url)
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    if result.platform.as_deref() == Some("apple_music") && result.playlist_id_valid {
        if !state.apple.is_configured() {
            result.capability = PublicLinkCapability::ConfigRequired;
            result.message = "CONFIG_REQUIRED · WAITING_FOR_APPLE_DEVELOPER_CREDENTIALS；公开目录读取需要部署者配置 Developer Token。".into();
            result.next_step =
                "普通用户可继续使用文件或文本；部署者可查看 Apple 配置向导，无需现在付费。".into();
            return Ok(Json(result));
        }
        match state.apple.import_playlist_link(&request.url).await {
            Ok(imported) => {
                result.playlist_name = imported.name;
                result.track_count = Some(imported.tracks.len());
                result.preview_tracks = imported.tracks;
                result.import_rows = imported.rows;
                result.capability = if result.preview_tracks.is_empty() {
                    PublicLinkCapability::PublicMetadataAvailable
                } else {
                    PublicLinkCapability::TrackImportAvailable
                };
                result.access_status = "official_api_read".into();
                result.structured_data_status = "official_api".into();
                result.message = format!(
                    "Apple 官方目录已完成分页：{} 个源条目，{} 首可导入。重复、缺少艺人和不可用元数据见逐项报告；未返回的曲目无法恢复。",
                    result.import_rows.len(),
                    result.preview_tracks.len()
                );
                result.next_step = "核对 Import Preview 后进入既有 Copy 确认流程；私人资料库未接入，未创建或写入任何歌单。".into();
            }
            Err(error) => {
                let code = error.to_string();
                result.capability = if matches!(
                    code.as_str(),
                    "APPLE_HTTP_401" | "APPLE_HTTP_403" | "WAITING_FOR_APPLE_DEVELOPER_CREDENTIALS"
                ) {
                    PublicLinkCapability::ConfigRequired
                } else {
                    PublicLinkCapability::UrlRecognitionOnly
                };
                result.access_status = "official_api_read_failed".into();
                // Only locally generated error codes; never expose API body or credentials.
                result.message = match code.as_str() {
                    "APPLE_HTTP_401" | "APPLE_HTTP_403" => {
                        "Apple 拒绝凭据或访问权限；请部署者检查 Token 有效期与资格。"
                    }
                    "APPLE_HTTP_404" | "APPLE_PLAYLIST_NOT_FOUND" => {
                        "该地区目录未找到歌单；确认公开状态与链接地区。私人资料库未接入。"
                    }
                    "APPLE_HTTP_429" => "Apple API 限流；请稍后重试。",
                    _ => "Apple 官方读取失败、响应异常或超出限制；未返回部分结果或替代歌曲。",
                }
                .into();
                result.next_step = "检查官方配置和公开 URL 后重试，或使用文件/文本导入。".into();
            }
        }
        return Ok(Json(result));
    }
    if result.capability != PublicLinkCapability::AuthRequired {
        return Ok(Json(result));
    }
    let id = result.playlist_id.as_deref().unwrap_or_default();
    let spotify = result.platform.as_deref() == Some("spotify");
    let session = if spotify {
        auth_session(&headers)
    } else {
        youtube_auth_session(&headers)
    };
    let connected = if spotify {
        state
            .spotify
            .connection_status(session.as_deref())
            .await
            .connected
    } else {
        state
            .youtube
            .connection_status(session.as_deref())
            .await
            .connected
    };
    if !connected {
        result.message =
            "AUTH_REQUIRED：未连接平台或 OAuth session 已过期，请先通过官方页面重新授权。".into();
        return Ok(Json(result));
    }
    let imported = if spotify {
        state
            .spotify
            .import_playlist_link(id, session.as_deref())
            .await
            .map(|r| (r.playlists.first().map(|p| p.name.clone()), r.tracks))
    } else {
        state
            .youtube
            .import_playlist_link(id, session.as_deref())
            .await
            .map(|r| (r.playlists.first().map(|p| p.name.clone()), r.tracks))
    };
    match imported {
        Ok((name, tracks)) => {
            result.import_rows = tracks
                .iter()
                .map(|track| models::PlaylistImportRow {
                    source_platform: track.platform.clone(),
                    playlist_id: id.to_owned(),
                    playlist_name: name.clone(),
                    track_title: Some(track.title.clone()),
                    artist: (!track.artists.is_empty()).then(|| track.artists.clone()),
                    duration_ms: track.duration_ms,
                    source_url: track.platform_url.clone(),
                    availability: "UNKNOWN".into(),
                    import_status: "IMPORTED".into(),
                })
                .collect();
            result.capability = if tracks.is_empty() {
                PublicLinkCapability::PublicMetadataAvailable
            } else {
                PublicLinkCapability::TrackImportAvailable
            };
            result.playlist_name = name;
            result.track_count = Some(tracks.len());
            result.preview_tracks = tracks;
            result.access_status = "official_api_read".into();
            result.structured_data_status = "official_api".into();
            result.message =
                "官方 API 已返回 Import Preview；仅用于用户确认后的传输，不进入画像、推荐或 LLM。"
                    .into();
            result.next_step =
                "核对曲目后可进入 Copy Playlist 预览；此操作尚未创建或写入任何播放列表。".into();
            // Platform API content must never go to the local analysis/LLM pipeline.
            result.can_analyze = false;
        }
        Err(error) => {
            let expired = error
                .chain()
                .filter_map(|cause| cause.downcast_ref::<reqwest::Error>())
                .any(|cause| cause.status() == Some(reqwest::StatusCode::UNAUTHORIZED));
            result.capability = if expired {
                PublicLinkCapability::AuthRequired
            } else {
                PublicLinkCapability::UrlRecognitionOnly
            };
            result.access_status = "official_api_read_failed".into();
            result.message = "无法通过官方 API 读取该 playlist，请确认账号权限、播放列表可访问性和 API 额度；没有返回替代歌曲。".into();
            result.next_step =
                "检查权限后重试；若会话失效，请重新连接。也可使用本地文件导入。".into();
            if expired {
                result.message =
                    "AUTH_REQUIRED：OAuth session 已失效，请重新连接后再读取歌单。".into();
            }
        }
    }
    Ok(Json(result))
}

async fn spotify_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Json<models::SpotifyConnectionStatus> {
    Json(
        state
            .spotify
            .connection_status(auth_session(&headers).as_deref())
            .await,
    )
}

async fn spotify_playlists(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<models::SpotifyPlaylistSummary>>, ApiError> {
    let session = auth_session(&headers)
        .ok_or_else(|| ApiError::unauthorized("请先通过 Spotify 官方页面授权"))?;
    Ok(Json(
        state
            .spotify
            .list_playlists(Some(&session))
            .await
            .map_err(ApiError::internal)?,
    ))
}

async fn spotify_import(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<SpotifyImportRequest>,
) -> Result<Json<models::SpotifyImportResult>, ApiError> {
    let session = auth_session(&headers)
        .ok_or_else(|| ApiError::unauthorized("请先通过 Spotify 官方页面授权"))?;
    Ok(Json(
        state
            .spotify
            .import_playlists(&request.playlist_ids, Some(&session))
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?,
    ))
}

async fn spotify_disconnect(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let removed = state
        .spotify
        .disconnect(auth_session(&headers).as_deref())
        .await;
    let mut response = Json(serde_json::json!({
        "disconnected": true,
        "local_token_deleted": removed,
        "message": "本地 Spotify token 已删除；如需撤销应用授权，也可在 Spotify 账号的应用管理中操作。"
    }))
    .into_response();
    if let Ok(cookie) = HeaderValue::from_str(&session_cookie("melody_spotify_session", "", 0)) {
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    Ok(response)
}

async fn youtube_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Json<models::YoutubeConnectionStatus> {
    let session = youtube_auth_session(&headers);
    let status = state.youtube.connection_status(session.as_deref()).await;
    tracing::info!(
        provider = "youtube",
        oauth_stage = "session_status",
        session_cookie_present = session.is_some(),
        connected = status.connected
    );
    Json(status)
}

async fn youtube_playlists(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Vec<models::YoutubePlaylistSummary>>, ApiError> {
    let session = youtube_auth_session(&headers)
        .ok_or_else(|| ApiError::unauthorized("请先通过 Google 官方页面授权 YouTube"))?;
    Ok(Json(
        state
            .youtube
            .list_playlists(Some(&session))
            .await
            .map_err(ApiError::internal)?,
    ))
}

async fn youtube_import(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<YoutubeImportRequest>,
) -> Result<Json<models::YoutubeImportResult>, ApiError> {
    let session = youtube_auth_session(&headers)
        .ok_or_else(|| ApiError::unauthorized("请先通过 Google 官方页面授权 YouTube"))?;
    Ok(Json(
        state
            .youtube
            .import_playlists(&request.playlist_ids, Some(&session))
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?,
    ))
}

async fn youtube_disconnect(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let removed = state
        .youtube
        .disconnect(youtube_auth_session(&headers).as_deref())
        .await;
    let mut response = Json(serde_json::json!({
        "disconnected": true,
        "local_token_deleted": removed,
        "message": "本地 Google/YouTube token 已删除；如需撤销授权，也可前往 Google 账号的第三方应用管理。"
    }))
    .into_response();
    if let Ok(cookie) = HeaderValue::from_str(&session_cookie("melody_youtube_session", "", 0)) {
        response.headers_mut().insert(header::SET_COOKIE, cookie);
    }
    Ok(response)
}

async fn youtube_authorize(
    State(state): State<AppState>,
    Query(request): Query<OAuthStart>,
) -> Response {
    match state.youtube.begin_authorization_for(request.write).await {
        Ok(url) => oauth_start_redirect(&url, "youtube"),
        Err(_) => oauth_redirect(
            &state.youtube.frontend_url(),
            "youtube",
            "error",
            "config_required",
        ),
    }
}

async fn youtube_callback(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SpotifyCallback>,
) -> Response {
    let frontend = app.youtube.frontend_url();
    tracing::info!(
        provider = "youtube",
        oauth_stage = "callback_received",
        code_present = query.code.as_ref().is_some_and(|v| !v.is_empty())
    );
    if let Some(error) = query.error.as_deref() {
        return oauth_redirect(
            &frontend,
            "youtube",
            "error",
            if error == "access_denied" {
                "authorization_cancelled"
            } else {
                "provider_error"
            },
        );
    }
    let Some(code) = query.code.as_deref() else {
        return oauth_redirect(&frontend, "youtube", "error", "missing_code");
    };
    let Some(state) = query.state.as_deref() else {
        return oauth_redirect(&frontend, "youtube", "error", "missing_state");
    };
    if !oauth_browser_state_matches(&headers, "youtube", state) {
        return oauth_redirect(&frontend, "youtube", "error", "state_mismatch");
    }
    let (session, frontend) = match app.youtube.complete_authorization(code, state).await {
        Ok(result) => result,
        Err(error) => {
            let reason = error
                .downcast_ref::<writers::youtube::YoutubeOAuthFailure>()
                .map(|failure| failure.0)
                .unwrap_or_else(|| oauth_error_reason(&error));
            return oauth_redirect(&frontend, "youtube", "error", reason);
        }
    };
    let cookie = session_cookie("melody_youtube_session", &session, 28_800);
    let mut response = oauth_redirect(&frontend, "youtube", "connected", "success");
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}

async fn spotify_authorize(
    State(state): State<AppState>,
    Query(request): Query<OAuthStart>,
) -> Response {
    match state.spotify.begin_authorization_for(request.write).await {
        Ok(url) => oauth_start_redirect(&url, "spotify"),
        Err(_) => oauth_redirect(
            &state.spotify.frontend_url(),
            "spotify",
            "error",
            "config_required",
        ),
    }
}

#[derive(Debug, Deserialize)]
struct OAuthStart {
    #[serde(default)]
    write: bool,
}

#[derive(Debug, Deserialize)]
struct SpotifyCallback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn spotify_callback(
    State(app): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<SpotifyCallback>,
) -> Response {
    let frontend = app.spotify.frontend_url();
    tracing::info!(
        provider = "spotify",
        oauth_stage = "callback_received",
        code_present = query.code.as_ref().is_some_and(|v| !v.is_empty())
    );
    if let Some(error) = query.error.as_deref() {
        return oauth_redirect(
            &frontend,
            "spotify",
            "error",
            if error == "access_denied" {
                "authorization_cancelled"
            } else {
                "provider_error"
            },
        );
    }
    let Some(code) = query.code.as_deref() else {
        return oauth_redirect(&frontend, "spotify", "error", "missing_code");
    };
    let Some(state) = query.state.as_deref() else {
        return oauth_redirect(&frontend, "spotify", "error", "missing_state");
    };
    if !oauth_browser_state_matches(&headers, "spotify", state) {
        return oauth_redirect(&frontend, "spotify", "error", "state_mismatch");
    }
    let (session, frontend) = match app.spotify.complete_authorization(code, state).await {
        Ok(result) => result,
        Err(error) => {
            return oauth_redirect(&frontend, "spotify", "error", oauth_error_reason(&error));
        }
    };
    let cookie = session_cookie("melody_spotify_session", &session, 28_800);
    let mut response = oauth_redirect(&frontend, "spotify", "connected", "success");
    if let Ok(value) = HeaderValue::from_str(&cookie) {
        response.headers_mut().append(header::SET_COOKIE, value);
    }
    response
}

fn oauth_browser_state_matches(headers: &HeaderMap, provider: &str, state: &str) -> bool {
    !state.is_empty()
        && named_auth_session(headers, &format!("melody_{provider}_oauth_state"))
            .is_some_and(|expected| expected == state)
}

fn oauth_start_redirect(url: &str, provider: &str) -> Response {
    let state = reqwest::Url::parse(url).ok().and_then(|url| {
        url.query_pairs()
            .find(|(key, _)| key == "state")
            .map(|(_, value)| value.into_owned())
    });
    let Some(state) = state else {
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    };
    let mut response = Redirect::temporary(url).into_response();
    if let Ok(cookie) = HeaderValue::from_str(&session_cookie(
        &format!("melody_{provider}_oauth_state"),
        &state,
        600,
    )) {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

fn oauth_error_reason(error: &anyhow::Error) -> &'static str {
    let message = error.to_string();
    if message.contains("state") {
        "state_mismatch"
    } else if message.contains("未配置") {
        "config_required"
    } else if message.contains("权限") {
        "permission_denied"
    } else if message.contains("频道") {
        "channel_required"
    } else {
        "authorization_failed"
    }
}

fn oauth_redirect(frontend: &str, provider: &str, status: &str, reason: &str) -> Response {
    tracing::info!(provider, oauth_stage = "callback_redirect", status, reason);
    let fallback = "http://127.0.0.1:5173/";
    let mut url = reqwest::Url::parse(frontend)
        .or_else(|_| reqwest::Url::parse(fallback))
        .expect("static fallback URL is valid");
    url.set_path("/");
    url.set_query(None);
    url.query_pairs_mut()
        .append_pair("oauth", status)
        .append_pair("provider", provider)
        .append_pair("reason", reason);
    let mut response = Redirect::temporary(url.as_str()).into_response();
    if let Ok(cookie) = HeaderValue::from_str(&session_cookie(
        &format!("melody_{provider}_oauth_state"),
        "",
        0,
    )) {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

fn session_cookie(name: &str, value: &str, max_age: u32) -> String {
    let secure = std::env::var("OAUTH_COOKIE_SECURE")
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes"
            )
        })
        || std::env::var("PUBLIC_BASE_URL")
            .ok()
            .is_some_and(|value| value.trim().to_ascii_lowercase().starts_with("https://"));
    format!(
        "{name}={value}; HttpOnly; SameSite=Lax; Path=/; Max-Age={max_age}{}",
        if secure { "; Secure" } else { "" }
    )
}

fn writer_for(state: &AppState, platform: &str) -> Option<Box<dyn PlaylistWriter>> {
    match platform {
        "spotify" => Some(Box::new(state.spotify.clone())),
        "youtube" => Some(Box::new(state.youtube.clone())),
        "file" => Some(Box::new(ExportFilePlaylistWriter)),
        "demo" => Some(Box::new(DemoPlaylistWriter)),
        "apple_music" => Some(Box::new(UnavailablePlaylistWriter {
            platform_name: "apple_music",
            display_name: "Apple Music",
            reason: "Apple Music 真写入尚未启用；请选择导出歌曲清单。",
        })),
        "netease" => Some(Box::new(UnavailablePlaylistWriter {
            platform_name: "netease",
            display_name: "网易云音乐",
            reason: "网易云音乐没有在本项目中配置合规写入 API。",
        })),
        "qq_music" => Some(Box::new(UnavailablePlaylistWriter {
            platform_name: "qq_music",
            display_name: "QQ 音乐",
            reason: "QQ 音乐没有在本项目中配置合规写入 API。",
        })),
        _ => None,
    }
}

async fn export_preview(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ExportPreviewRequest>,
) -> Result<Json<ExportPreview>, ApiError> {
    if request.playlist_name.trim().is_empty() {
        return Err(ApiError::bad_request("目标歌单名称不能为空"));
    }
    if request.tracks.is_empty() {
        return Err(ApiError::bad_request("请至少选择一首歌曲"));
    }
    if request.tracks.len() > 500 {
        return Err(ApiError::bad_request("单次最多处理 500 首歌曲"));
    }
    let writer = writer_for(&state, &request.platform)
        .ok_or_else(|| ApiError::bad_request("未知的目标平台"))?;
    let session = auth_session(&headers);
    let status = writer
        .authorize(session.as_deref())
        .await
        .map_err(ApiError::internal)?;
    if status.availability != "available" {
        return Err(ApiError::bad_request(status.message));
    }
    if request.platform == "spotify" && !status.authorized {
        return Err(ApiError::unauthorized(
            "请先通过 Spotify OAuth 授权；不会要求输入平台密码。",
        ));
    }

    let matches = if request.platform == "spotify" {
        export::spotify_playlist::SpotifyPlaylistExportService::new(writer.as_ref())
            .match_tracks(&request.tracks, session.as_deref())
            .await
            .map_err(ApiError::internal)?
    } else {
        let mut matches = Vec::with_capacity(request.tracks.len());
        for track in &request.tracks {
            matches.push(
                writer
                    .search_track(track, session.as_deref())
                    .await
                    .map_err(ApiError::internal)?,
            );
        }
        matches
    };
    let auto_matched_count = matches
        .iter()
        .filter(|m| m.status == MatchStatus::Matched)
        .count();
    let needs_confirmation_count = matches
        .iter()
        .filter(|m| m.status == MatchStatus::NeedsConfirmation)
        .count();
    let unmatched_count = matches
        .iter()
        .filter(|m| m.status == MatchStatus::Unmatched)
        .count();
    let preview = ExportPreview {
        preview_id: Uuid::new_v4().to_string(),
        platform: request.platform,
        playlist_name: request.playlist_name,
        requested_count: matches.len(),
        auto_matched_count,
        needs_confirmation_count,
        unmatched_count,
        requires_explicit_confirmation: true,
        is_demo: writer.platform() == "demo",
        disclosure: (writer.platform() == "demo")
            .then(|| "Demo 写入只生成虚拟歌单，不会修改真实平台账号。".into()),
        matches,
    };
    state.previews.write().await.insert(
        preview.preview_id.clone(),
        StoredPreview {
            preview: preview.clone(),
            format: request.format,
        },
    );
    Ok(Json(preview))
}

async fn export_execute(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ExportExecuteRequest>,
) -> Result<Json<PlaylistExportResult>, ApiError> {
    if !request.confirmed {
        return Err(ApiError::bad_request("写入前必须查看预览并明确确认"));
    }
    let stored = state
        .previews
        .write()
        .await
        .remove(&request.preview_id)
        .ok_or_else(|| ApiError::not_found("预览不存在或已经执行；请重新生成预览"))?;
    let writer = writer_for(&state, &stored.preview.platform)
        .ok_or_else(|| ApiError::bad_request("未知平台"))?;
    let session = auth_session(&headers);
    let status = writer
        .authorize(session.as_deref())
        .await
        .map_err(ApiError::internal)?;
    if stored.preview.platform == "spotify" && !status.authorized {
        return Err(ApiError::unauthorized(
            "Spotify 授权已失效，请重新授权后再次预览",
        ));
    }

    let mut ready: Vec<PlatformTrackMatch> = Vec::new();
    let mut ambiguous = Vec::new();
    let mut failed = Vec::new();
    for mut matched in stored.preview.matches {
        match matched.status {
            MatchStatus::Matched => ready.push(matched),
            MatchStatus::NeedsConfirmation => {
                if let Some(selected_id) = request.selections.get(&matched.source_track.id) {
                    if let Some(candidate) = matched
                        .candidates
                        .iter()
                        .find(|candidate| {
                            &candidate.target_track_id == selected_id
                                && candidate.available_in_market
                        })
                        .cloned()
                    {
                        matched.target_track_id = Some(candidate.target_track_id.clone());
                        matched.target_url = candidate.target_url.clone();
                        matched.version_type = candidate.version_type;
                        matched.confidence = candidate.confidence;
                        matched.match_reason =
                            format!("用户在预览中确认：{}", candidate.match_reason);
                        matched.status = MatchStatus::Selected;
                        ready.push(matched);
                    } else {
                        failed.push(TrackFailure {
                            track: matched.source_track.clone(),
                            reason: "选择的候选无效或在目标地区不可播放".into(),
                        });
                    }
                } else {
                    ambiguous.push(matched);
                }
            }
            MatchStatus::Unmatched => failed.push(TrackFailure {
                track: matched.source_track.clone(),
                reason: matched.match_reason.clone(),
            }),
            MatchStatus::Selected => ready.push(matched),
        }
    }

    let created = writer
        .create_playlist(&stored.preview.playlist_name, session.as_deref())
        .await
        .map_err(ApiError::internal)?;
    let track_ids: Vec<String> = ready
        .iter()
        .filter_map(|m| m.target_track_id.clone())
        .collect();
    let outcome = writer
        .add_tracks(&created.id, &track_ids, session.as_deref())
        .await
        .map_err(ApiError::internal)?;
    let added_set: std::collections::HashSet<_> = outcome.added_ids.iter().collect();
    let mut successful = Vec::new();
    for matched in ready {
        if matched
            .target_track_id
            .as_ref()
            .is_some_and(|id| added_set.contains(id))
        {
            successful.push(matched);
        } else {
            let reason = matched
                .target_track_id
                .as_ref()
                .and_then(|id| outcome.failures.get(id))
                .cloned()
                .unwrap_or_else(|| "未加入目标歌单".into());
            failed.push(TrackFailure {
                track: matched.source_track,
                reason,
            });
        }
    }
    let export_id = Uuid::new_v4().to_string();
    let mut playlist_url = writer
        .get_playlist_url(&created, session.as_deref())
        .await
        .map_err(ApiError::internal)?;
    if stored.preview.platform == "file" {
        let format = stored.format.as_deref().unwrap_or("csv");
        let artifact = build_file_artifact(&stored.preview.playlist_name, format, &successful)
            .map_err(ApiError::bad_request)?;
        state
            .downloads
            .write()
            .await
            .insert(export_id.clone(), artifact);
        playlist_url = Some(format!("/api/exports/{export_id}/download"));
    }
    let result = PlaylistExportResult {
        playlist_name: stored.preview.playlist_name,
        platform: stored.preview.platform.clone(),
        playlist_url,
        requested_count: stored.preview.requested_count,
        added_count: successful.len(),
        failed_count: failed.len(),
        needs_confirmation_count: ambiguous.len(),
        successful_tracks: successful,
        failed_tracks: failed,
        ambiguous_tracks: ambiguous,
        is_demo: stored.preview.platform == "demo",
        disclosure: (stored.preview.platform == "demo")
            .then(|| "这是 Demo 虚拟写入结果，不是真实音乐平台歌单。".into()),
    };
    Ok(Json(result))
}

fn safe_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "melodypath_playlist".into()
    } else {
        cleaned
    }
}

fn csv_cell(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn build_file_artifact(
    name: &str,
    format: &str,
    tracks: &[PlatformTrackMatch],
) -> Result<DownloadArtifact, String> {
    let base = safe_filename(name);
    match format {
        "csv" => {
            let mut output = String::from(
                "title,artists,album,genres,release_year,source_platform,source_url\n",
            );
            for item in tracks {
                let t = &item.source_track;
                output.push_str(&format!(
                    "{},{},{},{},{},{},{}\n",
                    csv_cell(&t.title),
                    csv_cell(&t.artists.join("; ")),
                    csv_cell(t.album.as_deref().unwrap_or("")),
                    csv_cell(&t.genres.join("; ")),
                    t.release_year.map(|v| v.to_string()).unwrap_or_default(),
                    csv_cell(&t.platform),
                    csv_cell(t.platform_url.as_deref().unwrap_or(""))
                ));
            }
            Ok(DownloadArtifact {
                filename: format!("{base}.csv"),
                content_type: "text/csv; charset=utf-8".into(),
                bytes: [vec![0xEF, 0xBB, 0xBF], output.into_bytes()].concat(),
            })
        }
        "json" => {
            let values: Vec<_> = tracks.iter().map(|m| &m.source_track).collect();
            let bytes = serde_json::to_vec_pretty(&serde_json::json!({ "playlist_name": name, "exported_by": "MelodyPath", "tracks": values })).map_err(|e| e.to_string())?;
            Ok(DownloadArtifact {
                filename: format!("{base}.json"),
                content_type: "application/json; charset=utf-8".into(),
                bytes,
            })
        }
        "m3u" => {
            let mut output = String::from("#EXTM3U\n");
            for item in tracks {
                let t = &item.source_track;
                output.push_str(&format!(
                    "#EXTINF:{},{} - {}\n{}\n",
                    t.duration_ms.map(|v| v / 1000).unwrap_or(0),
                    t.artists.join(", "),
                    t.title,
                    t.platform_url.as_deref().unwrap_or("")
                ));
            }
            Ok(DownloadArtifact {
                filename: format!("{base}.m3u8"),
                content_type: "audio/x-mpegurl; charset=utf-8".into(),
                bytes: output.into_bytes(),
            })
        }
        _ => Err("导出格式仅支持 csv、json 或 m3u".into()),
    }
}

async fn download_export(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let artifact = state
        .downloads
        .read()
        .await
        .get(&id)
        .cloned()
        .ok_or_else(|| ApiError::not_found("导出文件不存在或服务已重启"))?;
    let mut response = Response::new(Body::from(artifact.bytes));
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&artifact.content_type).map_err(ApiError::internal)?,
    );
    response.headers_mut().insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!(
            "attachment; filename*=UTF-8''{}",
            urlencoding::encode(&artifact.filename)
        ))
        .map_err(ApiError::internal)?,
    );
    Ok(response)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::to_bytes;
    use tower::ServiceExt;

    async fn test_app() -> Router {
        app(AppState {
            previews: Arc::new(RwLock::new(HashMap::new())),
            downloads: Arc::new(RwLock::new(HashMap::new())),
            spotify: SpotifyPlaylistWriter::new(),
            youtube: YoutubePlaylistWriter::new(),
            apple: AppleMusicConnector::default(),
            platforms: platforms::PlatformService::new(),
            agent: agent::AgentService::in_memory().await.unwrap(),
            metadata: metadata::MetadataService::new(),
            analyses: Arc::new(RwLock::new(HashMap::new())),
            imports: Arc::new(RwLock::new(HashMap::new())),
            transfer_previews: Arc::new(RwLock::new(HashMap::new())),
            transfer_runs: Arc::new(StdRwLock::new(HashMap::new())),
        })
    }

    #[tokio::test]
    async fn health_endpoint_works() {
        let response = test_app()
            .await
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn oauth_callback_without_code_returns_to_frontend_with_safe_error() {
        for path in ["/api/spotify/callback", "/api/youtube/callback"] {
            let response = test_app()
                .await
                .oneshot(
                    axum::http::Request::builder()
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert!(response.status().is_redirection());
            let location = response
                .headers()
                .get(header::LOCATION)
                .unwrap()
                .to_str()
                .unwrap();
            assert!(location.contains("oauth=error"));
            assert!(location.contains("reason=missing_code"));
            assert!(!location.contains("token"));
        }
    }

    #[tokio::test]
    async fn execute_requires_explicit_confirmation() {
        let body = serde_json::json!({ "preview_id": "missing", "confirmed": false });
        let response = test_app()
            .await
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/exports/execute")
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = to_bytes(response.into_body(), 4096).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("明确确认"));
    }

    #[tokio::test]
    async fn transfer_run_is_sessionized_and_queryable() {
        let router = test_app().await;
        let track = crate::demo::demo_playlists()[0].tracks[0].clone();
        let preview_request = serde_json::json!({
            "playlist_name": "Session test",
            "tracks": [track],
            "destination_platform": "youtube",
            "allow_alternate_versions": false,
            "use_mock": true
        });
        let response = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/transfers/preview")
                    .header("content-type", "application/json")
                    .body(Body::from(preview_request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let preview: models::TransferPreview = serde_json::from_slice(&body).unwrap();
        assert!(preview.is_mock);
        assert_eq!(preview.status, "MOCK_VERIFIED");
        assert!(!String::from_utf8_lossy(&body).contains("REAL_VERIFIED"));
        let execute_request = serde_json::json!({
            "preview_id": preview.preview_id,
            "confirmed": true,
            "selections": {},
            "privacy": "private"
        });
        let response = router
            .clone()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/api/transfers/runs")
                    .header("content-type", "application/json")
                    .body(Body::from(execute_request.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let created: models::TransferRun = serde_json::from_slice(&body).unwrap();

        let mut completed = None;
        for _ in 0..40 {
            let response = router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .uri(format!("/api/transfers/runs/{}", created.id))
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let current: models::TransferRun = serde_json::from_slice(&body).unwrap();
            if current.status == "COMPLETED" {
                completed = Some(current);
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
        let completed = completed.expect("mock transfer run should complete");
        assert_eq!(completed.processed_count, 1);
        assert_eq!(completed.progress, 1.0);
        assert!(
            completed
                .result
                .as_ref()
                .is_some_and(|result| result.is_mock)
        );
        assert!(completed.result.as_ref().is_some_and(|result| {
            result.report_csv_url.is_some() && result.report_json_url.is_some()
        }));
    }

    #[test]
    fn file_export_handles_csv_escaping() {
        let mut demo = crate::demo::demo_playlists()[0].tracks[0].clone();
        demo.title = "A, \"quoted\" song".into();
        let item = crate::models::PlatformTrackMatch {
            source_track: demo,
            target_platform: "file".into(),
            target_track_id: Some("x".into()),
            target_url: None,
            version_type: crate::models::VersionType::Original,
            confidence: 1.0,
            match_reason: "export".into(),
            status: MatchStatus::Matched,
            candidates: vec![],
        };
        let artifact = build_file_artifact("test", "csv", &[item]).unwrap();
        let text = String::from_utf8_lossy(&artifact.bytes);
        assert!(text.contains("\"A, \"\"quoted\"\" song\""));
    }
    #[test]
    fn oauth_state_is_bound_to_browser_and_provider() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("melody_spotify_oauth_state=expected"),
        );
        assert!(oauth_browser_state_matches(&headers, "spotify", "expected"));
        assert!(!oauth_browser_state_matches(&headers, "spotify", "forged"));
        assert!(!oauth_browser_state_matches(
            &headers, "youtube", "expected"
        ));
        assert!(!oauth_browser_state_matches(
            &HeaderMap::new(),
            "spotify",
            "expected"
        ));
        let response = oauth_start_redirect(
            "https://accounts.spotify.com/authorize?state=synthetic",
            "spotify",
        );
        let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
        assert!(
            cookie.contains("HttpOnly")
                && cookie.contains("SameSite=Lax")
                && cookie.contains("Max-Age=600")
        );
    }

    #[tokio::test]
    async fn callback_errors_and_cross_browser_states_never_connect() {
        for provider in ["spotify", "youtube"] {
            for (query, reason) in [
                ("error=access_denied", "authorization_cancelled"),
                ("error=invalid_client", "provider_error"),
                ("code=synthetic", "missing_state"),
                ("code=synthetic&state=forged", "state_mismatch"),
            ] {
                let response = test_app()
                    .await
                    .oneshot(
                        axum::http::Request::builder()
                            .uri(format!("/api/{provider}/callback?{query}"))
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                let location = response.headers()[header::LOCATION].to_str().unwrap();
                assert!(location.contains(reason));
                assert!(!location.contains("synthetic") && !location.contains("connected"));
                assert!(
                    response.headers()[header::SET_COOKIE]
                        .to_str()
                        .unwrap()
                        .contains("Max-Age=0")
                );
            }
        }
    }
    #[tokio::test]
    async fn public_link_http_contract_does_not_return_fake_tracks() {
        let router = test_app().await;
        for (url, expected_status, expected_capability) in [
            ("not a URL", StatusCode::BAD_REQUEST, ""),
            (
                "https://music.apple.com/us/playlist/pl.synthetic",
                StatusCode::OK,
                "CONFIG_REQUIRED",
            ),
            (
                "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
                StatusCode::OK,
                "AUTH_REQUIRED",
            ),
            (
                "https://www.youtube.com/playlist?list=PLabcdefghijk",
                StatusCode::OK,
                "AUTH_REQUIRED",
            ),
            (
                "https://music.163.com/song?id=123456",
                StatusCode::OK,
                "URL_RECOGNITION_ONLY",
            ),
            (
                "https://example.org/playlist/123",
                StatusCode::OK,
                "UNSUPPORTED",
            ),
        ] {
            let response = router
                .clone()
                .oneshot(
                    axum::http::Request::builder()
                        .method("POST")
                        .uri("/api/playlists/inspect-link")
                        .header("content-type", "application/json")
                        .body(Body::from(serde_json::json!({"url":url}).to_string()))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected_status);
            if expected_status == StatusCode::OK {
                let body = to_bytes(response.into_body(), 16384).await.unwrap();
                let result: serde_json::Value = serde_json::from_slice(&body).unwrap();
                assert_eq!(result["capability"], expected_capability);
                assert_eq!(result["can_analyze"], false);
                assert_eq!(result["preview_tracks"].as_array().unwrap().len(), 0);
            }
        }
    }
}
