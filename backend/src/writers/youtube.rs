use super::{AddTracksOutcome, CreatedPlaylist, PlaylistWriter, status};
use crate::{
    models::{
        MatchCandidate, MatchStatus, PlatformTrackMatch, ProviderConfigurationStatus, Track,
        VersionType, WriterStatus, YoutubeConnectionStatus, YoutubeImportResult,
        YoutubeImportedPlaylist, YoutubePlaylistSummary,
    },
    normalize::{detect_version, normalize_text, token_similarity},
};
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use base64::Engine;
use rand::Rng;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, LazyLock},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

const YOUTUBE_SCOPE: &str = "https://www.googleapis.com/auth/youtube.force-ssl";
pub const DEFAULT_GOOGLE_REDIRECT_URI: &str = "http://127.0.0.1:3000/api/youtube/callback";
pub const YOUTUBE_POLICY_NOTICE: &str = "YouTube API 数据会保留来源标注，并只用于用户授权的播放列表读取、预览、传输和写回。YouTube 政策不允许本项目提供 API 未给出的独立派生指标，因此不会用这些数据生成 MelodyPath 画像或跨平台评分。";

#[derive(Clone)]
pub struct YoutubeConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub api_key: String,
    pub frontend_url: String,
    pub authorize_url: String,
    pub token_url: String,
    pub api_base_url: String,
}

impl std::fmt::Debug for YoutubeConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("YoutubeConfig")
            .field("client_id", &"[redacted]")
            .field("client_secret", &"[redacted]")
            .field("redirect_uri", &self.redirect_uri)
            .field("api_key", &"[redacted]")
            .field("frontend_url", &self.frontend_url)
            .field("authorize_url", &self.authorize_url)
            .field("token_url", &self.token_url)
            .field("api_base_url", &self.api_base_url)
            .finish()
    }
}

impl YoutubeConfig {
    fn from_env() -> Option<Self> {
        Some(Self {
            client_id: nonempty_env("GOOGLE_CLIENT_ID")?,
            client_secret: nonempty_env("GOOGLE_CLIENT_SECRET")?,
            redirect_uri: nonempty_env("GOOGLE_REDIRECT_URI")?,
            api_key: nonempty_env("YOUTUBE_API_KEY")?,
            frontend_url: std::env::var("FRONTEND_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:5173".into()),
            authorize_url: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            api_base_url: "https://www.googleapis.com/youtube/v3".into(),
        })
    }
}

#[derive(Clone)]
struct YoutubeTokenSet {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: u64,
    channel_id: String,
    display_name: String,
    avatar_url: Option<String>,
}

impl std::fmt::Debug for YoutubeTokenSet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("YoutubeTokenSet")
            .field("access_token", &"[redacted]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[redacted]"),
            )
            .field("expires_at", &self.expires_at)
            .field("channel_id", &self.channel_id)
            .field("display_name", &self.display_name)
            .field("avatar_url", &self.avatar_url)
            .finish()
    }
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
}

#[derive(Clone)]
pub struct YoutubePlaylistWriter {
    config: Option<YoutubeConfig>,
    client: Client,
    pending_states: Arc<RwLock<HashMap<String, u64>>>,
    sessions: Arc<RwLock<HashMap<String, YoutubeTokenSet>>>,
}

impl YoutubePlaylistWriter {
    pub fn new() -> Self {
        Self::from_config(YoutubeConfig::from_env(), Client::new())
    }

    fn from_config(config: Option<YoutubeConfig>, client: Client) -> Self {
        Self {
            config,
            client,
            pending_states: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_some()
    }

    pub fn configuration_status(&self) -> ProviderConfigurationStatus {
        let required = [
            "GOOGLE_CLIENT_ID",
            "GOOGLE_CLIENT_SECRET",
            "GOOGLE_REDIRECT_URI",
            "YOUTUBE_API_KEY",
        ];
        let (present, missing) = if self.config.is_some() {
            (required.iter().map(|item| (*item).into()).collect(), vec![])
        } else {
            let present: Vec<String> = required
                .iter()
                .filter(|name| nonempty_env(name).is_some())
                .map(|item| (*item).into())
                .collect();
            let missing = required
                .iter()
                .filter(|name| !present.iter().any(|item| item == **name))
                .map(|item| (*item).into())
                .collect();
            (present, missing)
        };
        let redirect_uri = self
            .config
            .as_ref()
            .map(|config| config.redirect_uri.clone())
            .or_else(|| nonempty_env("GOOGLE_REDIRECT_URI"))
            .unwrap_or_else(|| DEFAULT_GOOGLE_REDIRECT_URI.into());
        let redirect_valid = reqwest::Url::parse(&redirect_uri)
            .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some());
        let configured = missing.is_empty() && redirect_valid;
        ProviderConfigurationStatus {
            platform: "youtube".into(),
            display_name: "YouTube / YouTube Music".into(),
            configured,
            validation_status: if !missing.is_empty() {
                "missing_environment_variables"
            } else if !redirect_valid {
                "invalid_redirect_uri"
            } else {
                "ready_for_oauth"
            }
            .into(),
            required_environment_variables: required.iter().map(|item| (*item).into()).collect(),
            present_environment_variables: present,
            missing_environment_variables: missing,
            redirect_uri: Some(redirect_uri.clone()),
            dashboard_url: "https://console.cloud.google.com/apis/credentials".into(),
            setup_steps: vec![
                "在 Google Cloud 创建项目并启用 YouTube Data API v3。".into(),
                "配置 OAuth consent screen，并创建 Web application 类型的 OAuth Client。".into(),
                format!("在 Authorized redirect URIs 中精确添加 {redirect_uri}。"),
                "创建受限的 YouTube Data API Key；把四项配置只写入后端环境变量后重启。".into(),
            ],
            secrets_exposed_to_frontend: false,
            message: if configured {
                "必要环境变量已存在；可以开始 Google 官方 OAuth。".into()
            } else {
                "Google/YouTube 配置尚不完整；Client Secret 不会在网页显示或保存。".into()
            },
        }
    }

    pub async fn validate_configuration(&self) -> ProviderConfigurationStatus {
        let mut status = self.configuration_status();
        let Some(config) = self.config.as_ref() else {
            return status;
        };
        let response = self
            .client
            .get(format!(
                "{}/videos",
                config.api_base_url.trim_end_matches('/')
            ))
            .query(&[
                ("part", "id"),
                ("id", "dQw4w9WgXcQ"),
                ("key", config.api_key.as_str()),
            ])
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                status.validation_status = "api_key_valid_oauth_pending".into();
                status.message = "YouTube API Key 可用；OAuth Client 与 Redirect URI 仍需在首次授权时由 Google 校验。".into();
            }
            Ok(response) => {
                status.configured = false;
                status.validation_status = "api_key_rejected".into();
                status.message = format!(
                    "YouTube Data API 配置检查返回 HTTP {}；响应正文不会写入日志。",
                    response.status()
                );
            }
            Err(error) => {
                status.validation_status = "network_check_failed".into();
                status.message = format!("无法连接 YouTube Data API：{error}");
            }
        }
        status
    }

    pub async fn begin_authorization(&self) -> Result<String> {
        let config = self
            .config
            .as_ref()
            .context("YouTube OAuth 未配置；请先打开配置向导")?;
        let state = random_token();
        self.pending_states
            .write()
            .await
            .insert(state.clone(), now_secs());
        Ok(format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&access_type=offline&include_granted_scopes=true&prompt=consent&state={}",
            config.authorize_url,
            urlencoding::encode(&config.client_id),
            urlencoding::encode(&config.redirect_uri),
            urlencoding::encode(YOUTUBE_SCOPE),
            urlencoding::encode(&state)
        ))
    }

    pub async fn complete_authorization(
        &self,
        code: &str,
        state: &str,
    ) -> Result<(String, String)> {
        let config = self.config.as_ref().context("YouTube OAuth 未配置")?;
        let created = self
            .pending_states
            .write()
            .await
            .remove(state)
            .context("OAuth state 无效或已使用")?;
        if now_secs().saturating_sub(created) > 600 {
            bail!("OAuth state 已过期，请重新授权");
        }
        let token: GoogleTokenResponse = self
            .client
            .post(&config.token_url)
            .form(&[
                ("code", code),
                ("client_id", config.client_id.as_str()),
                ("client_secret", config.client_secret.as_str()),
                ("redirect_uri", config.redirect_uri.as_str()),
                ("grant_type", "authorization_code"),
            ])
            .send()
            .await
            .context("无法连接 Google token endpoint")?
            .error_for_status()
            .context("Google 拒绝了授权码")?
            .json()
            .await?;
        let (channel_id, display_name, avatar_url) =
            self.fetch_channel_profile(&token.access_token).await?;
        let session = random_token();
        self.sessions.write().await.insert(
            session.clone(),
            YoutubeTokenSet {
                access_token: token.access_token,
                refresh_token: token.refresh_token,
                expires_at: now_secs() + token.expires_in.saturating_sub(30),
                channel_id,
                display_name,
                avatar_url,
            },
        );
        Ok((session, config.frontend_url.clone()))
    }

    pub async fn connection_status(&self, auth_session: Option<&str>) -> YoutubeConnectionStatus {
        if !self.is_configured() {
            return YoutubeConnectionStatus {
                configured: false,
                connected: false,
                channel_id: None,
                display_name: None,
                avatar_url: None,
                message: "尚未配置 Google OAuth 与 YouTube Data API。".into(),
                policy_notice: YOUTUBE_POLICY_NOTICE.into(),
            };
        }
        match self.token(auth_session).await {
            Ok(token) => YoutubeConnectionStatus {
                configured: true,
                connected: true,
                channel_id: Some(token.channel_id),
                display_name: Some(token.display_name),
                avatar_url: token.avatar_url,
                message: "已通过 Google 官方 OAuth 连接 YouTube 频道。".into(),
                policy_notice: YOUTUBE_POLICY_NOTICE.into(),
            },
            Err(_) => YoutubeConnectionStatus {
                configured: true,
                connected: false,
                channel_id: None,
                display_name: None,
                avatar_url: None,
                message: "OAuth 已配置，请在 Google 官方页面授权。".into(),
                policy_notice: YOUTUBE_POLICY_NOTICE.into(),
            },
        }
    }

    pub async fn disconnect(&self, auth_session: Option<&str>) -> bool {
        let Some(session) = auth_session else {
            return false;
        };
        self.sessions.write().await.remove(session).is_some()
    }

    async fn fetch_channel_profile(
        &self,
        access_token: &str,
    ) -> Result<(String, String, Option<String>)> {
        let config = self.config.as_ref().context("YouTube OAuth 未配置")?;
        let payload: Value = self
            .client
            .get(format!(
                "{}/channels",
                config.api_base_url.trim_end_matches('/')
            ))
            .bearer_auth(access_token)
            .query(&[("part", "snippet"), ("mine", "true"), ("maxResults", "1")])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let channel = payload
            .get("items")
            .and_then(Value::as_array)
            .and_then(|items| items.first())
            .context("当前 Google 账号没有可访问的 YouTube 频道")?;
        let id = channel
            .get("id")
            .and_then(Value::as_str)
            .context("YouTube 频道缺少 id")?
            .to_string();
        let title = channel
            .pointer("/snippet/title")
            .and_then(Value::as_str)
            .unwrap_or("YouTube 用户")
            .to_string();
        let avatar = channel
            .pointer("/snippet/thumbnails/default/url")
            .and_then(Value::as_str)
            .map(str::to_string);
        Ok((id, title, avatar))
    }

    async fn token(&self, auth_session: Option<&str>) -> Result<YoutubeTokenSet> {
        let session = auth_session.context("尚未授权 YouTube")?;
        let existing = self
            .sessions
            .read()
            .await
            .get(session)
            .cloned()
            .context("YouTube 会话无效或服务已重启，请重新授权")?;
        if existing.expires_at > now_secs() {
            return Ok(existing);
        }
        let refresh = existing
            .refresh_token
            .clone()
            .context("Google token 已过期且没有 refresh token，请重新授权")?;
        let config = self.config.as_ref().context("YouTube OAuth 未配置")?;
        let response: GoogleTokenResponse = self
            .client
            .post(&config.token_url)
            .form(&[
                ("client_id", config.client_id.as_str()),
                ("client_secret", config.client_secret.as_str()),
                ("refresh_token", refresh.as_str()),
                ("grant_type", "refresh_token"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let refreshed = YoutubeTokenSet {
            access_token: response.access_token,
            refresh_token: response.refresh_token.or(existing.refresh_token),
            expires_at: now_secs() + response.expires_in.saturating_sub(30),
            channel_id: existing.channel_id,
            display_name: existing.display_name,
            avatar_url: existing.avatar_url,
        };
        self.sessions
            .write()
            .await
            .insert(session.into(), refreshed.clone());
        Ok(refreshed)
    }

    pub async fn list_playlists(
        &self,
        auth_session: Option<&str>,
    ) -> Result<Vec<YoutubePlaylistSummary>> {
        let token = self.token(auth_session).await?;
        self.list_playlists_with_token(&token).await
    }

    async fn list_playlists_with_token(
        &self,
        token: &YoutubeTokenSet,
    ) -> Result<Vec<YoutubePlaylistSummary>> {
        let config = self.config.as_ref().context("YouTube OAuth 未配置")?;
        let mut playlists = Vec::new();
        let mut page_token: Option<String> = None;
        let mut seen_pages = HashSet::new();
        loop {
            let mut request = self
                .client
                .get(format!(
                    "{}/playlists",
                    config.api_base_url.trim_end_matches('/')
                ))
                .bearer_auth(&token.access_token)
                .query(&[
                    ("part", "snippet,contentDetails,status"),
                    ("mine", "true"),
                    ("maxResults", "50"),
                ]);
            if let Some(value) = page_token.as_deref() {
                request = request.query(&[("pageToken", value)]);
            }
            let payload: Value = request.send().await?.error_for_status()?.json().await?;
            for item in payload
                .get("items")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let Some(id) = item.get("id").and_then(Value::as_str) else {
                    continue;
                };
                playlists.push(YoutubePlaylistSummary {
                    id: id.into(),
                    name: item
                        .pointer("/snippet/title")
                        .and_then(Value::as_str)
                        .unwrap_or("未命名播放列表")
                        .into(),
                    owner_name: item
                        .pointer("/snippet/channelTitle")
                        .and_then(Value::as_str)
                        .unwrap_or(&token.display_name)
                        .into(),
                    item_count: item
                        .pointer("/contentDetails/itemCount")
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as usize,
                    privacy_status: item
                        .pointer("/status/privacyStatus")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    youtube_url: format!("https://www.youtube.com/playlist?list={id}"),
                    image_url: item
                        .pointer("/snippet/thumbnails/medium/url")
                        .or_else(|| item.pointer("/snippet/thumbnails/default/url"))
                        .and_then(Value::as_str)
                        .map(str::to_string),
                });
            }
            let next = payload
                .get("nextPageToken")
                .and_then(Value::as_str)
                .map(str::to_string);
            let Some(next) = next else { break };
            if !seen_pages.insert(next.clone()) || seen_pages.len() > 200 {
                bail!("YouTube 歌单分页响应重复，已安全停止")
            }
            page_token = Some(next);
        }
        Ok(playlists)
    }

    pub async fn import_playlists(
        &self,
        playlist_ids: &[String],
        auth_session: Option<&str>,
    ) -> Result<YoutubeImportResult> {
        if playlist_ids.is_empty() {
            bail!("请至少选择一个 YouTube 播放列表");
        }
        if playlist_ids.len() > 20 {
            bail!("一次最多选择 20 个 YouTube 播放列表");
        }
        let token = self.token(auth_session).await?;
        let accessible: HashMap<_, _> = self
            .list_playlists_with_token(&token)
            .await?
            .into_iter()
            .map(|playlist| (playlist.id.clone(), playlist))
            .collect();
        let config = self.config.as_ref().context("YouTube OAuth 未配置")?;
        let mut tracks = Vec::new();
        let mut seen_tracks = HashSet::new();
        let mut imported_playlists = Vec::new();
        for playlist_id in playlist_ids {
            let summary = accessible
                .get(playlist_id)
                .with_context(|| format!("播放列表 {playlist_id} 不在当前账号拥有的列表中"))?;
            let before = tracks.len();
            let mut page_token: Option<String> = None;
            let mut seen_pages = HashSet::new();
            loop {
                let mut request = self
                    .client
                    .get(format!(
                        "{}/playlistItems",
                        config.api_base_url.trim_end_matches('/')
                    ))
                    .bearer_auth(&token.access_token)
                    .query(&[
                        ("part", "snippet,contentDetails"),
                        ("playlistId", playlist_id.as_str()),
                        ("maxResults", "50"),
                    ]);
                if let Some(value) = page_token.as_deref() {
                    request = request.query(&[("pageToken", value)]);
                }
                let payload: Value = request.send().await?.error_for_status()?.json().await?;
                for item in payload
                    .get("items")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let Some(track) = youtube_track(item) else {
                        continue;
                    };
                    if seen_tracks.insert(track.id.clone()) {
                        tracks.push(track);
                    }
                }
                let next = payload
                    .get("nextPageToken")
                    .and_then(Value::as_str)
                    .map(str::to_string);
                let Some(next) = next else { break };
                if !seen_pages.insert(next.clone()) || seen_pages.len() > 500 {
                    bail!("YouTube 曲目分页响应重复，已安全停止")
                }
                page_token = Some(next);
            }
            imported_playlists.push(YoutubeImportedPlaylist {
                id: summary.id.clone(),
                name: summary.name.clone(),
                youtube_url: summary.youtube_url.clone(),
                imported_count: tracks.len() - before,
            });
        }
        let track_count = tracks.len();
        Ok(YoutubeImportResult {
            playlists: imported_playlists,
            tracks,
            track_count,
            data_use: crate::platforms::youtube_data_use(),
            policy_notice: YOUTUBE_POLICY_NOTICE.into(),
            attribution:
                "视频与播放列表元数据来自 YouTube Data API；播放和原始页面由 YouTube 提供。".into(),
        })
    }

    async fn search_items(&self, access_token: &str, query: &str) -> Result<Vec<Value>> {
        let config = self.config.as_ref().context("YouTube OAuth 未配置")?;
        let payload: Value = self
            .client
            .get(format!(
                "{}/search",
                config.api_base_url.trim_end_matches('/')
            ))
            .bearer_auth(access_token)
            .query(&[
                ("part", "snippet"),
                ("type", "video"),
                ("maxResults", "5"),
                ("q", query),
                ("key", config.api_key.as_str()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(payload
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }
}

#[async_trait]
impl PlaylistWriter for YoutubePlaylistWriter {
    fn platform(&self) -> &'static str {
        "youtube"
    }

    async fn authorize(&self, auth_session: Option<&str>) -> Result<WriterStatus> {
        if !self.is_configured() {
            return Ok(status(
                "youtube",
                "YouTube",
                "needs_configuration",
                false,
                false,
                "需要配置 Google OAuth 与 YouTube Data API；Demo 和文件导出不受影响。",
            ));
        }
        let authorized = match auth_session {
            Some(session) => self.sessions.read().await.contains_key(session),
            None => false,
        };
        Ok(status(
            "youtube",
            "YouTube",
            "available",
            authorized,
            false,
            if authorized {
                "已通过 Google OAuth 授权；创建前仍需预览并确认。"
            } else {
                "配置已就绪，请先通过 Google 官方页面授权。"
            },
        ))
    }

    async fn search_track(
        &self,
        track: &Track,
        auth_session: Option<&str>,
    ) -> Result<PlatformTrackMatch> {
        if track.platform == "youtube"
            && let Some(video_id) = track.external_ids.get("youtube")
        {
            return Ok(PlatformTrackMatch {
                source_track: track.clone(),
                target_platform: "youtube".into(),
                target_track_id: Some(video_id.clone()),
                target_url: track.platform_url.clone(),
                version_type: track.version_type.clone(),
                confidence: 1.0,
                match_reason: "同一 YouTube Video ID，直接用于用户发起的播放列表传输".into(),
                status: MatchStatus::Matched,
                candidates: vec![],
            });
        }
        let token = self.token(auth_session).await?;
        let query = format!("{} {}", track.artists.join(" "), track.title);
        let mut candidates: Vec<MatchCandidate> = self
            .search_items(&token.access_token, &query)
            .await?
            .iter()
            .filter_map(|item| youtube_candidate(track, item))
            .collect();
        candidates.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
        let best = candidates.first().cloned();
        let (match_status, id, url, version, confidence, reason) = match best {
            Some(candidate) if candidate.confidence >= 0.88 => (
                MatchStatus::Matched,
                Some(candidate.target_track_id),
                candidate.target_url,
                candidate.version_type,
                candidate.confidence,
                candidate.match_reason,
            ),
            Some(candidate) if candidate.confidence >= 0.65 => (
                MatchStatus::NeedsConfirmation,
                None,
                None,
                candidate.version_type,
                candidate.confidence,
                format!("{}；请确认视频版本", candidate.match_reason),
            ),
            Some(candidate) => (
                MatchStatus::Unmatched,
                None,
                None,
                candidate.version_type,
                candidate.confidence,
                "候选置信度过低，未自动选择".into(),
            ),
            None => (
                MatchStatus::Unmatched,
                None,
                None,
                VersionType::Unknown,
                0.0,
                "YouTube 未找到候选视频".into(),
            ),
        };
        Ok(PlatformTrackMatch {
            source_track: track.clone(),
            target_platform: "youtube".into(),
            target_track_id: id,
            target_url: url,
            version_type: version,
            confidence,
            match_reason: reason,
            status: match_status,
            candidates,
        })
    }

    async fn create_playlist(
        &self,
        name: &str,
        auth_session: Option<&str>,
    ) -> Result<CreatedPlaylist> {
        let token = self.token(auth_session).await?;
        let config = self.config.as_ref().context("YouTube OAuth 未配置")?;
        let payload: Value = self
            .client
            .post(format!(
                "{}/playlists",
                config.api_base_url.trim_end_matches('/')
            ))
            .bearer_auth(&token.access_token)
            .query(&[("part", "snippet,status")])
            .json(&json!({
                "snippet": { "title": name, "description": "由 MelodyPath 在用户确认后创建的新播放列表" },
                "status": { "privacyStatus": "private" }
            }))
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let id = payload
            .get("id")
            .and_then(Value::as_str)
            .context("YouTube 创建播放列表响应缺少 id")?
            .to_string();
        Ok(CreatedPlaylist {
            url: Some(format!("https://www.youtube.com/playlist?list={id}")),
            id,
        })
    }

    async fn add_tracks(
        &self,
        playlist_id: &str,
        track_ids: &[String],
        auth_session: Option<&str>,
    ) -> Result<AddTracksOutcome> {
        let token = self.token(auth_session).await?;
        let config = self.config.as_ref().context("YouTube OAuth 未配置")?;
        let mut added_ids = Vec::new();
        let mut failures = HashMap::new();
        for video_id in track_ids {
            let response = self
                .client
                .post(format!(
                    "{}/playlistItems",
                    config.api_base_url.trim_end_matches('/')
                ))
                .bearer_auth(&token.access_token)
                .query(&[("part", "snippet")])
                .json(&json!({
                    "snippet": {
                        "playlistId": playlist_id,
                        "resourceId": { "kind": "youtube#video", "videoId": video_id }
                    }
                }))
                .send()
                .await?;
            if response.status().is_success() {
                added_ids.push(video_id.clone());
            } else {
                failures.insert(
                    video_id.clone(),
                    format!(
                        "YouTube HTTP {}；未记录可能含敏感信息的响应正文",
                        response.status()
                    ),
                );
            }
        }
        Ok(AddTracksOutcome {
            added_ids,
            failures,
        })
    }
}

static NOISE_GROUP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\s*[\[(][^\])]*(official\s*(music\s*)?video|official\s*audio|lyrics?|audio|mv)[^\])]*[\])]\s*")
        .expect("YouTube title noise regex is valid")
});
static NOISE_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\s*[-|:]\s*(official\s*(music\s*)?video|official\s*audio|lyrics?|audio|mv)\s*$",
    )
    .expect("YouTube title suffix regex is valid")
});

pub(crate) fn clean_youtube_title(raw: &str) -> String {
    let without_group = NOISE_GROUP.replace_all(raw, " ");
    let without_suffix = NOISE_SUFFIX.replace_all(&without_group, " ");
    without_suffix
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(['-', '|', ':', ' '])
        .trim()
        .to_string()
}

fn clean_channel_artist(raw: &str) -> String {
    let mut value = raw.trim().to_string();
    for suffix in [" - Topic", " – Topic", "VEVO", " Vevo", " Official"] {
        if value
            .to_ascii_lowercase()
            .ends_with(&suffix.to_ascii_lowercase())
        {
            value.truncate(value.len().saturating_sub(suffix.len()));
            value = value.trim().to_string();
        }
    }
    if value.is_empty() {
        "YouTube 频道".into()
    } else {
        value
    }
}

pub(crate) fn normalize_youtube_metadata(raw_title: &str, channel: &str) -> (String, Vec<String>) {
    let cleaned = clean_youtube_title(raw_title);
    if let Some((artist, title)) = cleaned.split_once(" - ")
        && !artist.trim().is_empty()
        && !title.trim().is_empty()
    {
        return (title.trim().to_string(), vec![artist.trim().to_string()]);
    }
    (cleaned, vec![clean_channel_artist(channel)])
}

pub(crate) fn youtube_track(item: &Value) -> Option<Track> {
    let video_id = item
        .pointer("/contentDetails/videoId")
        .or_else(|| item.pointer("/snippet/resourceId/videoId"))
        .and_then(Value::as_str)?;
    let raw_title = item.pointer("/snippet/title").and_then(Value::as_str)?;
    if matches!(raw_title, "Deleted video" | "Private video") {
        return None;
    }
    let channel = item
        .pointer("/snippet/videoOwnerChannelTitle")
        .or_else(|| item.pointer("/snippet/channelTitle"))
        .and_then(Value::as_str)
        .unwrap_or("YouTube 频道");
    let (title, artists) = normalize_youtube_metadata(raw_title, channel);
    if title.is_empty() {
        return None;
    }
    Some(Track {
        id: format!("youtube:{video_id}"),
        normalized_title: normalize_text(&title),
        version_type: detect_version(&title),
        title,
        artists,
        album: None,
        genres: vec![],
        release_year: None,
        language: None,
        duration_ms: None,
        platform: "youtube".into(),
        platform_url: Some(format!("https://www.youtube.com/watch?v={video_id}")),
        external_ids: HashMap::from([("youtube".into(), video_id.into())]),
        mood_tags: vec![],
        energy_score: None,
        popularity: None,
        metadata_confidence: 0.72,
    })
}

fn youtube_candidate(source: &Track, item: &Value) -> Option<MatchCandidate> {
    let video_id = item.pointer("/id/videoId").and_then(Value::as_str)?;
    let raw_title = item.pointer("/snippet/title").and_then(Value::as_str)?;
    let channel = item
        .pointer("/snippet/channelTitle")
        .and_then(Value::as_str)
        .unwrap_or("YouTube 频道");
    let (title, artists) = normalize_youtube_metadata(raw_title, channel);
    let title_score = token_similarity(&source.title, &title);
    let artist_score = token_similarity(&source.artists.join(" "), &artists.join(" "));
    let target_version = detect_version(&title);
    let version_score = if target_version == source.version_type {
        1.0
    } else {
        0.35
    };
    let confidence =
        (title_score * 0.62 + artist_score * 0.30 + version_score * 0.08).clamp(0.0, 1.0);
    Some(MatchCandidate {
        target_track_id: video_id.into(),
        title,
        artists,
        album: None,
        target_url: Some(format!("https://www.youtube.com/watch?v={video_id}")),
        version_type: target_version,
        confidence,
        match_reason: format!(
            "清理 Official Video/Lyrics 等标题噪声后：歌名 {:.0}% · 歌手 {:.0}%",
            title_score * 100.0,
            artist_score * 100.0
        ),
        available_in_market: true,
    })
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::ZERO)
        .as_secs()
}

fn random_token() -> String {
    let mut bytes = [0_u8; 32];
    rand::rng().fill(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

fn nonempty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_common_youtube_music_title_noise() {
        let (title, artists) =
            normalize_youtube_metadata("DEAN - instagram (Official Music Video)", "DEAN Official");
        assert_eq!(title, "instagram");
        assert_eq!(artists, vec!["DEAN"]);
        assert_eq!(clean_youtube_title("Song | Lyrics"), "Song");
    }

    #[test]
    fn converts_youtube_playlist_item_to_unified_track() {
        let item = json!({
            "snippet": { "title": "BIBI - Kazino [Official Video]", "videoOwnerChannelTitle": "BIBI - Topic" },
            "contentDetails": { "videoId": "video123" }
        });
        let track = youtube_track(&item).unwrap();
        assert_eq!(track.platform, "youtube");
        assert_eq!(track.title, "Kazino");
        assert_eq!(track.artists, vec!["BIBI"]);
        assert_eq!(
            track.external_ids.get("youtube").map(String::as_str),
            Some("video123")
        );
    }

    #[tokio::test]
    async fn rejects_unknown_oauth_state_before_network_request() {
        let writer = YoutubePlaylistWriter::from_config(
            Some(YoutubeConfig {
                client_id: "client".into(),
                client_secret: "super-secret".into(),
                redirect_uri: DEFAULT_GOOGLE_REDIRECT_URI.into(),
                api_key: "api-key".into(),
                frontend_url: "http://127.0.0.1:5173".into(),
                authorize_url: "http://127.0.0.1:9/auth".into(),
                token_url: "http://127.0.0.1:9/token".into(),
                api_base_url: "http://127.0.0.1:9".into(),
            }),
            Client::new(),
        );
        let error = writer
            .complete_authorization("code", "forged-state")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("state"));
    }

    #[test]
    fn debug_output_redacts_google_secrets() {
        let config = YoutubeConfig {
            client_id: "client-id-sensitive".into(),
            client_secret: "client-secret-sensitive".into(),
            redirect_uri: DEFAULT_GOOGLE_REDIRECT_URI.into(),
            api_key: "api-key-sensitive".into(),
            frontend_url: "http://127.0.0.1:5173".into(),
            authorize_url: "https://accounts.google.com".into(),
            token_url: "https://oauth2.googleapis.com/token".into(),
            api_base_url: "https://www.googleapis.com/youtube/v3".into(),
        };
        let output = format!("{config:?}");
        assert!(!output.contains("client-secret-sensitive"));
        assert!(!output.contains("api-key-sensitive"));
    }
}
