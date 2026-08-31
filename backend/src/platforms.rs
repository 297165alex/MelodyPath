use crate::models::{DataUseCapabilities, PlatformCapability, PlaylistLinkInspection};
use anyhow::{Context, Result};
use reqwest::{Client, Url, redirect::Policy};
use std::time::Duration;

#[derive(Clone)]
pub struct PlatformService {
    client: Client,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecognizedLink {
    platform: &'static str,
    label: &'static str,
    playlist_id: Option<String>,
    normalized_url: Option<String>,
}

pub fn spotify_data_use() -> DataUseCapabilities {
    DataUseCapabilities {
        can_read_account_identity: true,
        can_list_playlists: true,
        can_read_playlist_items: true,
        can_display_attributed_metadata: true,
        can_transfer_playlist_metadata: true,
        can_create_playlist: true,
        can_add_items: true,
        can_analyze_content: false,
        can_derive_metrics: false,
        can_cross_platform_compare: false,
        can_send_to_llm: false,
        can_train_model: false,
        explanation: "Spotify Developer Policy 允许经授权读取、归属展示、个人数据/歌单元数据传输和歌单管理，但禁止分析 Spotify Content、派生指标/画像以及 AI/ML 摄入。".into(),
    }
}

pub fn youtube_data_use() -> DataUseCapabilities {
    DataUseCapabilities {
        can_read_account_identity: true,
        can_list_playlists: true,
        can_read_playlist_items: true,
        can_display_attributed_metadata: true,
        can_transfer_playlist_metadata: true,
        can_create_playlist: true,
        can_add_items: true,
        can_analyze_content: false,
        can_derive_metrics: false,
        can_cross_platform_compare: false,
        can_send_to_llm: false,
        can_train_model: false,
        explanation: "YouTube API 数据可以在用户授权范围内读取、清楚标注来源并管理播放列表；YouTube 政策不允许提供 API 未给出的独立派生指标，因此不进入 MelodyPath 画像或跨平台指标。".into(),
    }
}

pub fn apple_data_use() -> DataUseCapabilities {
    DataUseCapabilities {
        can_read_account_identity: false,
        can_list_playlists: true,
        can_read_playlist_items: true,
        can_display_attributed_metadata: true,
        can_transfer_playlist_metadata: false,
        can_create_playlist: true,
        can_add_items: true,
        can_analyze_content: false,
        can_derive_metrics: false,
        can_cross_platform_compare: false,
        can_send_to_llm: false,
        can_train_model: false,
        explanation: "MusicKit 可在用户同意后读取和管理资料库；本项目尚未完成真实账号与条款审核，分析类能力保持关闭。".into(),
    }
}

impl PlatformService {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(7))
            .redirect(Policy::custom(|attempt| {
                if attempt.previous().len() >= 5 || !allowed_host(attempt.url()) {
                    attempt.stop()
                } else {
                    attempt.follow()
                }
            }))
            .user_agent("MelodyPath/0.2 (+public-playlist-link-check; no cookies)")
            .build()
            .expect("public link client configuration is valid");
        Self { client }
    }

    pub fn capabilities(
        &self,
        spotify_configured: bool,
        youtube_configured: bool,
        apple_configured: bool,
    ) -> Vec<PlatformCapability> {
        vec![
            PlatformCapability {
                platform: "netease".into(),
                display_name: "网易云音乐".into(),
                region: "中国平台".into(),
                capability_status: "qualification_required".into(),
                status_label: "需要平台接入资格".into(),
                account_connection: "requires_official_credentials".into(),
                public_playlist_links: "recognition_and_access_check".into(),
                playlist_read: "not_verified_for_public_web".into(),
                playlist_write: "requires_official_credentials".into(),
                search_links: true,
                requires_review: true,
                configured: false,
                official_docs_url: Some("https://developer.music.163.com/".into()),
                action_kind: "paste_link".into(),
                description: "可识别公开分享链接并检查页面是否可访问；没有已验证的通用网页歌单读取接口时不会抓取或冒充接通。".into(),
                policy_notice: Some("官方开放平台需要申请 appId/密钥；本版本不接收账号密码或 Cookie。".into()),
                data_use: DataUseCapabilities::unavailable("当前没有可验证的普通网页官方歌单数据权限。"),
            },
            PlatformCapability {
                platform: "qq_music".into(),
                display_name: "QQ音乐".into(),
                region: "中国平台".into(),
                capability_status: "qualification_required".into(),
                status_label: "需要平台接入资格".into(),
                account_connection: "restricted_official_sdk".into(),
                public_playlist_links: "recognition_and_access_check".into(),
                playlist_read: "not_verified_for_public_web".into(),
                playlist_write: "requires_platform_approval".into(),
                search_links: true,
                requires_review: true,
                configured: false,
                official_docs_url: Some("https://cloud.tencent.com/document/product/1081/67456".into()),
                action_kind: "paste_link".into(),
                description: "可识别公开分享链接并检查页面是否可访问；已核实的官方能力属于受限 SDK/合作场景。".into(),
                policy_notice: Some("未获得正式资格前不提供账号连接、私人歌单读取或写回。".into()),
                data_use: DataUseCapabilities::unavailable("已发现的官方能力限于腾讯连连 IoT/H5 合作场景。"),
            },
            PlatformCapability {
                platform: "kugou".into(),
                display_name: "酷狗音乐".into(),
                region: "中国平台".into(),
                capability_status: "qualification_required".into(),
                status_label: "需要平台接入资格".into(),
                account_connection: "official_sdk_only".into(),
                public_playlist_links: "not_implemented".into(),
                playlist_read: "requires_platform_approval".into(),
                playlist_write: "requires_platform_approval".into(),
                search_links: false,
                requires_review: true,
                configured: false,
                official_docs_url: Some("https://open.kugou.com/docs".into()),
                action_kind: "qualification".into(),
                description: "官方开放平台以 SDK 与商务接入为主，本项目仅保留正式连接器接口。".into(),
                policy_notice: None,
                data_use: DataUseCapabilities::unavailable("需要酷狗官方 SDK 或商务接入资格。"),
            },
            PlatformCapability {
                platform: "kuwo".into(),
                display_name: "酷我音乐".into(),
                region: "中国平台".into(),
                capability_status: "not_connected".into(),
                status_label: "尚未接通".into(),
                account_connection: "not_verified".into(),
                public_playlist_links: "not_implemented".into(),
                playlist_read: "not_verified".into(),
                playlist_write: "not_verified".into(),
                search_links: false,
                requires_review: true,
                configured: false,
                official_docs_url: None,
                action_kind: "planned".into(),
                description: "尚未找到可验证的通用个人网页授权与歌单接口；不会使用逆向接口或模拟登录。".into(),
                policy_notice: None,
                data_use: DataUseCapabilities::unavailable("尚未验证通用个人网页授权接口。"),
            },
            PlatformCapability {
                platform: "spotify".into(),
                display_name: "Spotify".into(),
                region: "国际平台".into(),
                capability_status: if spotify_configured { "official_oauth_ready" } else { "needs_configuration" }.into(),
                status_label: if spotify_configured { "官方账号连接已支持" } else { "需要配置开发者应用" }.into(),
                account_connection: "official_authorization_code".into(),
                public_playlist_links: "recognition_only".into(),
                playlist_read: if spotify_configured { "official_api_transfer_only" } else { "needs_configuration" }.into(),
                playlist_write: if spotify_configured { "official_api" } else { "needs_configuration" }.into(),
                search_links: true,
                requires_review: true,
                configured: spotify_configured,
                official_docs_url: Some("https://developer.spotify.com/documentation/web-api/concepts/authorization".into()),
                action_kind: if spotify_configured { "connect" } else { "configure" }.into(),
                description: "使用官方 Authorization Code Flow；读取数据只用于用户主动发起的歌单传输和写回。".into(),
                policy_notice: Some("Spotify 内容不会发送给 LLM，也不会用于画像、衍生指标或模型训练。".into()),
                data_use: spotify_data_use(),
            },
            PlatformCapability {
                platform: "apple_music".into(),
                display_name: "Apple Music".into(),
                region: "国际平台".into(),
                capability_status: if apple_configured { "implemented_unverified" } else { "needs_configuration" }.into(),
                status_label: if apple_configured { "已配置，尚待真实账号验收" } else { "完成配置即可使用" }.into(),
                account_connection: "musickit_planned".into(),
                public_playlist_links: "not_implemented".into(),
                playlist_read: "planned".into(),
                playlist_write: "planned".into(),
                search_links: true,
                requires_review: true,
                configured: apple_configured,
                official_docs_url: Some("https://developer.apple.com/musickit/".into()),
                action_kind: "configure".into(),
                description: "预留 MusicKit 正式连接器；当前只提供平台搜索链接与文件导出。".into(),
                policy_notice: None,
                data_use: apple_data_use(),
            },
            PlatformCapability {
                platform: "youtube_music".into(),
                display_name: "YouTube Music".into(),
                region: "国际平台".into(),
                capability_status: if youtube_configured { "official_oauth_ready" } else { "needs_configuration" }.into(),
                status_label: if youtube_configured { "官方账号连接已支持" } else { "完成配置即可使用" }.into(),
                account_connection: "official_google_oauth".into(),
                public_playlist_links: "recognition_only".into(),
                playlist_read: if youtube_configured { "official_api_transfer_only" } else { "needs_configuration" }.into(),
                playlist_write: if youtube_configured { "official_api" } else { "needs_configuration" }.into(),
                search_links: true,
                requires_review: true,
                configured: youtube_configured,
                official_docs_url: Some("https://developers.google.com/youtube/v3/guides/auth/server-side-web-apps".into()),
                action_kind: if youtube_configured { "connect" } else { "configure" }.into(),
                description: "使用 Google 官方 OAuth 与 YouTube Data API；支持用户拥有的播放列表读取、标题清理、传输和写回。".into(),
                policy_notice: Some("YouTube API 数据保留来源标注，不用于 API 未提供的派生画像或跨平台评分。".into()),
                data_use: youtube_data_use(),
            },
        ]
    }

    pub async fn inspect_link(&self, raw: &str) -> Result<PlaylistLinkInspection> {
        let recognized = recognize_link(raw)?;
        let Some(link) = recognized else {
            return Ok(PlaylistLinkInspection {
                platform: None,
                platform_label: None,
                recognized: false,
                playlist_id: None,
                normalized_url: None,
                resolved_url: None,
                publicly_accessible: None,
                access_status: "unrecognized".into(),
                structured_data_status: "not_checked".into(),
                playlist_name: None,
                track_count: None,
                preview_tracks: vec![],
                can_analyze: false,
                message: "未识别为当前支持检查的官方歌单分享链接。".into(),
                next_step: "请确认链接公开可访问，或展开“更多导入方式”粘贴歌曲清单。".into(),
            });
        };

        let mut link = link;
        let parsed = Url::parse(raw.trim()).context("公开歌单链接格式无效")?;
        let response = self
            .client
            .get(parsed)
            .header("Range", "bytes=0-65535")
            .send()
            .await;
        let mut resolved_url = None;
        let mut structured_data_status = "not_available".to_string();
        let (publicly_accessible, access_status, message) = match response {
            Ok(mut response) if response.status().is_success() => {
                let final_url = response.url().clone();
                resolved_url = Some(final_url.to_string());
                if let Ok(Some(resolved_link)) = recognize_link(final_url.as_str())
                    && resolved_link.platform == link.platform
                    && resolved_link.playlist_id.is_some()
                {
                    link = resolved_link;
                }
                let prefix = read_body_prefix(&mut response, 256 * 1024).await;
                structured_data_status = inspect_public_structure(&prefix);
                let structure_note = match structured_data_status.as_str() {
                    "schema_org_playlist_found_policy_unverified" => {
                        "页面含 schema.org 歌单结构，但尚未验证平台条款允许自动导入，因此没有提取曲目。"
                    }
                    "page_state_found_not_stable_api" => {
                        "页面含站点内部状态数据，但它不是稳定的官方开放 API，因此没有依赖或解析。"
                    }
                    _ => "页面未发现可作为稳定官方歌单 API 的公开结构化曲目数据。",
                };
                (
                    Some(true),
                    "page_reachable".to_string(),
                    format!(
                        "已识别 {} 链接且官方公开页面可访问。{structure_note}",
                        link.label
                    ),
                )
            }
            Ok(response) => (
                Some(false),
                "page_not_accessible".to_string(),
                format!(
                    "官方分享页面返回 HTTP {}，可能已设为私密、失效或受地区限制。",
                    response.status()
                ),
            ),
            Err(error) => (
                None,
                "check_failed".to_string(),
                format!("无法可靠检查公开页面：{error}。未使用 Cookie、账号密码或逆向接口。"),
            ),
        };

        Ok(PlaylistLinkInspection {
            platform: Some(link.platform.into()),
            platform_label: Some(link.label.into()),
            recognized: true,
            playlist_id: link.playlist_id,
            normalized_url: link.normalized_url,
            resolved_url,
            publicly_accessible,
            access_status,
            structured_data_status,
            playlist_name: None,
            track_count: None,
            preview_tracks: vec![],
            can_analyze: false,
            message,
            next_step: if link.platform == "spotify" {
                "请使用上方 Spotify 官方授权选择歌单；Spotify 数据只可用于合规传输与写回。".into()
            } else {
                "如需立即分析，请在“更多导入方式”中直接粘贴歌曲清单；这是备用方式，不需要制作 JSON/CSV。".into()
            },
        })
    }
}

async fn read_body_prefix(response: &mut reqwest::Response, limit: usize) -> Vec<u8> {
    let mut output = Vec::new();
    while output.len() < limit {
        match response.chunk().await {
            Ok(Some(chunk)) => {
                let remaining = limit - output.len();
                output.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
            }
            _ => break,
        }
    }
    output
}

fn inspect_public_structure(body: &[u8]) -> String {
    let text = String::from_utf8_lossy(body).to_ascii_lowercase();
    if text.contains("application/ld+json")
        && (text.contains("musicplaylist") || text.contains("\"track\""))
    {
        "schema_org_playlist_found_policy_unverified".into()
    } else if text.contains("__next_data__")
        || text.contains("__initial_state__")
        || text.contains("window.__data__")
    {
        "page_state_found_not_stable_api".into()
    } else {
        "no_public_playlist_structure".into()
    }
}

fn host_matches(host: &str, domain: &str) -> bool {
    host == domain || host.ends_with(&format!(".{domain}"))
}

fn allowed_host(url: &Url) -> bool {
    let Some(host) = url.host_str().map(str::to_ascii_lowercase) else {
        return false;
    };
    [
        "music.163.com",
        "163cn.tv",
        "y.qq.com",
        "spotify.com",
        "open.spotify.com",
        "youtube.com",
        "youtu.be",
        "music.youtube.com",
    ]
    .iter()
    .any(|domain| host_matches(&host, domain))
}

fn query_value(url: &Url, names: &[&str]) -> Option<String> {
    url.query_pairs().find_map(|(key, value)| {
        names
            .iter()
            .any(|name| key.eq_ignore_ascii_case(name))
            .then(|| value.into_owned())
    })
}

fn numeric_path_id(url: &Url) -> Option<String> {
    url.path_segments()?
        .rev()
        .find(|part| part.len() >= 3 && part.chars().all(|character| character.is_ascii_digit()))
        .map(str::to_string)
}

fn recognize_link(raw: &str) -> Result<Option<RecognizedLink>> {
    let url = Url::parse(raw.trim()).context("请输入完整的 http/https 公开歌单链接")?;
    if !matches!(url.scheme(), "http" | "https") || !allowed_host(&url) {
        return Ok(None);
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if host_matches(&host, "music.163.com") || host_matches(&host, "163cn.tv") {
        let playlist_id =
            query_value(&url, &["id", "playlistId"]).or_else(|| numeric_path_id(&url));
        return Ok(Some(RecognizedLink {
            platform: "netease",
            label: "网易云音乐",
            normalized_url: playlist_id
                .as_ref()
                .map(|id| format!("https://music.163.com/playlist?id={id}")),
            playlist_id,
        }));
    }
    if host_matches(&host, "y.qq.com") {
        let playlist_id =
            query_value(&url, &["id", "dissid", "playlistId"]).or_else(|| numeric_path_id(&url));
        return Ok(Some(RecognizedLink {
            platform: "qq_music",
            label: "QQ音乐",
            normalized_url: playlist_id
                .as_ref()
                .map(|id| format!("https://y.qq.com/n/ryqq/playlist/{id}")),
            playlist_id,
        }));
    }
    if host_matches(&host, "spotify.com") {
        let playlist_id = url
            .path_segments()
            .and_then(|mut parts| {
                parts
                    .find(|part| *part == "playlist")
                    .and_then(|_| parts.next())
            })
            .map(str::to_string);
        return Ok(Some(RecognizedLink {
            platform: "spotify",
            label: "Spotify",
            normalized_url: playlist_id
                .as_ref()
                .map(|id| format!("https://open.spotify.com/playlist/{id}")),
            playlist_id,
        }));
    }
    if host_matches(&host, "youtube.com") || host_matches(&host, "youtu.be") {
        let playlist_id = query_value(&url, &["list"]);
        return Ok(Some(RecognizedLink {
            platform: "youtube_music",
            label: "YouTube Music",
            normalized_url: playlist_id
                .as_ref()
                .map(|id| format!("https://www.youtube.com/playlist?list={id}")),
            playlist_id,
        }));
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_netease_and_qq_without_network_access() {
        let netease = recognize_link("https://music.163.com/playlist?id=123456")
            .unwrap()
            .unwrap();
        assert_eq!(netease.platform, "netease");
        assert_eq!(netease.playlist_id.as_deref(), Some("123456"));
        let qq = recognize_link("https://y.qq.com/n/ryqq/playlist/987654")
            .unwrap()
            .unwrap();
        assert_eq!(qq.platform, "qq_music");
        assert_eq!(qq.playlist_id.as_deref(), Some("987654"));
    }

    #[test]
    fn rejects_arbitrary_hosts_to_prevent_ssrf() {
        assert!(
            recognize_link("http://127.0.0.1:3000/private")
                .unwrap()
                .is_none()
        );
        assert!(
            recognize_link("https://spotify.com.example.org/playlist/abc")
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn spotify_and_youtube_ids_are_normalized() {
        let spotify = recognize_link("https://open.spotify.com/playlist/abc123?si=x")
            .unwrap()
            .unwrap();
        assert_eq!(spotify.playlist_id.as_deref(), Some("abc123"));
        let youtube = recognize_link("https://music.youtube.com/playlist?list=PLabc")
            .unwrap()
            .unwrap();
        assert_eq!(youtube.playlist_id.as_deref(), Some("PLabc"));
    }
}
