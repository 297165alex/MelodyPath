use super::{AddTracksOutcome, CreatedPlaylist, PlaylistWriter, status};
use crate::{
    models::{
        MatchCandidate, MatchStatus, PlatformTrackMatch, ProviderConfigurationStatus,
        SpotifyConnectionStatus, SpotifyImportResult, SpotifyImportedPlaylist,
        SpotifyPlaylistSummary, Track, VersionType, WriterStatus,
    },
    normalize::{detect_version, normalize_text, token_similarity},
    secure_store::SecureJsonStore,
};
use anyhow::{Context, Result, bail};
use async_trait::async_trait;
use base64::{Engine, engine::general_purpose::STANDARD};
use rand::Rng;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::sync::RwLock;

const SCOPES: &str = "playlist-read-private playlist-read-collaborative";
const WRITE_SCOPE: &str = "playlist-modify-private";
pub const SPOTIFY_POLICY_NOTICE: &str = "依据 Spotify Developer Policy，Spotify 内容不会发送给 LLM、用于训练、建立用户画像或计算衍生听歌指标；这里只支持用户主动发起的歌单选择、合规传输与写回。";
pub const DEFAULT_SPOTIFY_REDIRECT_URI: &str = "http://127.0.0.1:3000/api/spotify/callback";

#[derive(Clone)]
pub struct SpotifyConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub frontend_url: String,
    pub market: String,
    pub accounts_authorize_url: String,
    pub accounts_token_url: String,
    pub api_base_url: String,
}

impl std::fmt::Debug for SpotifyConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SpotifyConfig")
            .field("client_id", &"[redacted]")
            .field("client_secret", &"[redacted]")
            .field("redirect_uri", &self.redirect_uri)
            .field("frontend_url", &self.frontend_url)
            .field("market", &self.market)
            .field("accounts_authorize_url", &self.accounts_authorize_url)
            .field("accounts_token_url", &self.accounts_token_url)
            .field("api_base_url", &self.api_base_url)
            .finish()
    }
}

impl SpotifyConfig {
    pub fn from_env() -> Option<Self> {
        Some(Self {
            client_id: nonempty_env("SPOTIFY_CLIENT_ID")?,
            client_secret: nonempty_env("SPOTIFY_CLIENT_SECRET")?,
            redirect_uri: nonempty_env("SPOTIFY_REDIRECT_URI")
                .or_else(|| public_endpoint("/api/spotify/callback"))?,
            frontend_url: nonempty_env("FRONTEND_URL")
                .or_else(|| nonempty_env("PUBLIC_BASE_URL"))
                .unwrap_or_else(|| "http://127.0.0.1:5173".into()),
            market: std::env::var("SPOTIFY_MARKET").unwrap_or_else(|_| "US".into()),
            accounts_authorize_url: "https://accounts.spotify.com/authorize".into(),
            accounts_token_url: "https://accounts.spotify.com/api/token".into(),
            api_base_url: "https://api.spotify.com/v1".into(),
        })
    }
}

fn public_endpoint(path: &str) -> Option<String> {
    let base = nonempty_env("PUBLIC_BASE_URL")?;
    let parsed = reqwest::Url::parse(&base).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return None;
    }
    Some(format!("{}{}", base.trim_end_matches('/'), path))
}

#[derive(Clone, Serialize, Deserialize)]
struct TokenSet {
    access_token: String,
    refresh_token: Option<String>,
    expires_at: u64,
    spotify_user_id: String,
    display_name: String,
    avatar_url: Option<String>,
    #[serde(default)]
    granted_scopes: String,
}

impl std::fmt::Debug for TokenSet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TokenSet")
            .field("access_token", &"[redacted]")
            .field(
                "refresh_token",
                &self.refresh_token.as_ref().map(|_| "[redacted]"),
            )
            .field("expires_at", &self.expires_at)
            .field("spotify_user_id", &self.spotify_user_id)
            .field("display_name", &self.display_name)
            .field("avatar_url", &self.avatar_url)
            .finish()
    }
}

#[derive(Clone)]
pub struct SpotifyPlaylistWriter {
    config: Option<SpotifyConfig>,
    client: Client,
    pending_states: Arc<RwLock<HashMap<String, u64>>>,
    sessions: Arc<RwLock<HashMap<String, TokenSet>>>,
    session_store: Option<SecureJsonStore>,
}

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: u64,
    #[serde(default)]
    scope: String,
}

impl SpotifyPlaylistWriter {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(12))
            .build()
            .unwrap_or_default();
        let session_store = SecureJsonStore::for_oauth_provider("spotify");
        let sessions = session_store
            .as_ref()
            .and_then(|store| store.load().ok())
            .unwrap_or_default();
        Self {
            config: SpotifyConfig::from_env(),
            client,
            pending_states: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(sessions)),
            session_store,
        }
    }

    fn from_config(config: Option<SpotifyConfig>, client: Client) -> Self {
        Self {
            config,
            client,
            pending_states: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_store: None,
        }
    }

    pub fn frontend_url(&self) -> String {
        self.config
            .as_ref()
            .map(|config| config.frontend_url.clone())
            .unwrap_or_else(|| crate::deployment::frontend_url())
    }

    async fn persist_sessions(&self) -> Result<()> {
        let Some(store) = &self.session_store else {
            return Ok(());
        };
        let sessions = self.sessions.read().await;
        if sessions.is_empty() {
            store.remove()
        } else {
            store.save(&*sessions)
        }
    }

    pub fn is_configured(&self) -> bool {
        self.config.as_ref().is_some_and(|config| {
            reqwest::Url::parse(&config.redirect_uri).is_ok_and(|url| {
                matches!(url.scheme(), "http" | "https")
                    && url.host_str().is_some()
                    && url.username().is_empty()
                    && url.password().is_none()
                    && url.fragment().is_none()
            })
        })
    }

    pub fn configuration_status(&self) -> ProviderConfigurationStatus {
        let required = [
            "SPOTIFY_CLIENT_ID",
            "SPOTIFY_CLIENT_SECRET",
            "SPOTIFY_REDIRECT_URI",
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
            .or_else(|| nonempty_env("SPOTIFY_REDIRECT_URI"))
            .or_else(|| public_endpoint("/api/spotify/callback"))
            .unwrap_or_else(|| DEFAULT_SPOTIFY_REDIRECT_URI.into());
        let redirect_valid = reqwest::Url::parse(&redirect_uri)
            .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some());
        let configured = missing.is_empty() && redirect_valid && self.is_configured();
        ProviderConfigurationStatus {
            platform: "spotify".into(),
            display_name: "Spotify".into(),
            configured,
            validation_status: if !missing.is_empty() {
                "missing_environment_variables"
            } else if !redirect_valid || !self.is_configured() {
                "invalid_redirect_uri"
            } else {
                "ready_for_oauth"
            }
            .into(),
            required_environment_variables: required.iter().map(|item| (*item).into()).collect(),
            present_environment_variables: present,
            missing_environment_variables: missing,
            redirect_uri: Some(redirect_uri.clone()),
            dashboard_url: "https://developer.spotify.com/dashboard".into(),
            setup_steps: vec![
                "在 Spotify Developer Dashboard 创建应用。".into(),
                format!("在 Redirect URIs 中精确添加 {redirect_uri}。"),
                "在后端运行环境设置 Client ID、Client Secret 与 Redirect URI，然后重启后端。"
                    .into(),
                "开发模式应用的拥有者需满足 Spotify 当前 Premium/用户白名单要求。".into(),
            ],
            secrets_exposed_to_frontend: false,
            message: if configured {
                "必要环境变量已存在；可以开始官方 OAuth。Redirect URI 是否已登记将在授权时由 Spotify 校验。".into()
            } else {
                "配置尚不完整；密钥只能写入后端环境变量，不能粘贴到网页。".into()
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
            .post(&config.accounts_token_url)
            .header(
                "Authorization",
                format!(
                    "Basic {}",
                    STANDARD.encode(format!("{}:{}", config.client_id, config.client_secret))
                ),
            )
            .form(&[("grant_type", "client_credentials")])
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                status.validation_status = "credentials_valid".into();
                status.message = "Spotify 已接受 Client ID/Secret；仍需在 Dashboard 精确登记回调地址并由用户完成 OAuth。".into();
            }
            Ok(response) => {
                status.configured = false;
                status.validation_status = "credentials_rejected".into();
                status.message = format!(
                    "Spotify 配置检查返回 HTTP {}。请核对 Client ID/Secret；响应正文不会写入日志。",
                    response.status()
                );
            }
            Err(error) => {
                status.validation_status = "network_check_failed".into();
                status.message = format!("无法连接 Spotify 配置检查端点：{error}");
            }
        }
        status
    }

    pub async fn connection_status(&self, auth_session: Option<&str>) -> SpotifyConnectionStatus {
        if !self.is_configured() {
            return SpotifyConnectionStatus {
                write_authorized: false,
                configured: false,
                connected: false,
                user_id: None,
                display_name: None,
                avatar_url: None,
                message:
                    "CONFIG_REQUIRED：尚未配置有效的 Spotify Developer 应用。文件导入仍可运行。"
                        .into(),
                policy_notice: SPOTIFY_POLICY_NOTICE.into(),
            };
        }
        match self.token(auth_session).await {
            Ok(token) => SpotifyConnectionStatus {
                write_authorized: token
                    .granted_scopes
                    .split_whitespace()
                    .any(|scope| scope == WRITE_SCOPE),
                configured: true,
                connected: true,
                user_id: Some(token.spotify_user_id),
                display_name: Some(token.display_name),
                avatar_url: token.avatar_url,
                message: "已通过 Spotify 官方 OAuth 连接；本地不保存平台密码。".into(),
                policy_notice: SPOTIFY_POLICY_NOTICE.into(),
            },
            Err(_) => SpotifyConnectionStatus {
                write_authorized: false,
                configured: true,
                connected: false,
                user_id: None,
                display_name: None,
                avatar_url: None,
                message: if auth_session.is_some() {
                    "Spotify 连接已失效，请重新连接。"
                } else {
                    "OAuth 已配置，请在 Spotify 官方页面授权。"
                }
                .into(),
                policy_notice: SPOTIFY_POLICY_NOTICE.into(),
            },
        }
    }

    pub async fn disconnect(&self, auth_session: Option<&str>) -> bool {
        let Some(session) = auth_session else {
            return false;
        };
        let removed = self.sessions.write().await.remove(session).is_some();
        if removed {
            let _ = self.persist_sessions().await;
        }
        removed
    }

    pub async fn list_playlists(
        &self,
        auth_session: Option<&str>,
    ) -> Result<Vec<SpotifyPlaylistSummary>> {
        let token = self.token(auth_session).await?;
        self.list_playlists_with_token(&token).await
    }

    async fn list_playlists_with_token(
        &self,
        token: &TokenSet,
    ) -> Result<Vec<SpotifyPlaylistSummary>> {
        let config = self.config.as_ref().context("Spotify OAuth 未配置")?;
        let mut playlists = Vec::new();
        let mut offset = 0_u32;
        loop {
            let payload: Value = self
                .client
                .get(format!(
                    "{}/me/playlists",
                    config.api_base_url.trim_end_matches('/')
                ))
                .bearer_auth(&token.access_token)
                .query(&[("limit", 50_u32), ("offset", offset)])
                .send()
                .await?
                .error_for_status()?
                .json()
                .await?;
            let items = payload
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for item in &items {
                let Some(id) = item.get("id").and_then(Value::as_str) else {
                    continue;
                };
                playlists.push(SpotifyPlaylistSummary {
                    id: id.into(),
                    name: item
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or("未命名歌单")
                        .into(),
                    owner_name: item
                        .pointer("/owner/display_name")
                        .or_else(|| item.pointer("/owner/id"))
                        .and_then(Value::as_str)
                        .unwrap_or("Spotify 用户")
                        .into(),
                    track_count: item
                        .pointer("/items/total")
                        .or_else(|| item.pointer("/tracks/total"))
                        .and_then(Value::as_u64)
                        .unwrap_or(0) as usize,
                    collaborative: item
                        .get("collaborative")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                    public: item.get("public").and_then(Value::as_bool),
                    spotify_url: item
                        .pointer("/external_urls/spotify")
                        .and_then(Value::as_str)
                        .map(str::to_string),
                    image_url: item
                        .get("images")
                        .and_then(Value::as_array)
                        .and_then(|images| images.first())
                        .and_then(|image| image.get("url"))
                        .and_then(Value::as_str)
                        .map(str::to_string),
                });
            }
            let total = payload
                .get("total")
                .and_then(Value::as_u64)
                .unwrap_or(playlists.len() as u64) as usize;
            offset += items.len() as u32;
            if items.is_empty() || offset as usize >= total {
                break;
            }
        }
        Ok(playlists)
    }

    pub async fn import_playlists(
        &self,
        playlist_ids: &[String],
        auth_session: Option<&str>,
    ) -> Result<SpotifyImportResult> {
        if playlist_ids.is_empty() {
            bail!("请至少选择一个 Spotify 歌单");
        }
        if playlist_ids.len() > 20 {
            bail!("一次最多选择 20 个 Spotify 歌单");
        }
        let token = self.token(auth_session).await?;
        let accessible = self.list_playlists_with_token(&token).await?;
        let accessible_by_id: HashMap<_, _> = accessible
            .into_iter()
            .map(|playlist| (playlist.id.clone(), playlist))
            .collect();
        self.import_playlist_summaries(playlist_ids, &accessible_by_id, &token)
            .await
    }

    async fn import_playlist_summaries(
        &self,
        playlist_ids: &[String],
        accessible_by_id: &HashMap<String, SpotifyPlaylistSummary>,
        token: &TokenSet,
    ) -> Result<SpotifyImportResult> {
        let mut tracks = Vec::new();
        let mut imported_playlists = Vec::new();
        let mut seen_track_ids = std::collections::HashSet::new();
        for playlist_id in playlist_ids {
            let summary = accessible_by_id
                .get(playlist_id)
                .with_context(|| format!("歌单 {playlist_id} 不在当前账号可访问列表中"))?;
            let before = tracks.len();
            let mut offset = 0_u32;
            loop {
                let payload: Value = self
                    .client
                    .get(format!(
                        "{}/playlists/{}/items",
                        self.config
                            .as_ref()
                            .context("Spotify OAuth 未配置")?
                            .api_base_url
                            .trim_end_matches('/'),
                        urlencoding::encode(playlist_id)
                    ))
                    .bearer_auth(&token.access_token)
                    .query(&[("limit", 50_u32), ("offset", offset)])
                    .send()
                    .await?
                    .error_for_status()?
                    .json()
                    .await?;
                let items = payload
                    .get("items")
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                for wrapper in &items {
                    let item = wrapper
                        .get("item")
                        .or_else(|| wrapper.get("track"))
                        .unwrap_or(wrapper);
                    if item
                        .get("type")
                        .and_then(Value::as_str)
                        .is_some_and(|kind| kind != "track")
                    {
                        continue;
                    }
                    let Some(track) = spotify_track(item) else {
                        continue;
                    };
                    if seen_track_ids.insert(track.id.clone()) {
                        tracks.push(track);
                    }
                }
                let total = payload
                    .get("total")
                    .and_then(Value::as_u64)
                    .unwrap_or((offset as usize + items.len()) as u64)
                    as usize;
                offset += items.len() as u32;
                if items.is_empty() || offset as usize >= total {
                    break;
                }
            }
            imported_playlists.push(SpotifyImportedPlaylist {
                id: summary.id.clone(),
                name: summary.name.clone(),
                spotify_url: summary.spotify_url.clone(),
                imported_count: tracks.len() - before,
            });
        }
        let track_count = tracks.len();
        Ok(SpotifyImportResult {
            playlists: imported_playlists,
            tracks,
            track_count,
            data_use: crate::platforms::spotify_data_use(),
            policy_notice: SPOTIFY_POLICY_NOTICE.into(),
            attribution: "曲目元数据来自 Spotify Web API；请在 Spotify 中查看和播放。".into(),
        })
    }

    pub async fn import_playlist_link(
        &self,
        id: &str,
        session: Option<&str>,
    ) -> Result<SpotifyImportResult> {
        if id.len() != 22 || !id.bytes().all(|b| b.is_ascii_alphanumeric()) {
            bail!("歌单 ID 格式无效");
        }
        let token = self.token(session).await?;
        let config = self.config.as_ref().context("Spotify OAuth 未配置")?;
        let metadata: Value = self
            .client
            .get(format!(
                "{}/playlists/{}",
                config.api_base_url.trim_end_matches('/'),
                id
            ))
            .bearer_auth(&token.access_token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if metadata.get("id").and_then(Value::as_str) != Some(id) {
            bail!("Spotify 返回的歌单身份不匹配");
        }
        let summary = SpotifyPlaylistSummary {
            id: id.into(),
            name: metadata
                .get("name")
                .and_then(Value::as_str)
                .context("歌单缺少名称")?
                .into(),
            owner_name: String::new(),
            track_count: 0,
            collaborative: false,
            public: metadata.get("public").and_then(Value::as_bool),
            spotify_url: Some(format!("https://open.spotify.com/playlist/{id}")),
            image_url: None,
        };
        self.import_playlist_summaries(&[id.into()], &HashMap::from([(id.into(), summary)]), &token)
            .await
    }

    pub async fn begin_authorization(&self) -> Result<String> {
        self.begin_authorization_for(false).await
    }

    pub async fn begin_authorization_for(&self, write: bool) -> Result<String> {
        if !self.is_configured() {
            bail!("CONFIG_REQUIRED: Spotify OAuth 未配置或回调地址无效");
        }
        let config = self
            .config
            .as_ref()
            .context("Spotify OAuth 未配置：请设置 SPOTIFY_CLIENT_ID 与 SPOTIFY_CLIENT_SECRET。")?;
        let state = random_token();
        {
            let mut pending = self.pending_states.write().await;
            pending.retain(|_, created| now_secs().saturating_sub(*created) <= 600);
            pending.insert(state.clone(), now_secs());
        }
        Ok(format!(
            "{}?response_type=code&client_id={}&scope={}&redirect_uri={}&state={}",
            config.accounts_authorize_url,
            urlencoding::encode(&config.client_id),
            urlencoding::encode(&if write {
                format!("{} {}", SCOPES, WRITE_SCOPE)
            } else {
                SCOPES.into()
            }),
            urlencoding::encode(&config.redirect_uri),
            urlencoding::encode(&state),
        ))
    }

    pub async fn complete_authorization(
        &self,
        code: &str,
        state: &str,
    ) -> Result<(String, String)> {
        let config = self.config.as_ref().context("Spotify OAuth 未配置")?;
        let created = self
            .pending_states
            .write()
            .await
            .remove(state)
            .context("OAuth state 无效或已使用")?;
        if now_secs().saturating_sub(created) > 600 {
            bail!("OAuth state 已过期，请重新授权");
        }
        tracing::info!(provider = "spotify", oauth_stage = "state_validated");
        let token: TokenResponse = self
            .client
            .post(&config.accounts_token_url)
            .header(
                "Authorization",
                format!(
                    "Basic {}",
                    STANDARD.encode(format!("{}:{}", config.client_id, config.client_secret))
                ),
            )
            .form(&[
                ("grant_type", "authorization_code"),
                ("code", code),
                ("redirect_uri", config.redirect_uri.as_str()),
            ])
            .send()
            .await
            .context("无法连接 Spotify token endpoint")?
            .error_for_status()
            .context("Spotify 拒绝了授权码")?
            .json()
            .await
            .context("Spotify token 响应无法解析")?;
        if token.access_token.trim().is_empty() {
            bail!("Spotify token 响应缺少有效 access token");
        }
        let profile: Value = self
            .client
            .get(format!("{}/me", config.api_base_url.trim_end_matches('/')))
            .bearer_auth(&token.access_token)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        let user_id = profile
            .get("id")
            .and_then(Value::as_str)
            .context("Spotify 用户资料缺少 id")?
            .to_string();
        let display_name = profile
            .get("display_name")
            .and_then(Value::as_str)
            .unwrap_or(&user_id)
            .to_string();
        let avatar_url = profile
            .get("images")
            .and_then(Value::as_array)
            .and_then(|images| images.first())
            .and_then(|image| image.get("url"))
            .and_then(Value::as_str)
            .map(str::to_string);
        tracing::info!(
            provider = "spotify",
            oauth_stage = "token_and_identity_validated"
        );
        let session = random_token();
        self.sessions.write().await.insert(
            session.clone(),
            TokenSet {
                granted_scopes: token.scope,
                access_token: token.access_token,
                refresh_token: token.refresh_token,
                expires_at: now_secs() + token.expires_in.saturating_sub(30),
                spotify_user_id: user_id,
                display_name,
                avatar_url,
            },
        );
        self.persist_sessions().await?;
        tracing::info!(provider = "spotify", oauth_stage = "session_saved");
        Ok((session, config.frontend_url.clone()))
    }

    async fn token(&self, session: Option<&str>) -> Result<TokenSet> {
        let session = session.context("尚未授权 Spotify")?;
        let existing = self
            .sessions
            .read()
            .await
            .get(session)
            .cloned()
            .context("Spotify 会话无效或服务已重启，请重新授权")?;
        if existing.expires_at > now_secs() {
            return Ok(existing);
        }
        let refresh = existing
            .refresh_token
            .clone()
            .context("Spotify token 已过期且无法刷新，请重新授权")?;
        let config = self.config.as_ref().context("Spotify OAuth 未配置")?;
        let response: TokenResponse = self
            .client
            .post(&config.accounts_token_url)
            .header(
                "Authorization",
                format!(
                    "Basic {}",
                    STANDARD.encode(format!("{}:{}", config.client_id, config.client_secret))
                ),
            )
            .form(&[
                ("grant_type", "refresh_token"),
                ("refresh_token", refresh.as_str()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        if response.access_token.trim().is_empty() {
            bail!("Spotify 刷新响应缺少有效 access token");
        }
        let refreshed = TokenSet {
            granted_scopes: if response.scope.is_empty() {
                existing.granted_scopes.clone()
            } else {
                response.scope
            },
            access_token: response.access_token,
            refresh_token: response.refresh_token.or(existing.refresh_token),
            expires_at: now_secs() + response.expires_in.saturating_sub(30),
            spotify_user_id: existing.spotify_user_id,
            display_name: existing.display_name,
            avatar_url: existing.avatar_url,
        };
        self.sessions
            .write()
            .await
            .insert(session.to_string(), refreshed.clone());
        self.persist_sessions().await?;
        Ok(refreshed)
    }

    async fn search_query(&self, access: &str, query: &str) -> Result<Vec<Value>> {
        let config = self.config.as_ref().context("Spotify OAuth 未配置")?;
        let payload: Value = self
            .client
            .get(format!(
                "{}/search",
                config.api_base_url.trim_end_matches('/')
            ))
            .bearer_auth(access)
            .query(&[
                ("q", query),
                ("type", "track"),
                ("limit", "5"),
                ("market", config.market.as_str()),
            ])
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(payload
            .pointer("/tracks/items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default())
    }

    fn candidate(&self, source: &Track, item: &Value) -> Option<MatchCandidate> {
        let title = item.get("name")?.as_str()?.to_string();
        let artists: Vec<String> = item
            .get("artists")?
            .as_array()?
            .iter()
            .filter_map(|a| a.get("name")?.as_str().map(str::to_string))
            .collect();
        let id = item.get("id")?.as_str()?.to_string();
        let album = item
            .pointer("/album/name")
            .and_then(Value::as_str)
            .map(str::to_string);
        let duration = item
            .get("duration_ms")
            .and_then(Value::as_u64)
            .map(|v| v as u32);
        let title_score = token_similarity(&source.title, &title);
        let source_artists = source.artists.join(" ");
        let artist_score = token_similarity(&source_artists, &artists.join(" "));
        let album_score = match (&source.album, &album) {
            (Some(a), Some(b)) => token_similarity(a, b),
            _ => 0.5,
        };
        let duration_score = match (source.duration_ms, duration) {
            (Some(a), Some(b)) => (1.0 - (a.abs_diff(b) as f32 / 15_000.0)).clamp(0.0, 1.0),
            _ => 0.5,
        };
        let source_isrc = source.external_ids.get("isrc");
        let target_isrc = item.pointer("/external_ids/isrc").and_then(Value::as_str);
        let isrc_exact = source_isrc
            .zip(target_isrc)
            .is_some_and(|(a, b)| a.eq_ignore_ascii_case(b));
        let source_version = source.version_type.clone();
        let target_version = detect_version(&title);
        let version_score = if source_version == target_version {
            1.0
        } else {
            0.2
        };
        let confidence = if isrc_exact {
            0.99
        } else {
            (title_score * 0.45
                + artist_score * 0.30
                + album_score * 0.10
                + duration_score * 0.10
                + version_score * 0.05)
                .clamp(0.0, 1.0)
        };
        let available = item
            .pointer("/is_playable")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        Some(MatchCandidate {
            target_track_id: id,
            title,
            artists,
            album,
            duration_ms: duration,
            channel_name: None,
            official_status: "spotify_catalog".into(),
            target_url: item
                .pointer("/external_urls/spotify")
                .and_then(Value::as_str)
                .map(str::to_string),
            version_type: target_version,
            confidence,
            match_reason: if isrc_exact {
                "ISRC 精确匹配".into()
            } else {
                format!(
                    "歌名 {:.0}% · 歌手 {:.0}% · 专辑/时长/版本综合",
                    title_score * 100.0,
                    artist_score * 100.0
                )
            },
            available_in_market: available,
        })
    }
}

fn spotify_track(item: &Value) -> Option<Track> {
    let spotify_id = item.get("id")?.as_str()?.to_string();
    let title = item.get("name")?.as_str()?.to_string();
    let artists: Vec<String> = item
        .get("artists")?
        .as_array()?
        .iter()
        .filter_map(|artist| artist.get("name")?.as_str().map(str::to_string))
        .collect();
    if artists.is_empty() {
        return None;
    }
    let mut external_ids = HashMap::from([("spotify".into(), spotify_id.clone())]);
    if let Some(isrc) = item.pointer("/external_ids/isrc").and_then(Value::as_str) {
        external_ids.insert("isrc".into(), isrc.into());
    }
    let release_year = item
        .pointer("/album/release_date")
        .and_then(Value::as_str)
        .and_then(|date| date.get(..4))
        .and_then(|year| year.parse::<u16>().ok());
    Some(Track {
        id: format!("spotify:{spotify_id}"),
        normalized_title: normalize_text(&title),
        version_type: detect_version(&title),
        title,
        artists,
        album: item
            .pointer("/album/name")
            .and_then(Value::as_str)
            .map(str::to_string),
        genres: vec![],
        release_year,
        language: None,
        duration_ms: item
            .get("duration_ms")
            .and_then(Value::as_u64)
            .and_then(|duration| u32::try_from(duration).ok()),
        platform: "spotify".into(),
        platform_url: item
            .pointer("/external_urls/spotify")
            .and_then(Value::as_str)
            .map(str::to_string),
        external_ids,
        mood_tags: vec![],
        energy_score: None,
        popularity: None,
        metadata_confidence: 1.0,
    })
}

#[async_trait]
impl PlaylistWriter for SpotifyPlaylistWriter {
    fn platform(&self) -> &'static str {
        "spotify"
    }

    async fn authorize(&self, auth_session: Option<&str>) -> Result<WriterStatus> {
        if !self.is_configured() {
            return Ok(status(
                "spotify",
                "Spotify",
                "needs_configuration",
                false,
                false,
                "需要设置 Spotify OAuth 环境变量；P0 导出和 Demo 不受影响。",
            ));
        }
        let authorized = self.token(auth_session).await.is_ok_and(|token| {
            token
                .granted_scopes
                .split_whitespace()
                .any(|scope| scope == WRITE_SCOPE)
        });
        Ok(status(
            "spotify",
            "Spotify",
            "available",
            authorized,
            false,
            if authorized {
                "已授权；写入前仍会要求预览并确认。"
            } else {
                "读取连接与写入授权分开；如需创建私有歌单，请单独授权写入。"
            },
        ))
    }

    async fn search_track(
        &self,
        track: &Track,
        auth_session: Option<&str>,
    ) -> Result<PlatformTrackMatch> {
        if track.platform == "spotify"
            && let Some(spotify_id) = track.external_ids.get("spotify")
        {
            return Ok(PlatformTrackMatch {
                source_track: track.clone(),
                target_platform: "spotify".into(),
                target_track_id: Some(spotify_id.clone()),
                target_url: track.platform_url.clone(),
                version_type: track.version_type.clone(),
                confidence: 1.0,
                match_reason: "同一 Spotify Track ID，直接用于用户发起的歌单传输".into(),
                status: MatchStatus::Matched,
                candidates: vec![],
            });
        }
        let token = self.token(auth_session).await?;
        let mut items = Vec::new();
        if let Some(isrc) = track.external_ids.get("isrc") {
            items = self
                .search_query(&token.access_token, &format!("isrc:{isrc}"))
                .await?;
        }
        if items.is_empty() {
            let query = format!("track:{} artist:{}", track.title, track.artists.join(" "));
            items = self.search_query(&token.access_token, &query).await?;
        }
        let mut candidates: Vec<_> = items
            .iter()
            .filter_map(|item| self.candidate(track, item))
            .collect();
        candidates.sort_by(|a, b| b.confidence.total_cmp(&a.confidence));
        let best = candidates.first().cloned();
        let (status_value, id, url, version, confidence, reason) = match best {
            Some(ref candidate)
                if candidate.confidence >= 0.88 && candidate.available_in_market =>
            {
                (
                    MatchStatus::Matched,
                    Some(candidate.target_track_id.clone()),
                    candidate.target_url.clone(),
                    candidate.version_type.clone(),
                    candidate.confidence,
                    candidate.match_reason.clone(),
                )
            }
            Some(ref candidate) if candidate.confidence >= 0.65 => (
                MatchStatus::NeedsConfirmation,
                None,
                None,
                candidate.version_type.clone(),
                candidate.confidence,
                format!("{}；请确认版本或地区可用性", candidate.match_reason),
            ),
            Some(ref candidate) => (
                MatchStatus::Unmatched,
                None,
                None,
                candidate.version_type.clone(),
                candidate.confidence,
                "候选置信度过低，未自动匹配".into(),
            ),
            None => (
                MatchStatus::Unmatched,
                None,
                None,
                VersionType::Unknown,
                0.0,
                "Spotify 未找到候选歌曲".into(),
            ),
        };
        Ok(PlatformTrackMatch {
            source_track: track.clone(),
            target_platform: "spotify".into(),
            target_track_id: id,
            target_url: url,
            version_type: version,
            confidence,
            match_reason: reason,
            status: status_value,
            candidates,
        })
    }

    async fn create_playlist(
        &self,
        name: &str,
        auth_session: Option<&str>,
    ) -> Result<CreatedPlaylist> {
        let token = self.token(auth_session).await?;
        if !token
            .granted_scopes
            .split_whitespace()
            .any(|scope| scope == WRITE_SCOPE)
        {
            bail!(
                "WRITE_AUTH_REQUIRED：当前会话没有确认过歌单写入权限，请从 Copy Playlist 单独授权写入"
            );
        }
        let response: Value = self.client
            .post(format!(
                "{}/me/playlists",
                self.config
                    .as_ref()
                    .context("Spotify OAuth 未配置")?
                    .api_base_url
                    .trim_end_matches('/')
            ))
            .bearer_auth(&token.access_token)
            .json(&json!({ "name": name, "public": false, "description": "由 MelodyPath 在用户确认后创建的新歌单" }))
            .send().await?.error_for_status()?.json().await?;
        Ok(CreatedPlaylist {
            id: response
                .get("id")
                .and_then(Value::as_str)
                .context("Spotify 创建歌单响应缺少 id")?
                .into(),
            url: response
                .pointer("/external_urls/spotify")
                .and_then(Value::as_str)
                .map(str::to_string),
        })
    }

    async fn add_tracks(
        &self,
        playlist_id: &str,
        track_ids: &[String],
        auth_session: Option<&str>,
    ) -> Result<AddTracksOutcome> {
        let token = self.token(auth_session).await?;
        if !token
            .granted_scopes
            .split_whitespace()
            .any(|scope| scope == WRITE_SCOPE)
        {
            bail!(
                "WRITE_AUTH_REQUIRED：当前会话没有确认过歌单写入权限，请从 Copy Playlist 单独授权写入"
            );
        }
        let mut added_ids = Vec::new();
        let mut failures = HashMap::new();
        for batch in track_ids.chunks(100) {
            let uris: Vec<_> = batch
                .iter()
                .map(|id| format!("spotify:track:{id}"))
                .collect();
            let response = self
                .client
                .post(format!(
                    "{}/playlists/{}/items",
                    self.config
                        .as_ref()
                        .context("Spotify OAuth 未配置")?
                        .api_base_url
                        .trim_end_matches('/'),
                    urlencoding::encode(playlist_id)
                ))
                .bearer_auth(&token.access_token)
                .json(&json!({ "uris": uris }))
                .send()
                .await?;
            if response.status().is_success() {
                added_ids.extend(batch.iter().cloned());
            } else {
                let status_code = response.status();
                let message = response.text().await.unwrap_or_default();
                for id in batch {
                    failures.insert(id.clone(), format!("Spotify {}: {}", status_code, message));
                }
            }
        }
        Ok(AddTracksOutcome {
            added_ids,
            failures,
        })
    }
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
    STANDARD.encode(bytes).replace(['/', '+', '='], "")
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
    use axum::{
        Json, Router,
        extract::Query,
        routing::{get, post},
    };

    fn test_config() -> SpotifyConfig {
        SpotifyConfig {
            client_id: "client-id-sensitive".into(),
            client_secret: "client-secret-sensitive".into(),
            redirect_uri: DEFAULT_SPOTIFY_REDIRECT_URI.into(),
            frontend_url: "http://127.0.0.1:5173".into(),
            market: "US".into(),
            accounts_authorize_url: "http://127.0.0.1:9/authorize".into(),
            accounts_token_url: "http://127.0.0.1:9/token".into(),
            api_base_url: "http://127.0.0.1:9/v1".into(),
        }
    }

    #[test]
    fn unconfigured_spotify_is_a_valid_state() {
        let writer = SpotifyPlaylistWriter::new();
        if std::env::var("SPOTIFY_CLIENT_ID").is_err() {
            assert!(!writer.is_configured());
        }
    }

    #[test]
    fn normalized_version_detection_distinguishes_live() {
        assert_ne!(detect_version("Song (Live)"), detect_version("Song"));
        assert_eq!(crate::normalize::normalize_text("The 1975"), "the 1975");
    }

    #[tokio::test]
    async fn rejects_forged_oauth_state_before_network_request() {
        let writer = SpotifyPlaylistWriter::from_config(Some(test_config()), Client::new());
        let error = writer
            .complete_authorization("code", "forged")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("state"));
    }

    #[tokio::test]
    async fn disconnect_removes_only_the_selected_session() {
        let writer = SpotifyPlaylistWriter::from_config(Some(test_config()), Client::new());
        writer.sessions.write().await.insert(
            "session".into(),
            TokenSet {
                granted_scopes: String::new(),
                access_token: "access-sensitive".into(),
                refresh_token: Some("refresh-sensitive".into()),
                expires_at: now_secs() + 300,
                spotify_user_id: "user".into(),
                display_name: "User".into(),
                avatar_url: None,
            },
        );
        assert!(writer.disconnect(Some("session")).await);
        assert!(!writer.disconnect(Some("session")).await);
    }

    #[test]
    fn debug_and_configuration_response_never_expose_secrets() {
        let config = test_config();
        let debug = format!("{config:?}");
        assert!(!debug.contains("client-secret-sensitive"));
        assert!(!debug.contains("client-id-sensitive"));
        let writer = SpotifyPlaylistWriter::from_config(Some(config), Client::new());
        let response = serde_json::to_string(&writer.configuration_status()).unwrap();
        assert!(!response.contains("client-secret-sensitive"));
        assert!(!response.contains("client-id-sensitive"));
    }

    #[tokio::test]
    async fn refreshes_token_and_paginates_playlists_and_tracks() {
        async fn token_endpoint() -> Json<Value> {
            Json(
                json!({"access_token":"fresh-access","refresh_token":"fresh-refresh","expires_in":3600}),
            )
        }
        async fn playlists(Query(query): Query<HashMap<String, String>>) -> Json<Value> {
            let offset = query
                .get("offset")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            let items = if offset < 2 {
                vec![
                    json!({"id":format!("p{offset}"),"name":format!("List {offset}"),"owner":{"id":"owner"},"items":{"total":2}}),
                ]
            } else {
                vec![]
            };
            Json(json!({"items":items,"total":2}))
        }
        async fn playlist_items(Query(query): Query<HashMap<String, String>>) -> Json<Value> {
            let offset = query
                .get("offset")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            let items = if offset < 2 {
                vec![
                    json!({"item":{"id":format!("t{offset}"),"name":format!("Track {offset}"),"type":"track","artists":[{"name":"Artist"}],"album":{"name":"Album","release_date":"2024"},"duration_ms":180000,"external_urls":{"spotify":format!("https://open.spotify.com/track/t{offset}")},"external_ids":{"isrc":format!("ISRC{offset}")}}}),
                ]
            } else {
                vec![]
            };
            Json(json!({"items":items,"total":2}))
        }
        let app = Router::new()
            .route("/token", post(token_endpoint))
            .route("/v1/me/playlists", get(playlists))
            .route("/v1/playlists/p0/items", get(playlist_items));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let mut config = test_config();
        config.accounts_token_url = format!("{base}/token");
        config.api_base_url = format!("{base}/v1");
        let writer = SpotifyPlaylistWriter::from_config(Some(config), Client::new());
        writer.sessions.write().await.insert(
            "session".into(),
            TokenSet {
                granted_scopes: String::new(),
                access_token: "expired".into(),
                refresh_token: Some("refresh".into()),
                expires_at: 0,
                spotify_user_id: "user".into(),
                display_name: "User".into(),
                avatar_url: None,
            },
        );
        let lists = writer.list_playlists(Some("session")).await.unwrap();
        assert_eq!(lists.len(), 2);
        assert_eq!(
            writer.sessions.read().await["session"].access_token,
            "fresh-access"
        );
        let imported = writer
            .import_playlists(&["p0".into()], Some("session"))
            .await
            .unwrap();
        assert_eq!(imported.track_count, 2);
    }
    #[tokio::test]
    async fn missing_config_and_absent_token_never_connect() {
        let writer = SpotifyPlaylistWriter::from_config(None, Client::new());
        assert!(
            writer
                .begin_authorization()
                .await
                .unwrap_err()
                .to_string()
                .contains("CONFIG_REQUIRED")
        );
        let status = writer.connection_status(None).await;
        assert!(!status.configured && !status.connected);
        let writer = SpotifyPlaylistWriter::from_config(Some(test_config()), Client::new());
        assert!(!writer.connection_status(None).await.connected);
        assert!(!writer.connection_status(Some("unknown")).await.connected);
    }

    #[tokio::test]
    async fn expired_state_is_consumed_and_invalid_redirect_is_blocked() {
        let mut config = test_config();
        let writer = SpotifyPlaylistWriter::from_config(Some(config.clone()), Client::new());
        writer
            .pending_states
            .write()
            .await
            .insert("expired".into(), now_secs() - 601);
        assert!(
            writer
                .complete_authorization("synthetic", "expired")
                .await
                .unwrap_err()
                .to_string()
                .contains("过期")
        );
        assert!(!writer.pending_states.read().await.contains_key("expired"));
        config.redirect_uri = "javascript:alert(1)".into();
        let writer = SpotifyPlaylistWriter::from_config(Some(config), Client::new());
        assert!(!writer.is_configured());
        assert!(!writer.configuration_status().configured);
        assert!(writer.begin_authorization().await.is_err());
    }

    #[test]
    fn token_response_requires_access_token() {
        assert!(serde_json::from_value::<TokenResponse>(json!({"expires_in":3600})).is_err());
    }
    // Explicit local HTTP fixtures verify protocol behavior, never real platform access.
    #[tokio::test]
    async fn synthetic_oauth_exchange_connects_only_with_valid_token_and_identity() {
        use axum::{
            Json, Router,
            routing::{get, post},
        };
        for mode in ["valid", "missing", "empty"] {
            let app = Router::new()
                .route("/token", post(move || async move { Json(match mode {
                    "missing" => json!({"expires_in":3600}),
                    "empty" => json!({"access_token":"", "expires_in":3600}),
                    _ => json!({"access_token":"synthetic-access", "refresh_token":"synthetic-refresh", "expires_in":3600}),
                }) }))
                .route("/me", get(|| async { Json(json!({"id":"synthetic-user","display_name":"Synthetic User"})) }));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let base = format!("http://{}", listener.local_addr().unwrap());
            let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let mut config = test_config();
            config.accounts_token_url = format!("{base}/token");
            config.api_base_url = base;
            let writer = SpotifyPlaylistWriter::from_config(Some(config), Client::new());
            let authorize = writer.begin_authorization().await.unwrap();
            let state = reqwest::Url::parse(&authorize)
                .unwrap()
                .query_pairs()
                .find(|(key, _)| key == "state")
                .unwrap()
                .1
                .into_owned();
            let result = writer
                .complete_authorization("synthetic-code", &state)
                .await;
            if mode == "valid" {
                let (session, _) = result.unwrap();
                let status = writer.connection_status(Some(&session)).await;
                assert!(status.connected);
                assert!(
                    !serde_json::to_string(&status)
                        .unwrap()
                        .contains("synthetic-access")
                );
                assert!(
                    writer
                        .complete_authorization("synthetic-code", &state)
                        .await
                        .is_err()
                );
                assert!(writer.disconnect(Some(&session)).await);
                assert!(!writer.connection_status(Some(&session)).await.connected);
            } else {
                assert!(result.is_err());
                assert!(writer.sessions.read().await.is_empty());
            }
            server.abort();
        }
    }

    #[tokio::test]
    async fn least_privilege_authorize_urls_and_readonly_write_guard() {
        let writer = SpotifyPlaylistWriter::from_config(Some(test_config()), Client::new());
        for write in [false, true] {
            let url =
                reqwest::Url::parse(&writer.begin_authorization_for(write).await.unwrap()).unwrap();
            let query: HashMap<_, _> = url.query_pairs().into_owned().collect();
            let expected = if write {
                format!("{} {}", SCOPES, WRITE_SCOPE)
            } else {
                SCOPES.to_string()
            };
            assert_eq!(query["scope"], expected);
            assert!(!query["scope"].contains("user-read-private"));
        }
        writer.sessions.write().await.insert(
            "synthetic-session".into(),
            TokenSet {
                granted_scopes: SCOPES.into(),
                access_token: "synthetic".into(),
                refresh_token: None,
                expires_at: now_secs() + 3600,
                spotify_user_id: "synthetic".into(),
                display_name: "Synthetic".into(),
                avatar_url: None,
            },
        );
        let status = writer.connection_status(Some("synthetic-session")).await;
        assert!(status.connected);
        assert!(!status.write_authorized);
        assert!(
            writer
                .create_playlist("Synthetic", Some("synthetic-session"))
                .await
                .unwrap_err()
                .to_string()
                .contains("WRITE_AUTH_REQUIRED")
        );
        assert!(
            writer
                .import_playlist_link("invalid!", Some("synthetic-session"))
                .await
                .is_err()
        );
        assert!(
            writer
                .add_tracks(
                    "synthetic-list",
                    &["synthetic-track".into()],
                    Some("synthetic-session")
                )
                .await
                .unwrap_err()
                .to_string()
                .contains("WRITE_AUTH_REQUIRED")
        );
        let valid_id = "1234567890123456789012";
        assert!(writer.import_playlist_link(valid_id, None).await.is_err());
        writer
            .sessions
            .write()
            .await
            .get_mut("synthetic-session")
            .unwrap()
            .expires_at = 0;
        assert!(
            !writer
                .connection_status(Some("synthetic-session"))
                .await
                .connected
        );
        assert!(
            writer
                .import_playlist_link(valid_id, Some("synthetic-session"))
                .await
                .is_err()
        );
    }

    // Synthetic HTTP fixture, never proof of real platform acceptance.
    #[tokio::test]
    async fn authorized_public_link_reads_official_pagination_and_propagates_failures() {
        use axum::{Json, Router, extract::Query, http::HeaderMap, routing::get};
        async fn metadata(headers: HeaderMap) -> Json<Value> {
            assert_eq!(headers["authorization"], "Bearer synthetic-access");
            Json(
                json!({"id":"1234567890123456789012","name":"Synthetic public list","owner":{"id":"different-user"}}),
            )
        }
        async fn items(
            headers: HeaderMap,
            Query(query): Query<HashMap<String, String>>,
        ) -> Json<Value> {
            assert_eq!(headers["authorization"], "Bearer synthetic-access");
            let page = query
                .get("offset")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(0);
            Json(
                json!({"items":[{"item":{"id":format!("track-{page}"),"name":"Synthetic Song","type":"track","artists":[{"name":"Synthetic Artist"}],"duration_ms":180000}}],"total":2}),
            )
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(
                listener,
                Router::new()
                    .route("/playlists/1234567890123456789012", get(metadata))
                    .route("/playlists/1234567890123456789012/items", get(items)),
            )
            .await
            .unwrap()
        });
        let mut config = test_config();
        config.api_base_url = base;
        let writer = SpotifyPlaylistWriter::from_config(Some(config), Client::new());
        writer.sessions.write().await.insert(
            "synthetic-session".into(),
            TokenSet {
                granted_scopes: SCOPES.into(),
                access_token: "synthetic-access".into(),
                refresh_token: None,
                expires_at: now_secs() + 3600,
                spotify_user_id: "synthetic".into(),
                display_name: "Synthetic".into(),
                avatar_url: None,
            },
        );
        let result = writer
            .import_playlist_link("1234567890123456789012", Some("synthetic-session"))
            .await
            .unwrap();
        assert_eq!(result.track_count, 2);
        assert_eq!(result.tracks.len(), 2);
        assert!(
            result
                .tracks
                .iter()
                .all(|track| track.platform == "spotify")
        );
        assert!(
            writer
                .import_playlist_link("2234567890123456789012", Some("synthetic-session"))
                .await
                .is_err()
        );
        server.abort();
        assert!(
            writer
                .import_playlist_link("1234567890123456789012", Some("synthetic-session"))
                .await
                .is_err()
        );
    }
}
