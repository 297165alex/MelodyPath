mod agent;
mod demo;
mod engine;
mod genre;
mod import;
mod metadata;
mod models;
mod normalize;
mod platforms;
mod recommendation;
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
use std::{collections::HashMap, convert::Infallible, path::PathBuf, sync::Arc, time::Duration};
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
    imports: Arc<RwLock<HashMap<String, import::StoredImport>>>,
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
        .layer(TraceLayer::new_for_http())
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
    let state = AppState {
        previews: Arc::new(RwLock::new(HashMap::new())),
        downloads: Arc::new(RwLock::new(HashMap::new())),
        spotify: SpotifyPlaylistWriter::new(),
        youtube: writers::youtube::YoutubePlaylistWriter::new(),
        apple: AppleMusicConnector::new(),
        platforms: platforms::PlatformService::new(),
        agent: agent::AgentService::new(database_path).await?,
        metadata: metadata::MetadataService::new(),
        imports: Arc::new(RwLock::new(HashMap::new())),
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
    Ok(Json(state.metadata.analyze(playlist).await))
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
    Ok(Json(
        state
            .metadata
            .analyze_imported(
                imported.name,
                imported.source_label,
                imported.data_state,
                imported.total_rows,
                imported.tracks,
            )
            .await,
    ))
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
                        let terminal = matches!(task.status.as_str(), "completed" | "failed" | "cancelled");
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
    Json(request): Json<PlaylistLinkRequest>,
) -> Result<Json<models::PlaylistLinkInspection>, ApiError> {
    if request.url.trim().len() > 2_048 {
        return Err(ApiError::bad_request("链接过长"));
    }
    Ok(Json(
        state
            .platforms
            .inspect_link(&request.url)
            .await
            .map_err(|error| ApiError::bad_request(error.to_string()))?,
    ))
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
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static(
            "melody_spotify_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0",
        ),
    );
    Ok(response)
}

async fn youtube_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Json<models::YoutubeConnectionStatus> {
    Json(
        state
            .youtube
            .connection_status(youtube_auth_session(&headers).as_deref())
            .await,
    )
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
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_static(
            "melody_youtube_session=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0",
        ),
    );
    Ok(response)
}

async fn youtube_authorize(State(state): State<AppState>) -> Result<Redirect, ApiError> {
    let url = state
        .youtube
        .begin_authorization()
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Redirect::temporary(&url))
}

async fn youtube_callback(
    State(app): State<AppState>,
    Query(query): Query<SpotifyCallback>,
) -> Result<Response, ApiError> {
    if let Some(error) = query.error {
        return Err(ApiError::bad_request(format!(
            "Google/YouTube 授权未完成：{error}"
        )));
    }
    let (session, frontend) = app
        .youtube
        .complete_authorization(
            query
                .code
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("回调缺少 code"))?,
            query
                .state
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("回调缺少 state"))?,
        )
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    let cookie =
        format!("melody_youtube_session={session}; HttpOnly; SameSite=Lax; Path=/; Max-Age=28800");
    let mut response =
        Redirect::temporary(&format!("{frontend}/?youtube=connected")).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(ApiError::internal)?,
    );
    Ok(response)
}

async fn spotify_authorize(State(state): State<AppState>) -> Result<Redirect, ApiError> {
    let url = state
        .spotify
        .begin_authorization()
        .await
        .map_err(|error| ApiError::bad_request(error.to_string()))?;
    Ok(Redirect::temporary(&url))
}

#[derive(Debug, Deserialize)]
struct SpotifyCallback {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

async fn spotify_callback(
    State(app): State<AppState>,
    Query(query): Query<SpotifyCallback>,
) -> Result<Response, ApiError> {
    if let Some(error) = query.error {
        return Err(ApiError::bad_request(format!(
            "Spotify 授权未完成：{error}"
        )));
    }
    let (session, frontend) = app
        .spotify
        .complete_authorization(
            query
                .code
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("回调缺少 code"))?,
            query
                .state
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("回调缺少 state"))?,
        )
        .await
        .map_err(ApiError::internal)?;
    let cookie =
        format!("melody_spotify_session={session}; HttpOnly; SameSite=Lax; Path=/; Max-Age=28800");
    let mut response =
        Redirect::temporary(&format!("{frontend}/?spotify=connected")).into_response();
    response.headers_mut().insert(
        header::SET_COOKIE,
        HeaderValue::from_str(&cookie).map_err(ApiError::internal)?,
    );
    Ok(response)
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

    let mut matches = Vec::with_capacity(request.tracks.len());
    for track in &request.tracks {
        matches.push(
            writer
                .search_track(track, session.as_deref())
                .await
                .map_err(ApiError::internal)?,
        );
    }
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
            apple: AppleMusicConnector::new(),
            platforms: platforms::PlatformService::new(),
            agent: agent::AgentService::in_memory().await.unwrap(),
            metadata: metadata::MetadataService::new(),
            imports: Arc::new(RwLock::new(HashMap::new())),
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
}
