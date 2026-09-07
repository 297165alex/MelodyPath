//! Single-instance public hosting boundary: same origin, signed browser workspaces,
//! private histories and previews, and bounded request admission.
use crate::{AppState, deployment::Deployment};
use axum::{
    Router,
    extract::{Request, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{any, get},
};
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Instant, SystemTime, UNIX_EPOCH},
};
use tokio::sync::{Mutex, Semaphore};
use tower::ServiceExt;
use tower_http::services::{ServeDir, ServeFile};

const COOKIE: &str = "melody_browser";
const LIFETIME: u64 = 8 * 3600;
const MAX_BROWSERS: usize = 64;

#[derive(Clone)]
struct Host {
    prototype: AppState,
    config: Deployment,
    signing_key: [u8; 32],
    visitors: Arc<Mutex<HashMap<String, Visitor>>>,
    capacity: Arc<Semaphore>,
}
struct Visitor {
    router: Router,
    expiry: u64,
    window: Instant,
    requests: u32,
}

pub fn router(prototype: AppState, config: Deployment) -> anyhow::Result<Router> {
    router_with_key(prototype, config, crate::secure_store::server_key()?)
}

fn router_with_key(
    prototype: AppState,
    config: Deployment,
    key: [u8; 32],
) -> anyhow::Result<Router> {
    let static_dir = config
        .static_dir
        .as_ref()
        .expect("production static directory validated");
    anyhow::ensure!(
        static_dir.join("index.html").is_file(),
        "CONFIG_REQUIRED: frontend index.html not found"
    );
    let static_files =
        ServeDir::new(static_dir).fallback(ServeFile::new(static_dir.join("index.html")));
    let signing_key =
        Sha256::digest([b"MelodyPath/browser-signing/v1".as_slice(), &key].concat()).into();
    let host = Host {
        prototype,
        config,
        signing_key,
        visitors: Default::default(),
        capacity: Arc::new(Semaphore::new(8)),
    };
    Ok(Router::new()
        .route("/health", get(crate::health))
        .route("/api/public-config", get(public_config))
        .route("/api", any(dispatch))
        .route("/api/{*path}", any(dispatch))
        .fallback_service(static_files)
        .with_state(host))
}

async fn public_config(State(host): State<Host>) -> impl IntoResponse {
    (
        [(header::CACHE_CONTROL, "no-store")],
        axum::Json(serde_json::json!({
            "public_demo_expires_at": host.config.demo_expires_at
        })),
    )
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn sign(id: &str, expiry: u64, key: &[u8; 32]) -> String {
    let payload = format!("{id}.{expiry}");
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC key length");
    mac.update(payload.as_bytes());
    format!(
        "{payload}.{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes())
    )
}
fn verify(value: &str, key: &[u8; 32]) -> Option<(String, u64)> {
    let fields: Vec<_> = value.split('.').collect();
    if fields.len() != 3
        || fields[0].len() != 32
        || !fields[0].bytes().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    let expiry = fields[1].parse::<u64>().ok()?;
    if expiry <= now() || expiry > now() + LIFETIME {
        return None;
    }
    let signature = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(fields[2])
        .ok()?;
    let mut mac = Hmac::<Sha256>::new_from_slice(key).ok()?;
    mac.update(format!("{}.{}", fields[0], fields[1]).as_bytes());
    mac.verify_slice(&signature).ok()?;
    Some((fields[0].into(), expiry))
}
fn error(status: StatusCode, message: &str) -> Response {
    (status, axum::Json(serde_json::json!({"error":message}))).into_response()
}

async fn dispatch(State(host): State<Host>, request: Request) -> Response {
    let path = request.uri().path();
    let mutation = !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    );
    let origin = request
        .headers()
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok());
    if origin.is_some_and(|v| v != host.config.origin)
        || (mutation && origin != Some(host.config.origin.as_str()))
        || request
            .headers()
            .get("sec-fetch-site")
            .is_some_and(|v| v == "cross-site")
            && !path.ends_with("/callback")
    {
        return error(
            StatusCode::FORBIDDEN,
            "同域安全校验失败，请从 MelodyPath 首页重试。",
        );
    }
    // Public users must not redirect the deployer's LLM key to a chosen endpoint.
    if (path == "/api/settings" && mutation)
        || path.starts_with("/api/config/") && path.ends_with("/check")
        || path == "/api/apple/musickit/bootstrap"
    {
        return error(StatusCode::FORBIDDEN, "此配置仅由部署者管理。");
    }
    let cookie = request
        .headers()
        .get(header::COOKIE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            v.split(';')
                .find_map(|part| part.trim().strip_prefix("melody_browser="))
        })
        .and_then(|value| verify(value, &host.signing_key));
    let fresh = cookie.is_none();
    if fresh && path != "/api/session" {
        return error(
            StatusCode::UNAUTHORIZED,
            "浏览器会话缺失或过期，请刷新首页后重新连接。",
        );
    }
    let (id, expiry) =
        cookie.unwrap_or_else(|| (uuid::Uuid::new_v4().simple().to_string(), now() + LIFETIME));
    let Ok(_permit) = host.capacity.clone().try_acquire_owned() else {
        return error(StatusCode::TOO_MANY_REQUESTS, "服务繁忙，请稍后重试。");
    };
    let mut visitors = host.visitors.lock().await;
    visitors.retain(|_, v| v.expiry > now());
    if !visitors.contains_key(&id) {
        if visitors.len() >= MAX_BROWSERS {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "课程演示容量已满，请稍后重试。",
            );
        }
        let dir = host.config.data_dir.join("browsers");
        if std::fs::create_dir_all(&dir).is_err() {
            return error(StatusCode::SERVICE_UNAVAILABLE, "无法创建会话存储。");
        }
        let Ok(state) = host
            .prototype
            .for_public_browser(dir.join(format!("{id}.db")))
            .await
        else {
            return error(StatusCode::SERVICE_UNAVAILABLE, "无法初始化会话存储。");
        };
        visitors.insert(
            id.clone(),
            Visitor {
                router: crate::app(state),
                expiry,
                window: Instant::now(),
                requests: 0,
            },
        );
    }
    let visitor = visitors.get_mut(&id).expect("visitor inserted");
    if visitor.window.elapsed().as_secs() >= 60 {
        visitor.window = Instant::now();
        visitor.requests = 0;
    }
    visitor.requests += 1;
    if visitor.requests > 120 {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            "请求过于频繁，请一分钟后重试。",
        );
    }
    let router = visitor.router.clone();
    drop(visitors);
    let mut response = if path == "/api/session" {
        axum::Json(serde_json::json!({"ready":true})).into_response()
    } else {
        // Discard the outer wildcard's path captures before matching the inner API.
        let (mut parts, body) = request.into_parts();
        parts.extensions.clear();
        let request = Request::from_parts(parts, body);
        router
            .oneshot(request)
            .await
            .unwrap_or_else(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "请求失败。"))
    };
    if fresh {
        let value = format!(
            "{COOKIE}={}; HttpOnly; Secure; SameSite=Lax; Path=/; Max-Age={LIFETIME}",
            sign(&id, expiry, &host.signing_key)
        );
        response.headers_mut().append(
            header::SET_COOKIE,
            HeaderValue::from_str(&value).expect("signed cookie is ASCII"),
        );
    }
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("same-origin"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_browser_identity_cannot_be_forged_or_reused_after_expiry() {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let key = [3; 32];
        assert_eq!(verify(&sign(&id, now() + 60, &key), &key).unwrap().0, id);
        assert!(verify(&sign(&id, now() + 60, &key), &[4; 32]).is_none());
        assert!(verify(&sign(&id, now() - 1, &key), &key).is_none());
        assert!(verify("../../other.db.123.bad", &key).is_none());
    }

    async fn fixture() -> Router {
        let dir = std::env::temp_dir().join(format!("melody-public-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("index.html"), "<html>synthetic frontend</html>").unwrap();
        let prototype = AppState {
            previews: Default::default(),
            downloads: Default::default(),
            spotify: crate::SpotifyPlaylistWriter::new(),
            youtube: crate::YoutubePlaylistWriter::new(),
            apple: crate::AppleMusicConnector::new(),
            platforms: crate::platforms::PlatformService::new(),
            agent: crate::agent::AgentService::in_memory().await.unwrap(),
            metadata: crate::metadata::MetadataService::new(),
            analyses: Default::default(),
            imports: Default::default(),
            transfer_previews: Default::default(),
            transfer_runs: Default::default(),
        };
        router_with_key(
            prototype,
            Deployment {
                production: true,
                origin: "https://music.example.com".into(),
                bind: "0.0.0.0:10000".into(),
                data_dir: dir.clone(),
                static_dir: Some(dir),
                demo_expires_at: Some("2026-09-21".into()),
            },
            [5; 32],
        )
        .unwrap()
    }
    async fn send(
        router: &Router,
        method: &str,
        path: &str,
        cookie: &str,
        origin: &str,
        body: serde_json::Value,
    ) -> Response {
        router
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header(header::COOKIE, cookie)
                    .header(header::ORIGIN, origin)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(axum::body::Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap()
    }
    async fn json(response: Response) -> serde_json::Value {
        serde_json::from_slice(
            &axum::body::to_bytes(response.into_body(), 1_000_000)
                .await
                .unwrap(),
        )
        .unwrap()
    }
    #[tokio::test]
    async fn public_same_origin_cookies_static_routes_and_workspace_isolation() {
        let router = fixture().await;
        let config = send(
            &router,
            "GET",
            "/api/public-config",
            "",
            "https://music.example.com",
            serde_json::json!(null),
        )
        .await;
        assert_eq!(
            json(config).await,
            serde_json::json!({"public_demo_expires_at":"2026-09-21"})
        );
        for path in ["/health", "/transfer", "/compare"] {
            assert_eq!(
                send(
                    &router,
                    "GET",
                    path,
                    "",
                    "https://music.example.com",
                    serde_json::json!(null)
                )
                .await
                .status(),
                StatusCode::OK
            );
        }
        let mut cookies = Vec::new();
        for _ in 0..2 {
            let response = send(
                &router,
                "GET",
                "/api/session",
                "",
                "https://music.example.com",
                serde_json::json!(null),
            )
            .await;
            let cookie = response.headers()[header::SET_COOKIE].to_str().unwrap();
            for flag in ["HttpOnly", "Secure", "SameSite=Lax", "Path=/"] {
                assert!(cookie.contains(flag));
            }
            cookies.push(cookie.split(';').next().unwrap().to_string());
        }
        let preview = send(&router,"POST","/api/imports/preview",&cookies[0],"https://music.example.com",serde_json::json!({"name":"Synthetic","format":"txt","content":"Artist - Song","data_state":"REAL_TEXT","text_order":"artist_title"})).await;
        assert_eq!(preview.status(), StatusCode::OK);
        let id = json(preview).await["id"].as_str().unwrap().to_string();
        let other = send(
            &router,
            "POST",
            &format!("/api/imports/{id}/analyze"),
            &cookies[1],
            "https://music.example.com",
            serde_json::json!({}),
        )
        .await;
        assert_eq!(other.status(), StatusCode::NOT_FOUND);
        assert_eq!(
            send(
                &router,
                "POST",
                "/api/imports/preview",
                &cookies[0],
                "https://evil.example",
                serde_json::json!({})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            send(
                &router,
                "PUT",
                "/api/settings",
                &cookies[0],
                "https://music.example.com",
                serde_json::json!({})
            )
            .await
            .status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            send(
                &router,
                "GET",
                "/api/tasks",
                "",
                "https://music.example.com",
                serde_json::json!(null)
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            send(
                &router,
                "GET",
                "/api/no-such-endpoint",
                &cookies[0],
                "https://music.example.com",
                serde_json::json!(null)
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        let task_a = send(
            &router,
            "POST",
            "/api/tasks",
            &cookies[0],
            "https://music.example.com",
            serde_json::json!({"scenario":"personal_exploration","use_demo":true}),
        )
        .await;
        assert_eq!(task_a.status(), StatusCode::CREATED);
        let task_id = json(task_a).await["id"].as_str().unwrap().to_string();
        assert_eq!(
            send(
                &router,
                "GET",
                &format!("/api/tasks/{task_id}"),
                &cookies[1],
                "https://music.example.com",
                serde_json::json!(null)
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            json(
                send(
                    &router,
                    "GET",
                    "/api/tasks",
                    &cookies[1],
                    "https://music.example.com",
                    serde_json::json!(null)
                )
                .await
            )
            .await,
            serde_json::json!([])
        );
    }
}
