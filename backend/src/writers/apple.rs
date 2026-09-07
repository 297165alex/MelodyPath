use crate::{
    models::{AppleMusicBootstrap, PlaylistImportRow, ProviderConfigurationStatus, Track},
    normalize::{normalize_text, parse_track_version},
};
use anyhow::{Result, anyhow, bail};
use base64::Engine;
use reqwest::{Client, Url, redirect::Policy};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const API: &str = "https://api.music.apple.com";
#[derive(Clone, Default)]
pub struct AppleMusicConnector {
    developer_token: Option<String>,
}
pub struct CatalogImport {
    pub name: Option<String>,
    pub tracks: Vec<Track>,
    pub rows: Vec<PlaylistImportRow>,
}

/// Catalog only: never guess a storefront or accept a private library ID.
pub fn catalog_link(raw: &str) -> Option<(String, String)> {
    let url = Url::parse(raw.trim()).ok()?;
    if url.scheme() != "https"
        || url.host_str() != Some("music.apple.com")
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return None;
    }
    let parts: Vec<_> = url.path_segments()?.filter(|s| !s.is_empty()).collect();
    if !(parts.len() == 3 || parts.len() == 4)
        || parts[1] != "playlist"
        || parts[0].len() != 2
        || !parts[0].bytes().all(|b| b.is_ascii_alphabetic())
    {
        return None;
    }
    let id = *parts.last()?;
    if !id.starts_with("pl.")
        || !(4..=200).contains(&id.len())
        || !id[3..]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return None;
    }
    Some((parts[0].to_ascii_lowercase(), id.to_owned()))
}

impl AppleMusicConnector {
    pub fn new() -> Self {
        Self {
            developer_token: std::env::var("APPLE_MUSIC_DEVELOPER_TOKEN")
                .ok()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty()),
        }
    }
    pub fn is_configured(&self) -> bool {
        self.developer_token
            .as_deref()
            .is_some_and(developer_token_looks_valid)
    }
    pub fn configuration_status(&self) -> ProviderConfigurationStatus {
        let configured = self.is_configured();
        let variable = "APPLE_MUSIC_DEVELOPER_TOKEN".to_string();
        ProviderConfigurationStatus {
            platform: "apple_music".into(), display_name: "Apple Music · Public Catalog".into(), configured,
            validation_status: if configured { "catalog_configured_not_real_verified" } else { "WAITING_FOR_APPLE_DEVELOPER_CREDENTIALS" }.into(),
            required_environment_variables: vec![variable.clone()],
            present_environment_variables: if self.developer_token.is_some() { vec![variable.clone()] } else { vec![] },
            missing_environment_variables: if self.developer_token.is_none() { vec![variable] } else { vec![] },
            redirect_uri: None,
            dashboard_url: "https://developer.apple.com/account/resources/identifiers/list/mediaServiceId".into(),
            setup_steps: vec![
                "无需现在购买会员。若部署者已有 Apple Developer Program 资格，可创建 Media ID / MusicKit key。".into(),
                "部署者在仓库外使用 Team ID、Key ID 与私钥生成 ES256 Developer Token；本应用不自动生成或读取私钥。".into(),
                "通过后端安全环境配置 APPLE_MUSIC_DEVELOPER_TOKEN，重启后端；定期轮换到期 token。普通用户无需开发者凭据。".into(),
                "粘贴带地区的公开 music.apple.com/.../playlist/.../pl.ID 链接，核对 Import Preview。配置检测不证明 Apple 已接受凭据。".into(),
                "私人资料库需要 Music User Token 与用户官方授权，本轮未接入；不能用目录 API 代替私人歌单读取。".into(),
            ], secrets_exposed_to_frontend: false,
            message: if configured { "OFFICIAL API · 配置格式有效，尚未真人验收；仅公开目录读取，Token 留在后端。" } else { "CONFIG_REQUIRED · WAITING_FOR_APPLE_DEVELOPER_CREDENTIALS；需要有效且未过期的后端 Developer Token。可继续文件/文本导入。" }.into(),
        }
    }
    pub fn bootstrap(&self) -> AppleMusicBootstrap {
        AppleMusicBootstrap {
            configured: false,
            developer_token: None,
            app_name: "MelodyPath".into(),
            app_build: env!("CARGO_PKG_VERSION").into(),
            real_account_validation: "not_completed".into(),
            message: "Web 账号连接未实现。公开目录通过后端读取，不向浏览器下发 Developer Token。"
                .into(),
        }
    }
    pub async fn import_playlist_link(&self, raw: &str) -> Result<CatalogImport> {
        let (storefront, id) =
            catalog_link(raw).ok_or_else(|| anyhow!("INVALID_APPLE_CATALOG_URL"))?;
        if !self.is_configured() {
            bail!("WAITING_FOR_APPLE_DEVELOPER_CREDENTIALS");
        }
        let client = Client::builder()
            .timeout(Duration::from_secs(15))
            .redirect(Policy::none())
            .build()?;
        let token = self.developer_token.as_deref().unwrap_or_default();
        let fetch = |url: String| {
            let client = &client;
            async move { get_json(client, &url, token).await }
        };
        tokio::time::timeout(
            Duration::from_secs(90),
            collect_catalog(&storefront, &id, fetch),
        )
        .await
        .map_err(|_| anyhow!("APPLE_IMPORT_TIMEOUT"))?
    }
}

async fn collect_catalog<F, Fut>(storefront: &str, id: &str, mut fetch: F) -> Result<CatalogImport>
where
    F: FnMut(String) -> Fut,
    Fut: std::future::Future<Output = Result<Value>>,
{
    let path = format!("/v1/catalog/{storefront}/playlists/{id}");
    let metadata = fetch(format!("{API}{path}")).await?;
    let playlist = api_data(&metadata)?
        .first()
        .ok_or_else(|| anyhow!("APPLE_PLAYLIST_NOT_FOUND"))?;
    let mut result = CatalogImport {
        name: string(&playlist["attributes"], "name"),
        tracks: vec![],
        rows: vec![],
    };
    let track_path = format!("{path}/tracks");
    let mut next = Some(format!("{API}{track_path}"));
    let mut visited = HashSet::new();
    let mut seen = HashSet::new();
    while let Some(url) = next.take() {
        if visited.len() >= 200 || !visited.insert(url.clone()) {
            bail!("APPLE_PAGINATION_LIMIT");
        }
        let page = fetch(url).await?;
        let rows = api_data(&page)?;
        if result.rows.len() + rows.len() > 10_000 {
            bail!("APPLE_TRACK_LIMIT");
        }
        for row in rows {
            append_row(&mut result, row, &id, &mut seen);
        }
        next = match page.get("next") {
            None | Some(Value::Null) => None,
            Some(Value::String(value)) => Some(next_url(value, &track_path)?),
            _ => bail!("APPLE_INVALID_PAGINATION"),
        };
    }
    Ok(result)
}

fn api_data(response: &Value) -> Result<&Vec<Value>> {
    // A 200 response containing resource errors must not become partial success.
    if let Some(errors) = response.get("errors")
        && !errors.as_array().is_some_and(Vec::is_empty)
    {
        bail!("APPLE_RESOURCE_ERRORS");
    }
    response
        .get("data")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("APPLE_INVALID_RESPONSE"))
}

// Treat pagination as untrusted input: do not forward credentials to another origin/path.
fn next_url(next: &str, expected_path: &str) -> Result<String> {
    if !next.starts_with('/') || next.starts_with("//") || next.contains('\\') {
        bail!("APPLE_UNSAFE_PAGINATION");
    }
    let url = Url::parse(&format!("{API}{next}"))?;
    if url.scheme() != "https"
        || url.host_str() != Some("api.music.apple.com")
        || url.path() != expected_path
        || url.fragment().is_some()
        || !url.query_pairs().all(|(k, v)| {
            matches!(k.as_ref(), "offset" | "limit")
                && !v.is_empty()
                && v.bytes().all(|b| b.is_ascii_digit())
        })
    {
        bail!("APPLE_UNSAFE_PAGINATION");
    }
    Ok(url.into())
}
async fn get_json(client: &Client, url: &str, token: &str) -> Result<Value> {
    let mut response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|_| anyhow!("APPLE_NETWORK_ERROR"))?;
    if !response.status().is_success() {
        bail!("APPLE_HTTP_{}", response.status().as_u16());
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| anyhow!("APPLE_RESPONSE_ERROR"))?
    {
        if bytes.len() + chunk.len() > 4 * 1024 * 1024 {
            bail!("APPLE_RESPONSE_TOO_LARGE");
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| anyhow!("APPLE_INVALID_JSON"))
}
fn string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}
fn append_row(
    result: &mut CatalogImport,
    resource: &Value,
    playlist_id: &str,
    seen: &mut HashSet<String>,
) {
    let attrs = &resource["attributes"];
    let title = string(attrs, "name");
    let artists: Vec<String> = string(attrs, "artistName").into_iter().collect();
    let duration_ms = attrs["durationInMillis"]
        .as_u64()
        .and_then(|n| u32::try_from(n).ok())
        .filter(|n| *n > 0);
    let source_url = string(attrs, "url").filter(|s| {
        Url::parse(s).is_ok_and(|u| {
            u.scheme() == "https"
                && u.host_str() == Some("music.apple.com")
                && u.username().is_empty()
                && u.password().is_none()
        })
    });
    let id = string(resource, "id");
    let availability = if !attrs.is_object() {
        "UNAVAILABLE"
    } else if attrs["playParams"].is_object() {
        "PLAYABLE_WITH_SUBSCRIPTION"
    } else {
        "UNKNOWN"
    };
    let status = if resource["type"] != "songs" {
        "SKIPPED_UNSUPPORTED_TYPE"
    } else if title.is_none() || id.is_none() {
        "SKIPPED_UNAVAILABLE_METADATA"
    } else if artists.is_empty() {
        "SKIPPED_MISSING_ARTIST"
    } else if !seen.insert(id.clone().unwrap()) {
        "SKIPPED_DUPLICATE"
    } else {
        "IMPORTED"
    };
    result.rows.push(PlaylistImportRow {
        source_platform: "apple_music".into(),
        playlist_id: playlist_id.into(),
        playlist_name: result.name.clone(),
        track_title: title.clone(),
        artist: artists.clone(),
        duration_ms,
        source_url: source_url.clone(),
        availability: availability.into(),
        import_status: status.into(),
    });
    if status != "IMPORTED" {
        return;
    }
    let title = title.unwrap();
    let mut external_ids = HashMap::new();
    if let Some(isrc) = string(attrs, "isrc") {
        external_ids.insert("isrc".into(), isrc);
    }
    result.tracks.push(Track {
        id: id.unwrap(),
        normalized_title: normalize_text(&title),
        version_type: parse_track_version(&title).version_type,
        title,
        artists,
        album: string(attrs, "albumName"),
        genres: vec![],
        release_year: None,
        language: None,
        duration_ms,
        platform: "apple_music".into(),
        platform_url: source_url,
        external_ids,
        mood_tags: vec![],
        energy_score: None,
        popularity: None,
        metadata_confidence: 1.0,
    });
}
fn developer_token_looks_valid(token: &str) -> bool {
    let parts: Vec<_> = token.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|s| s.is_empty()) {
        return false;
    }
    let decode = |s| {
        base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(s)
            .ok()
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
    };
    let (Some(header), Some(payload)) = (decode(parts[0]), decode(parts[1])) else {
        return false;
    };
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    header["alg"] == "ES256"
        && string(&header, "kid").is_some()
        && string(&payload, "iss").is_some()
        && payload["exp"].as_u64().is_some_and(|exp| exp > now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn http_transport_uses_bearer_and_never_exposes_body_or_follows_redirect() {
        use axum::{
            Json, Router,
            http::{HeaderMap, StatusCode},
            response::Redirect,
            routing::get,
        };
        let app = Router::new()
            .route(
                "/ok",
                get(|headers: HeaderMap| async move {
                    assert_eq!(headers.get("authorization").unwrap(), "Bearer synthetic");
                    Json(json!({"data": []}))
                }),
            )
            .route(
                "/denied",
                get(|| async { (StatusCode::FORBIDDEN, "private-error-body-must-not-surface") }),
            )
            .route("/redirect", get(|| async { Redirect::temporary("/ok") }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .build()
            .unwrap();
        assert_eq!(
            get_json(&client, &format!("http://{address}/ok"), "synthetic")
                .await
                .unwrap(),
            json!({"data":[]})
        );
        assert_eq!(
            get_json(&client, &format!("http://{address}/denied"), "synthetic")
                .await
                .unwrap_err()
                .to_string(),
            "APPLE_HTTP_403"
        );
        assert_eq!(
            get_json(&client, &format!("http://{address}/redirect"), "synthetic")
                .await
                .unwrap_err()
                .to_string(),
            "APPLE_HTTP_307"
        );
        server.abort();
    }

    #[tokio::test]
    async fn catalog_pagination_collects_all_rows_and_rejects_partial_success() {
        let mut pages = std::collections::VecDeque::from([
            json!({"data":[{"attributes":{"name":"Synthetic catalog"}}]}),
            json!({"data":[{"id":"1","type":"songs","attributes":{"name":"One","artistName":"Artist"}}],"next":"/v1/catalog/us/playlists/pl.test/tracks?offset=1"}),
            json!({"data":[{"id":"2","type":"songs","attributes":{"name":"Two","artistName":"Artist"}}]}),
        ]);
        let mut urls = vec![];
        let result = collect_catalog("us", "pl.test", |url| {
            urls.push(url);
            let page = pages.pop_front().unwrap();
            async { Ok(page) }
        })
        .await
        .unwrap();
        assert_eq!(result.tracks.len(), 2);
        assert_eq!(result.rows.len(), 2);
        assert!(urls[2].ends_with("tracks?offset=1"));
        assert_eq!(result.name.as_deref(), Some("Synthetic catalog"));
        for bad in [
            json!({"data":[],"next":"//evil.example"}),
            json!({"errors":[]}),
            json!({"data":[],"next":42}),
            json!({"data":[{"id":"1","type":"songs","attributes":{"name":"Partial","artistName":"Artist"}}],"errors":[{"code":"synthetic"}]}),
        ] {
            let mut calls = 0;
            assert!(
                collect_catalog("us", "pl.test", |_| {
                    calls += 1;
                    let page = if calls == 1 {
                        json!({"data":[{"attributes":{"name":"Synthetic"}}]})
                    } else {
                        bad.clone()
                    };
                    async { Ok(page) }
                })
                .await
                .is_err()
            );
        }
        let mut calls = 0;
        assert!(collect_catalog("us", "pl.test", |_| {
            calls += 1;
            let result = match calls {
                1 => Ok(json!({"data":[{}]})),
                2 => Ok(json!({"data":[{"id":"1","type":"songs","attributes":{"name":"One","artistName":"Artist"}}],"next":"/v1/catalog/us/playlists/pl.test/tracks?offset=1"})),
                _ => Err(anyhow!("APPLE_HTTP_429")),
            };
            async { result }
        }).await.is_err());
    }

    #[test]
    fn configured_bootstrap_never_discloses_token_and_expiry_is_checked() {
        let encode =
            |v: Value| base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v.to_string());
        // Deliberately unsigned synthetic fixture: shape checking is not Apple validation.
        let token = format!(
            "{}.{}.synthetic",
            encode(json!({"alg":"ES256","kid":"synthetic"})),
            encode(json!({"iss":"synthetic","exp":4102444800u64}))
        );
        let connector = AppleMusicConnector {
            developer_token: Some(token),
        };
        assert!(connector.is_configured());
        assert!(connector.bootstrap().developer_token.is_none());
        let serialized = serde_json::to_string(&connector.configuration_status()).unwrap();
        assert!(!serialized.contains("synthetic"));
        let expired = format!(
            "{}.{}.synthetic",
            encode(json!({"alg":"ES256","kid":"synthetic"})),
            encode(json!({"iss":"synthetic","exp":1}))
        );
        assert!(!developer_token_looks_valid(&expired));
    }
    #[test]
    fn public_url_variants_and_invalid_urls() {
        for raw in [
            "https://music.apple.com/cn/playlist/中文/pl.abc123?l=en",
            "https://music.apple.com/US/playlist/pl.u-abc123/",
        ] {
            assert!(catalog_link(raw).is_some());
        }
        for raw in [
            "https://music.apple.com/playlist/pl.abc",
            "https://music.apple.com/us/album/name/123",
            "https://music.apple.com/us/playlist/name/p.private",
            "http://music.apple.com/us/playlist/pl.abc",
            "https://music.apple.com.evil/us/playlist/pl.abc",
            "https://user@music.apple.com/us/playlist/pl.abc",
            "https://music.apple.com/us/playlist/pl.%2Fbad",
            "invalid",
        ] {
            assert!(catalog_link(raw).is_none(), "{raw}");
        }
    }
    #[test]
    fn pagination_cannot_leak_credentials() {
        let path = "/v1/catalog/us/playlists/pl.test/tracks";
        assert!(next_url(&format!("{path}?offset=100&limit=100"), path).is_ok());
        for raw in [
            "https://evil.test/",
            "//evil.test/",
            "/v1/me/library/playlists",
            "/v1/catalog/us/playlists/pl.other/tracks",
            "/v1/catalog/us/playlists/pl.test/tracks?offset=bad",
            "/v1/catalog/us/playlists/pl.test/tracks#bad",
        ] {
            assert!(next_url(raw, path).is_err());
        }
    }
    #[test]
    fn conversion_reports_every_source_row_without_fabrication() {
        let mut result = CatalogImport {
            name: Some("Synthetic list".into()),
            tracks: vec![],
            rows: vec![],
        };
        let mut seen = HashSet::new();
        for row in [
            json!({"id":"1","type":"songs","attributes":{"name":"  中文 日本語 한국어  ","artistName":"  Synthetic 艺人 ","playParams":{"id":"1"}}}),
            json!({"id":"1","type":"songs","attributes":{"name":"中文 日本語 한국어","artistName":"Synthetic 艺人"}}),
            json!({"id":"2","type":"songs","attributes":{"name":"Missing artist"}}),
            json!({"id":"3","type":"songs"}),
            json!({"id":"4","type":"music-videos","attributes":{"name":"Video"}}),
            json!({"id":"5","type":"songs","attributes":{"name":"Other","artistName":"Artist","durationInMillis":180000}}),
        ] {
            append_row(&mut result, &row, "pl.test", &mut seen);
        }
        assert_eq!(result.rows.len(), 6);
        assert_eq!(result.tracks.len(), 2);
        assert_eq!(result.rows[1].import_status, "SKIPPED_DUPLICATE");
        assert_eq!(result.rows[2].import_status, "SKIPPED_MISSING_ARTIST");
        assert_eq!(result.rows[3].availability, "UNAVAILABLE");
        assert_eq!(result.rows[5].availability, "UNKNOWN");
        let transfer = crate::models::TransferTrack::from(&result.tracks[0]);
        assert_eq!(transfer.title, "中文 日本語 한국어");
        assert_eq!(transfer.duration_ms, None);
        assert_eq!(transfer.source_url, None);
        assert_eq!(result.tracks[1].duration_ms, Some(180000));
    }
    #[tokio::test]
    async fn unconfigured_stops_before_network() {
        let connector = AppleMusicConnector::default();
        assert!(!connector.is_configured());
        assert!(connector.bootstrap().developer_token.is_none());
        assert_eq!(
            connector
                .import_playlist_link("https://music.apple.com/us/playlist/pl.test")
                .await
                .err()
                .unwrap()
                .to_string(),
            "WAITING_FOR_APPLE_DEVELOPER_CREDENTIALS"
        );
        assert!(!developer_token_looks_valid("replace-me"));
        assert!(!developer_token_looks_valid("a.b.c"));
    }
}
