use crate::models::{
    DataUseCapabilities, PlatformCapability, PlaylistLinkInspection, PublicLinkCapability,
};
use anyhow::{Context, Result};
use reqwest::{Client, Url, redirect::Policy};
use std::time::Duration;

mod china;
mod netease;
mod netease_tracks;

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
    let mut data = DataUseCapabilities::unavailable(
        "公开目录数据仅用于用户主动预览和传输；私人资料库与写入未接入，不进入画像或 LLM。",
    );
    data.can_read_playlist_items = true;
    data.can_display_attributed_metadata = true;
    data.can_transfer_playlist_metadata = true;
    data
}

impl PlatformService {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(7))
            .redirect(Policy::custom(|attempt| {
                if !public_redirect_allowed(attempt.url(), attempt.previous()) {
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
        let mut capabilities = vec![
            PlatformCapability {
                platform: "netease".into(),
                auth_supported: false,
                playlist_read_supported: false,
                playlist_write_supported: false,
                public_link_import_supported: true,
                file_import_supported: true,
                compare_supported: true,
                transfer_source_supported: true,
                transfer_destination_supported: false,
                copy_source_supported: true,
                copy_destination_supported: false,
                playlist_read_for_copy: false,
                playlist_read_for_compare: false,
                playlist_read_for_recommendation: false,
                alternate_version_search_supported: false,
                status: "IMPORT_ONLY".into(),
                reason: "匿名公开 HTML 条件导入；不接入账号 OAuth，失败可使用文件/文本。".into(),
                display_name: "网易云音乐".into(),
                region: "中国平台".into(),
                capability_status: "qualification_required".into(),
                status_label: "需要平台接入资格".into(),
                account_connection: "requires_official_credentials".into(),
                public_playlist_links: "ACCESSIBILITY_CHECK_ONLY".into(),
                playlist_read: "not_verified_for_public_web".into(),
                playlist_write: "requires_official_credentials".into(),
                search_links: true,
                requires_review: true,
                configured: false,
                official_docs_url: Some("https://developer.music.163.com/".into()),
                action_kind: "paste_link".into(),
                description: "公开页面列出可验证歌曲时尝试导入；最多导入 20 首并明确展示实际导入数与页面声明总数。没有可用公开歌曲时保留可访问性检查。".into(),
                policy_notice: Some("只读取公开 HTML 和 JSON-LD，不使用账号密码、Cookie、私有 API 或签名接口。".into()),
                data_use: DataUseCapabilities::unavailable("当前没有可验证的普通网页官方歌单数据权限。"),
            },
            PlatformCapability {
                platform: "qq_music".into(),
                auth_supported: false,
                playlist_read_supported: false,
                playlist_write_supported: false,
                public_link_import_supported: true,
                file_import_supported: true,
                compare_supported: true,
                transfer_source_supported: true,
                transfer_destination_supported: false,
                copy_source_supported: true,
                copy_destination_supported: false,
                playlist_read_for_copy: false,
                playlist_read_for_compare: false,
                playlist_read_for_recommendation: false,
                alternate_version_search_supported: false,
                status: "IMPORT_ONLY".into(),
                reason: "公开页面如提供完整可验证 JSON-LD 曲目则有限导入，否则仅返回可访问性检查。".into(),
                display_name: "QQ音乐".into(),
                region: "中国平台".into(),
                capability_status: "qualification_required".into(),
                status_label: "需要平台接入资格".into(),
                account_connection: "restricted_official_sdk".into(),
                public_playlist_links: "ACCESSIBILITY_CHECK_ONLY".into(),
                playlist_read: "not_verified_for_public_web".into(),
                playlist_write: "requires_platform_approval".into(),
                search_links: true,
                requires_review: true,
                configured: false,
                official_docs_url: Some("https://cloud.tencent.com/document/product/1081/67456".into()),
                action_kind: "paste_link".into(),
                description: "可识别公开分享链接并检查页面；只读取公开 HTML/JSON-LD 中明确给出的曲目，页面壳不会生成歌曲。".into(),
                policy_notice: Some("未获得正式资格前不提供账号连接、私人歌单读取或写回。".into()),
                data_use: DataUseCapabilities::unavailable("已发现的官方能力限于腾讯连连 IoT/H5 合作场景。"),
            },
            PlatformCapability {
                platform: "kugou".into(),
                auth_supported: false,
                playlist_read_supported: false,
                playlist_write_supported: false,
                public_link_import_supported: true,
                file_import_supported: true,
                compare_supported: true,
                transfer_source_supported: true,
                transfer_destination_supported: false,
                copy_source_supported: true,
                copy_destination_supported: false,
                playlist_read_for_copy: false,
                playlist_read_for_compare: false,
                playlist_read_for_recommendation: false,
                alternate_version_search_supported: false,
                status: "PARTNERSHIP_REQUIRED".into(),
                reason: "开放平台不等于个人歌单 OAuth；未验证 writer 前只提供导入。".into(),
                display_name: "酷狗音乐".into(),
                region: "中国平台".into(),
                capability_status: "qualification_required".into(),
                status_label: "需要平台接入资格".into(),
                account_connection: "official_sdk_only".into(),
                public_playlist_links: "ACCESSIBILITY_CHECK_ONLY".into(),
                playlist_read: "requires_platform_approval".into(),
                playlist_write: "requires_platform_approval".into(),
                search_links: true,
                requires_review: true,
                configured: false,
                official_docs_url: Some("https://open.kugou.com/docs".into()),
                action_kind: "paste_link".into(),
                description: "识别官方公开歌单页；仅当 HTML/JSON-LD 明确提供标题与艺人时导入，否则保持 ACCESSIBILITY_CHECK_ONLY。".into(),
                policy_notice: Some("不解码站内状态，不调用私有接口，不使用 Cookie。".into()),
                data_use: DataUseCapabilities::unavailable("需要酷狗官方 SDK 或商务接入资格。"),
            },
            PlatformCapability {
                platform: "kuwo".into(),
                auth_supported: false,
                playlist_read_supported: false,
                playlist_write_supported: false,
                public_link_import_supported: false,
                file_import_supported: true,
                compare_supported: true,
                transfer_source_supported: true,
                transfer_destination_supported: false,
                copy_source_supported: true,
                copy_destination_supported: false,
                playlist_read_for_copy: false,
                playlist_read_for_compare: false,
                playlist_read_for_recommendation: false,
                alternate_version_search_supported: false,
                status: "IMPORT_ONLY".into(),
                reason: "没有已验证的官方个人歌单接口；只提供文件或文本导入。".into(),
                display_name: "酷我音乐".into(),
                region: "中国平台".into(),
                capability_status: "not_connected".into(),
                status_label: "尚未接通".into(),
                account_connection: "not_verified".into(),
                public_playlist_links: "UNSUPPORTED".into(),
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
                auth_supported: true,
                playlist_read_supported: true,
                playlist_write_supported: true,
                public_link_import_supported: false,
                file_import_supported: true,
                compare_supported: false,
                transfer_source_supported: true,
                transfer_destination_supported: true,
                copy_source_supported: true,
                copy_destination_supported: true,
                playlist_read_for_copy: true,
                playlist_read_for_compare: false,
                playlist_read_for_recommendation: false,
                alternate_version_search_supported: false,
                status: if spotify_configured {
                    "MANUAL_AUTH_REQUIRED"
                } else {
                    "IMPLEMENTED_BUT_UNCONFIGURED"
                }
                .into(),
                reason: "官方 OAuth 能力已实现；连接与真实读写仍取决于开发者配置和用户授权。".into(),
                display_name: "Spotify".into(),
                region: "国际平台".into(),
                capability_status: if spotify_configured { "official_oauth_ready" } else { "needs_configuration" }.into(),
                status_label: if spotify_configured { "官方账号连接已支持" } else { "需要配置开发者应用" }.into(),
                account_connection: "official_authorization_code".into(),
                public_playlist_links: "AUTH_REQUIRED".into(),
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
                auth_supported: false,
                playlist_read_supported: false,
                playlist_write_supported: false,
                public_link_import_supported: true,
                file_import_supported: true,
                compare_supported: true,
                transfer_source_supported: true,
                transfer_destination_supported: false,
                copy_source_supported: true,
                copy_destination_supported: false,
                playlist_read_for_copy: false,
                playlist_read_for_compare: false,
                playlist_read_for_recommendation: false,
                alternate_version_search_supported: false,
                status: if apple_configured { "OFFICIAL_API_NOT_REAL_VERIFIED" } else { "CONFIG_REQUIRED" }.into(),
                reason: "公开目录 API 已实现，待真人验收；私人资料库账号连接未实现。".into(),
                display_name: "Apple Music".into(),
                region: "国际平台".into(),
                capability_status: if apple_configured { "catalog_ready" } else { "needs_configuration" }.into(),
                status_label: if apple_configured { "OFFICIAL API · NOT REAL VERIFIED" } else { "OFFICIAL API · CONFIG REQUIRED" }.into(),
                account_connection: "not_implemented".into(),
                public_playlist_links: if apple_configured { "TRACK_IMPORT_AVAILABLE" } else { "CONFIG_REQUIRED" }.into(),
                playlist_read: "public_catalog_only".into(),
                playlist_write: "not_implemented".into(),
                search_links: true,
                requires_review: true,
                configured: apple_configured,
                official_docs_url: Some("https://developer.apple.com/musickit/".into()),
                action_kind: "import".into(),
                description: if apple_configured {
                    "公开目录元数据与曲目分页已实现；请粘贴公开歌单 URL。私人资料库未接入。"
                } else {
                    "WAITING_FOR_APPLE_DEVELOPER_CREDENTIALS；部署者配置后可读取公开目录，普通用户可先用文件或文本。"
                }
                .into(),
                policy_notice: None,
                data_use: apple_data_use(),
            },
            PlatformCapability {
                platform: "youtube_music".into(),
                auth_supported: true,
                playlist_read_supported: true,
                playlist_write_supported: true,
                public_link_import_supported: false,
                file_import_supported: true,
                compare_supported: false,
                transfer_source_supported: true,
                transfer_destination_supported: true,
                copy_source_supported: true,
                copy_destination_supported: true,
                playlist_read_for_copy: true,
                playlist_read_for_compare: false,
                playlist_read_for_recommendation: false,
                alternate_version_search_supported: true,
                status: if youtube_configured {
                    "MANUAL_AUTH_REQUIRED"
                } else {
                    "IMPLEMENTED_BUT_UNCONFIGURED"
                }
                .into(),
                reason: "Google OAuth 与 YouTube Data API 能力已实现；真实使用取决于配置、授权与额度。".into(),
                display_name: "YouTube Music".into(),
                region: "国际平台".into(),
                capability_status: if youtube_configured { "official_oauth_ready" } else { "needs_configuration" }.into(),
                status_label: if youtube_configured { "官方账号连接已支持" } else { "完成配置即可使用" }.into(),
                account_connection: "official_google_oauth".into(),
                public_playlist_links: "AUTH_REQUIRED".into(),
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
        ];
        let mut qishui = capabilities
            .iter()
            .find(|item| item.platform == "kuwo")
            .unwrap()
            .clone();
        qishui.platform = "qishui".into();
        qishui.display_name = "汽水音乐".into();
        qishui.public_playlist_links = "URL_RECOGNITION_ONLY".into();
        qishui.status_label = "仅文件或文本导入".into();
        qishui.action_kind = "import".into();
        qishui.reason = "当前只识别已知官方域名，未验证歌单 ID 解析或官方曲目读取接口。".into();
        qishui.description = qishui.reason.clone();
        qishui.official_docs_url = Some("https://qishui.douyin.com/".into());
        capabilities.push(qishui);
        for item in &mut capabilities {
            if matches!(
                item.platform.as_str(),
                "netease" | "qq_music" | "kugou" | "qishui" | "kuwo"
            ) {
                item.status = "FILE_IMPORT_AVAILABLE".into();
                item.status_label = format!("FILE / TEXT IMPORT · {}", item.public_playlist_links);
                item.reason = "未核实可用于本项目的通用官方个人歌单 API；无账号连接。没有确认统一官方导出格式，可自行整理文件或文本。".into();
            }
            if matches!(item.platform.as_str(), "spotify" | "youtube_music") {
                item.public_link_import_supported = true;
            }
        }
        capabilities
    }

    pub async fn inspect_link(&self, raw: &str) -> Result<PlaylistLinkInspection> {
        let recognized = recognize_link(raw)?;
        let Some(link) = recognized else {
            return Ok(PlaylistLinkInspection {
                import_preview: None,
                import_rows: vec![],
                capability: PublicLinkCapability::Unsupported,
                url_valid: false,
                playlist_id_valid: false,
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
                declared_count: None,
                visible_count: 0,
                imported_count: 0,
                skipped_count: 0,
                unexposed_count: 0,
                partial_import: false,
                track_count: None,
                preview_tracks: vec![],
                can_analyze: false,
                message: "未识别为当前支持检查的官方歌单分享链接。".into(),
                next_step: "请确认链接公开可访问，或展开“更多导入方式”粘贴歌曲清单。".into(),
            });
        };

        let initial = recognition_result(&link);
        // Account APIs are a separate, explicitly authorized Copy flow. Never scrape
        // their pages or route platform data into the local analysis importer.
        let parsed = Url::parse(raw.trim()).context("公开歌单链接格式无效")?;
        let official_short_link = parsed
            .host_str()
            .is_some_and(|host| host_matches(host, "163cn.tv"))
            && parsed.path().len() > 1;
        if !matches!(link.platform, "netease" | "qq_music" | "kugou")
            || (link.playlist_id.is_none() && !official_short_link)
        {
            return Ok(initial);
        }
        let mut link = link;
        let mut request = self
            .client
            .get(link.normalized_url.as_deref().unwrap_or(parsed.as_str()));
        if link.platform != "netease" {
            request = request.header("Range", "bytes=0-65535");
        }
        let response = request.send().await;
        let mut resolved_url = None;
        let mut structured_data_status = "not_available".to_string();
        let mut public_metadata = None;
        let mut imported_page = None;
        let mut china_page = None;
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
                if link.platform == "netease" {
                    let html = response
                        .headers()
                        .get(reqwest::header::CONTENT_TYPE)
                        .and_then(|v| v.to_str().ok())
                        .is_some_and(|v| {
                            v.split(';')
                                .next()
                                .is_some_and(|v| v.trim().eq_ignore_ascii_case("text/html"))
                        });
                    let metadata = if response.status() != reqwest::StatusCode::OK || !html {
                        netease::PublicMetadata {
                            status: "public_page_response_rejected",
                            ..Default::default()
                        }
                    } else {
                        match read_public_page(&mut response, 1024 * 1024).await {
                            Some(body) => {
                                let mut metadata = netease::extract(&body);
                                if let Some(id) = link.playlist_id.as_deref() {
                                    match netease_tracks::import(
                                        &body,
                                        id,
                                        metadata.name.as_deref(),
                                    )
                                    .await
                                    {
                                        Ok(imported) => imported_page = Some(imported),
                                        Err(status) => metadata.status = status,
                                    }
                                }
                                metadata
                            }
                            None => netease::PublicMetadata {
                                status: "public_page_incomplete_or_too_large",
                                ..Default::default()
                            },
                        }
                    };
                    structured_data_status = metadata.status.into();
                    let message = metadata.explanation();
                    public_metadata = Some(metadata);
                    (Some(true), "page_reachable".to_string(), message)
                } else {
                    let prefix = read_body_prefix(&mut response, 256 * 1024).await;
                    structured_data_status = inspect_public_structure(&prefix);
                    if let (Some(adapter), Some(playlist_id)) = (
                        china::PublicJsonLdAdapter::for_platform(link.platform),
                        link.playlist_id.as_deref(),
                    ) {
                        use china::ChinaPlatformAdapter;
                        let page = adapter.parse_public_metadata(
                            &prefix,
                            playlist_id,
                            resolved_url
                                .as_deref()
                                .or(link.normalized_url.as_deref())
                                .unwrap_or(raw),
                        );
                        structured_data_status = page.status.into();
                        china_page = Some(page);
                    }
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
            }
            Ok(response) => (
                Some(false),
                "page_not_accessible".to_string(),
                format!(
                    "官方分享页面返回 HTTP {}，可能已设为私密、失效或受地区限制。",
                    response.status()
                ),
            ),
            Err(_) => (
                None,
                "check_failed".to_string(),
                "公开页面检查失败，请检查网络或使用本地文件导入；当前不能读取完整曲目。".into(),
            ),
        };

        if let Some(page) = china_page {
            let tracks = china::into_tracks(&page.raw_tracks);
            let imported_count = tracks.len();
            let skipped_count = page
                .rows
                .iter()
                .filter(|row| row.import_status != "IMPORTED")
                .count();
            let declared_count = page.declared_count;
            let unexposed_count = declared_count
                .unwrap_or(page.visible_count)
                .saturating_sub(page.visible_count);
            let partial_import = imported_count < page.visible_count || unexposed_count > 0;
            let can_analyze = imported_count > 0;
            let total = declared_count.unwrap_or(page.visible_count);
            return Ok(PlaylistLinkInspection {
                import_preview: None,
                import_rows: page.rows,
                capability: if can_analyze {
                    PublicLinkCapability::TrackImportAvailable
                } else {
                    PublicLinkCapability::AccessibilityCheckOnly
                },
                url_valid: true,
                playlist_id_valid: link.playlist_id.is_some(),
                platform: Some(link.platform.into()),
                platform_label: Some(link.label.into()),
                recognized: true,
                playlist_id: link.playlist_id,
                normalized_url: link.normalized_url,
                resolved_url,
                publicly_accessible,
                access_status,
                structured_data_status,
                playlist_name: page.name,
                declared_count,
                visible_count: page.visible_count,
                imported_count,
                skipped_count,
                unexposed_count,
                partial_import,
                track_count: declared_count,
                preview_tracks: tracks,
                can_analyze,
                message: if can_analyze {
                    format!("Imported {imported_count}/{total} tracks")
                } else {
                    "Playlist recognized but tracks unavailable.".into()
                },
                next_step: if can_analyze {
                    "核对 Import Preview，确认后进入既有 MetadataResolver 与推荐流程。".into()
                } else {
                    "ACCESSIBILITY_CHECK_ONLY：请上传 TXT/CSV/JSON/M3U 或粘贴歌曲清单。".into()
                },
            });
        }

        let (tracks, rows, visible_count) = imported_page
            .map(|p| (p.tracks, p.rows, p.visible_count))
            .unwrap_or_default();
        let can_analyze = !tracks.is_empty();
        let declared_count = public_metadata
            .as_ref()
            .and_then(|metadata| metadata.declared_tracks);
        let imported_count = tracks.len();
        let skipped_count = rows
            .iter()
            .filter(|row| row.import_status == "SKIPPED_DETAIL_UNAVAILABLE")
            .count();
        let (unexposed_count, partial_import) =
            netease_import_state(declared_count, visible_count, imported_count);
        let message = if !rows.is_empty() && !tracks.is_empty() {
            let total = declared_count.unwrap_or(visible_count);
            if unexposed_count > 0 {
                format!(
                    "网易云歌单解析成功\n公开页面仅提供部分歌曲，\n已导入 {} / {} 首歌曲",
                    imported_count, total
                )
            } else {
                let reason = if visible_count > netease_tracks::MAX_DETAILS {
                    "原因：当前公开页面解析限制，仅导入前20首歌曲用于分析。"
                } else if skipped_count > 0 {
                    "原因：部分公开歌曲缺少可验证元数据。"
                } else {
                    ""
                };
                format!(
                    "网易云歌单解析成功\n已导入：{} / {} 首歌曲\n{}",
                    imported_count, total, reason
                )
            }
        } else if link.platform == "netease" {
            "检测到网易云歌单，\n但当前无法获取公开歌曲列表。\n请使用 TXT/CSV 导入。".into()
        } else {
            message
        };
        Ok(PlaylistLinkInspection {
            import_preview: None,
            import_rows: rows,
            capability: if can_analyze {
                PublicLinkCapability::TrackImportAvailable
            } else {
                PublicLinkCapability::AccessibilityCheckOnly
            },
            url_valid: true,
            playlist_id_valid: link.playlist_id.is_some(),
            platform: Some(link.platform.into()),
            platform_label: Some(link.label.into()),
            recognized: true,
            playlist_id: link.playlist_id,
            normalized_url: link.normalized_url,
            resolved_url,
            publicly_accessible,
            access_status,
            structured_data_status: if can_analyze {
                "public_html_tracks_imported".into()
            } else {
                structured_data_status
            },
            playlist_name: public_metadata.as_ref().and_then(|m| m.name.clone()),
            declared_count,
            visible_count,
            imported_count,
            skipped_count,
            unexposed_count,
            partial_import,
            track_count: declared_count,
            preview_tracks: tracks,
            can_analyze,
            message,
            next_step: if can_analyze {
                "核对 Import Preview 与未导入数量，确认后使用既有 MetadataResolver 和推荐流程。"
                    .into()
            } else if link.platform == "spotify" {
                "请使用上方 Spotify 官方授权选择歌单；Spotify 数据只可用于合规传输与写回。".into()
            } else {
                "如需立即分析，请在“更多导入方式”中直接粘贴歌曲清单；这是备用方式，不需要制作 JSON/CSV。".into()
            },
        })
    }
}

fn netease_import_state(
    declared_count: Option<usize>,
    visible_count: usize,
    imported_count: usize,
) -> (usize, bool) {
    let unexposed_count = declared_count
        .map(|declared| declared.saturating_sub(visible_count))
        .unwrap_or(0);
    let partial_import = imported_count > 0
        && declared_count
            .map(|declared| imported_count < declared)
            .unwrap_or(imported_count < visible_count);
    (unexposed_count, partial_import)
}

// A bounded complete body is required before parsing NetEase metadata. A partial
// response or read error must not be mistaken for a complete public playlist.
async fn read_public_page(response: &mut reqwest::Response, limit: usize) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if chunk.len() > limit.saturating_sub(body.len()) {
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    Some(body)
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
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
        || !matches!(url.port(), None | Some(80) | Some(443))
    {
        return false;
    }
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
        "music.apple.com",
        "kugou.com",
        "qishui.douyin.com",
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

// These checks are for the anonymous accessibility client, never an OAuth client.
fn public_redirect_allowed(target: &Url, previous: &[Url]) -> bool {
    if previous.is_empty() || previous.len() >= 5 || target.scheme() != "https" {
        return false;
    }
    let Ok(Some(source)) = recognize_link(previous[0].as_str()) else {
        return false;
    };
    let Ok(Some(destination)) = recognize_link(target.as_str()) else {
        return false;
    };
    source.platform == destination.platform
        && matches!(source.platform, "netease" | "qq_music" | "kugou")
        && (destination.playlist_id.is_some()
            || (target.host_str() == Some("163cn.tv") && target.path().len() > 1))
}

fn recognize_link(raw: &str) -> Result<Option<RecognizedLink>> {
    let url = Url::parse(raw.trim()).context("请输入完整的 http/https 公开歌单链接")?;
    if !matches!(url.scheme(), "http" | "https") || !allowed_host(&url) {
        return Ok(None);
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if (host_matches(&host, "music.163.com")
        || host_matches(&host, "163cn.tv")
        || host_matches(&host, "y.qq.com"))
        && (url.scheme() != "https"
            || url.port_or_known_default() != Some(443)
            || !matches!(
                host.as_str(),
                "music.163.com" | "y.music.163.com" | "163cn.tv" | "y.qq.com"
            ))
    {
        return Ok(None);
    }
    // NetEase's /#/playlist?id=... is a client-side route, not an HTTP query.
    let effective = url
        .fragment()
        .filter(|f| f.starts_with("/playlist?"))
        .and_then(|f| Url::parse(&format!("https://{host}{f}")).ok());
    let route = effective.as_ref().unwrap_or(&url);
    if host_matches(&host, "music.163.com") || host_matches(&host, "163cn.tv") {
        let playlist_path = route.path().trim_end_matches('/');
        let supported_playlist_route = match host.as_str() {
            "music.163.com" => matches!(playlist_path, "/playlist" | "/m/playlist"),
            "y.music.163.com" => playlist_path == "/m/playlist",
            _ => false,
        };
        let playlist_id = query_value(route, &["id", "playlistId"])
            .filter(|id| supported_playlist_route && numeric_id(id));
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
        let path = url.path().trim_end_matches('/');
        let playlist_id = if path == "/n/m/detail/taoge/index.html" {
            query_value(&url, &["id", "dissid", "playlistId"])
        } else {
            path.strip_prefix("/n/ryqq/playlist/")
                .or_else(|| path.strip_prefix("/n/ryqq_v2/playlist/"))
                .map(str::to_string)
        }
        .filter(|id| numeric_id(id));
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
            .filter(|id| id.len() == 22 && id.bytes().all(|b| b.is_ascii_alphanumeric()))
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
        let playlist_id = query_value(&url, &["list"]).filter(|id| {
            (url.path() == "/playlist" || url.path() == "/watch" || host == "youtu.be")
                && (10..=100).contains(&id.len())
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        });
        return Ok(Some(RecognizedLink {
            platform: "youtube_music",
            label: "YouTube Music",
            normalized_url: playlist_id
                .as_ref()
                .map(|id| format!("https://www.youtube.com/playlist?list={id}")),
            playlist_id,
        }));
    }
    if host == "music.apple.com" {
        let playlist_id = crate::writers::apple::catalog_link(raw).map(|(_, id)| id);
        return Ok(Some(RecognizedLink {
            platform: "apple_music",
            label: "Apple Music",
            playlist_id,
            normalized_url: None,
        }));
    }
    if host_matches(&host, "kugou.com") || host == "qishui.douyin.com" {
        let (platform, label) = if host_matches(&host, "kugou.com") {
            ("kugou", "酷狗音乐")
        } else {
            ("qishui", "汽水音乐")
        };
        let playlist_id =
            if platform == "kugou" && matches!(host.as_str(), "www.kugou.com" | "m.kugou.com") {
                let path = url.path().trim_end_matches('/');
                path.strip_prefix("/songlist/")
                    .or_else(|| path.strip_prefix("/playlist/"))
                    .filter(|id| {
                        (3..=80).contains(&id.len())
                            && id.bytes().all(|byte| {
                                byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-')
                            })
                    })
                    .map(str::to_string)
            } else {
                None
            };
        return Ok(Some(RecognizedLink {
            platform,
            label,
            normalized_url: playlist_id
                .as_ref()
                .map(|id| format!("https://www.kugou.com/songlist/{id}/")),
            playlist_id,
        }));
    }
    Ok(None)
}

fn numeric_id(id: &str) -> bool {
    !id.is_empty() && id.len() <= 20 && id.bytes().all(|b| b.is_ascii_digit())
}

fn recognition_result(link: &RecognizedLink) -> PlaylistLinkInspection {
    let auth = matches!(link.platform, "spotify" | "youtube_music") && link.playlist_id.is_some();
    PlaylistLinkInspection {
        import_preview: None,
        import_rows: vec![],
        capability: if auth {
            PublicLinkCapability::AuthRequired
        } else {
            PublicLinkCapability::UrlRecognitionOnly
        },
        url_valid: true,
        playlist_id_valid: link.playlist_id.is_some(),
        platform: Some(link.platform.into()),
        platform_label: Some(link.label.into()),
        recognized: true,
        playlist_id: link.playlist_id.clone(),
        normalized_url: link.normalized_url.clone(),
        resolved_url: None,
        publicly_accessible: None,
        access_status: "not_checked".into(),
        structured_data_status: "not_checked".into(),
        playlist_name: None,
        declared_count: None,
        visible_count: 0,
        imported_count: 0,
        skipped_count: 0,
        unexposed_count: 0,
        partial_import: false,
        track_count: None,
        preview_tracks: vec![],
        can_analyze: false,
        message: if auth {
            format!(
                "已识别 {} playlist；当前不能通过此链接读取完整曲目，需要官方 OAuth 并在 Copy Playlist 选择账号可访问的歌单。",
                link.label
            )
        } else if link.playlist_id.is_none() {
            format!(
                "已识别 {} 域名，但未提取到有效歌单 ID；当前不能读取完整曲目。请检查是否为完整歌单链接。",
                link.label
            )
        } else {
            "已识别链接，但当前不能读取完整曲目。当前项目没有接通该平台的官方公开歌单读取接口。"
                .into()
        },
        next_step: if auth {
            "配置 Provider 并连接官方账号，然后进入 Copy Playlist；平台数据不用于画像或 LLM。"
                .into()
        } else {
            "请使用本地文件或粘贴歌曲清单；尚无本项目可用的官方曲目导入接口。".into()
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn chinese_lookalike_paths_never_trigger_a_fetch() {
        for raw in [
            "https://music.163.com/not-playlist?id=123456",
            "https://music.163.com/playlist/123456/song/789",
            "https://y.music.163.com/playlist?id=123456",
            "https://y.qq.com/not-playlist/123456",
            "https://y.qq.com/n/ryqq/playlist/123456/song/789",
            "https://y.qq.com/n/ryqq/playlist/invalid?id=123456",
        ] {
            let result = PlatformService::new().inspect_link(raw).await.unwrap();
            assert!(!result.playlist_id_valid, "{raw}");
            assert_eq!(result.access_status, "not_checked");
            assert!(result.preview_tracks.is_empty());
        }
    }

    #[test]
    fn chinese_accessibility_redirects_stay_on_exact_https_platform_hosts() {
        let source = Url::parse("https://music.163.com/playlist?id=123456").unwrap();
        for raw in [
            "http://music.163.com/playlist?id=123456",
            "https://unverified.music.163.com/playlist?id=123456",
            "https://music.163.com:80/playlist?id=123456",
            "https://music.163.com/other?id=123456",
            "https://y.qq.com/n/ryqq/playlist/123456",
            "https://music.163.com.evil.example/playlist?id=123456",
            "https://user:pass@music.163.com/playlist?id=123456",
            "https://localhost/playlist?id=123456",
            "https://127.0.0.1/playlist?id=123456",
            "https://169.254.169.254/",
            "https://10.0.0.1/",
            "https://172.16.0.1/",
            "https://192.168.1.1/",
            "https://[::1]/",
            "file:///etc/passwd",
            "ftp://music.163.com/playlist?id=123456",
        ] {
            assert!(
                !public_redirect_allowed(&Url::parse(raw).unwrap(), std::slice::from_ref(&source)),
                "{raw}"
            );
        }
        assert!(public_redirect_allowed(
            &source,
            &[Url::parse("https://163cn.tv/synthetic").unwrap()]
        ));
        assert!(!public_redirect_allowed(&source, &vec![source.clone(); 5]));
        assert!(!public_redirect_allowed(&source, &[]));
        let qq = Url::parse("https://y.qq.com/n/ryqq_v2/playlist/123456").unwrap();
        assert!(public_redirect_allowed(
            &qq,
            &[Url::parse("https://y.qq.com/n/ryqq/playlist/123456").unwrap()]
        ));
    }

    #[test]
    fn qq_current_official_route_and_netease_fragment_canonicalize() {
        for (raw, expected) in [
            (
                "https://y.qq.com/n/ryqq_v2/playlist/123456?tracking=ignored",
                "https://y.qq.com/n/ryqq/playlist/123456",
            ),
            (
                "https://music.163.com/#/playlist?id=123456&tracking=ignored",
                "https://music.163.com/playlist?id=123456",
            ),
        ] {
            let result = recognize_link(raw).unwrap().unwrap();
            assert_eq!(result.normalized_url.as_deref(), Some(expected));
        }
    }

    #[test]
    fn chinese_url_variants_remain_honest_about_ids() {
        for (url, platform, id) in [
            (
                "https://music.163.com/#/playlist?id=123456",
                "netease",
                Some("123456"),
            ),
            (
                "https://music.163.com/m/playlist?id=123456",
                "netease",
                Some("123456"),
            ),
            (
                "https://y.music.163.com/m/playlist?id=7736940069&userid=4899204022&creatorId=4899204022",
                "netease",
                Some("7736940069"),
            ),
            (
                "https://y.music.163.com/m/playlist?id=7558013954&userid=4899204022&creatorId=1321948954",
                "netease",
                Some("7558013954"),
            ),
            ("https://music.163.com/playlist?id=bad", "netease", None),
            ("https://163cn.tv/synthetic", "netease", None),
            (
                "https://y.qq.com/n/ryqq/playlist/123456",
                "qq_music",
                Some("123456"),
            ),
            (
                "https://y.qq.com/n/m/detail/taoge/index.html?id=123456",
                "qq_music",
                Some("123456"),
            ),
            ("https://y.qq.com/n/ryqq/playlist/bad", "qq_music", None),
            ("https://www.kugou.com/share/test", "kugou", None),
            (
                "https://m.kugou.com/playlist/123456",
                "kugou",
                Some("123456"),
            ),
            ("https://qishui.douyin.com/share/test", "qishui", None),
            ("https://qishui.douyin.com/playlist/123456", "qishui", None),
        ] {
            let link = recognize_link(url).unwrap().unwrap();
            assert_eq!(link.platform, platform);
            assert_eq!(link.playlist_id.as_deref(), id);
        }
    }

    #[test]
    fn netease_mobile_playlist_urls_canonicalize_without_fabricating_metadata() {
        for (raw, id) in [
            (
                "https://y.music.163.com/m/playlist?id=7736940069&userid=4899204022&creatorId=4899204022",
                "7736940069",
            ),
            (
                "https://y.music.163.com/m/playlist?id=7558013954&userid=4899204022&creatorId=1321948954",
                "7558013954",
            ),
        ] {
            let link = recognize_link(raw).unwrap().unwrap();
            assert_eq!(link.platform, "netease");
            assert_eq!(link.playlist_id.as_deref(), Some(id));
            assert_eq!(
                link.normalized_url.as_deref(),
                Some(format!("https://music.163.com/playlist?id={id}").as_str())
            );

            let preview = recognition_result(&link);
            assert_eq!(preview.capability, PublicLinkCapability::UrlRecognitionOnly);
            assert!(preview.playlist_name.is_none());
            assert!(preview.track_count.is_none());
            assert!(preview.preview_tracks.is_empty());
            assert!(preview.import_rows.is_empty());
        }
    }

    #[test]
    fn capability_matrix_does_not_confuse_catalog_and_account_access() {
        for configured in [false, true] {
            let items = PlatformService::new().capabilities(configured, configured, configured);
            let apple = items.iter().find(|p| p.platform == "apple_music").unwrap();
            assert!(apple.public_link_import_supported);
            assert!(
                !apple.auth_supported
                    && !apple.playlist_write_supported
                    && !apple.playlist_read_supported
            );
            assert_eq!(
                apple.public_playlist_links,
                if configured {
                    "TRACK_IMPORT_AVAILABLE"
                } else {
                    "CONFIG_REQUIRED"
                }
            );
            for platform in ["netease", "qq_music", "kugou", "qishui"] {
                let item = items.iter().find(|p| p.platform == platform).unwrap();
                assert_eq!(item.status, "FILE_IMPORT_AVAILABLE");
                assert!(!item.auth_supported);
                assert_eq!(
                    item.public_link_import_supported,
                    matches!(platform, "netease" | "qq_music" | "kugou")
                );
                assert!(item.file_import_supported);
            }
        }
    }

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
        let spotify =
            recognize_link("https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M?si=x")
                .unwrap()
                .unwrap();
        assert_eq!(
            spotify.playlist_id.as_deref(),
            Some("37i9dQZF1DXcBWIGoYBM5M")
        );
        let youtube = recognize_link("https://music.youtube.com/playlist?list=PLabcdefghijk")
            .unwrap()
            .unwrap();
        assert_eq!(youtube.playlist_id.as_deref(), Some("PLabcdefghijk"));
    }

    #[test]
    fn capability_matrix_never_advertises_fake_connect_or_writer() {
        let capabilities = PlatformService::new().capabilities(false, false, false);
        for platform in ["netease", "qq_music", "kugou", "kuwo", "qishui"] {
            let item = capabilities
                .iter()
                .find(|item| item.platform == platform)
                .unwrap();
            assert!(!item.auth_supported);
            assert!(!item.playlist_write_supported);
            assert!(item.file_import_supported);
            assert!(!item.transfer_destination_supported);
            assert!(!item.playlist_read_for_copy);
            assert!(!item.copy_destination_supported);
            assert_eq!(item.status, "FILE_IMPORT_AVAILABLE");
        }
        let spotify = capabilities
            .iter()
            .find(|item| item.platform == "spotify")
            .unwrap();
        let youtube = capabilities
            .iter()
            .find(|item| item.platform == "youtube_music")
            .unwrap();
        assert!(spotify.auth_supported && spotify.playlist_write_supported);
        assert!(youtube.auth_supported && youtube.playlist_write_supported);
        assert!(spotify.playlist_read_for_copy && spotify.copy_destination_supported);
        assert!(youtube.playlist_read_for_copy && youtube.copy_destination_supported);
        assert!(!spotify.playlist_read_for_recommendation);
        assert!(!youtube.playlist_read_for_compare);
        assert!(!spotify.configured && !youtube.configured);
        let apple = capabilities
            .iter()
            .find(|item| item.platform == "apple_music")
            .unwrap();
        assert_eq!(apple.status, "CONFIG_REQUIRED");
        assert!(!apple.auth_supported && !apple.configured);
    }
    #[tokio::test]
    async fn public_links_never_invent_tracks_or_real_verification() {
        for (url, expected) in [
            (
                "https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M",
                PublicLinkCapability::AuthRequired,
            ),
            (
                "https://music.youtube.com/playlist?list=PLabcdefghijk",
                PublicLinkCapability::AuthRequired,
            ),
            (
                "https://music.apple.com/cn/playlist/test/pl.abc123",
                PublicLinkCapability::UrlRecognitionOnly,
            ),
            (
                "https://www.kugou.com/share/test",
                PublicLinkCapability::UrlRecognitionOnly,
            ),
            (
                "https://qishui.douyin.com/share/test",
                PublicLinkCapability::UrlRecognitionOnly,
            ),
            (
                "https://example.org/playlist/123",
                PublicLinkCapability::Unsupported,
            ),
        ] {
            let result = PlatformService::new().inspect_link(url).await.unwrap();
            assert_eq!(result.capability, expected);
            assert!(result.preview_tracks.is_empty());
            assert!(result.track_count.is_none());
            assert!(!result.can_analyze);
            assert!(
                !serde_json::to_string(&result)
                    .unwrap()
                    .contains("REAL_VERIFIED")
            );
        }
    }

    #[test]
    fn invalid_urls_and_unsafe_authorities_are_rejected() {
        assert!(recognize_link("not a URL").is_err());
        for url in [
            "file:///etc/passwd",
            "https://user:pass@open.spotify.com/playlist/test",
            "http://music.163.com:8080/playlist?id=123",
            "https://music.163.com.evil.example/playlist?id=123",
            "https://unverified.music.163.com/m/playlist?id=123",
        ] {
            assert!(recognize_link(url).unwrap().is_none());
        }
    }

    #[tokio::test]
    async fn supported_domains_with_invalid_ids_do_not_trigger_network_or_import() {
        for url in [
            "https://open.spotify.com/playlist/invalid",
            "https://www.youtube.com/playlist?list=bad!",
            "https://music.163.com/playlist?id=abc",
            "https://music.163.com/song?id=123456",
            "https://y.qq.com/n/ryqq/playlist/not-an-id",
            "https://music.apple.com/cn/album/test/123",
        ] {
            let result = PlatformService::new().inspect_link(url).await.unwrap();
            assert_eq!(result.capability, PublicLinkCapability::UrlRecognitionOnly);
            assert!(!result.playlist_id_valid);
            assert_eq!(result.access_status, "not_checked");
            assert!(result.preview_tracks.is_empty());
            assert_eq!(result.declared_count, None);
            assert_eq!(result.visible_count, 0);
            assert_eq!(result.imported_count, 0);
            assert_eq!(result.skipped_count, 0);
            assert_eq!(result.unexposed_count, 0);
            assert!(!result.partial_import);
        }
    }

    #[test]
    fn netease_invalid_url_has_no_import_counts() {
        let link = recognize_link("https://music.163.com/playlist?id=not-a-number")
            .unwrap()
            .unwrap();
        let result = recognition_result(&link);
        assert!(!result.playlist_id_valid);
        assert_eq!(result.declared_count, None);
        assert_eq!(result.visible_count, 0);
        assert_eq!(result.imported_count, 0);
        assert_eq!(result.skipped_count, 0);
        assert_eq!(result.unexposed_count, 0);
        assert!(!result.partial_import);
    }

    #[test]
    fn netease_count_state_distinguishes_complete_large_and_partially_exposed() {
        assert_eq!(netease_import_state(Some(5), 5, 5), (0, false));
        assert_eq!(netease_import_state(Some(200), 200, 20), (0, true));
        assert_eq!(netease_import_state(Some(1196), 10, 10), (1186, true));
        assert_eq!(netease_import_state(Some(10), 10, 8), (0, true));
    }

    #[test]
    fn netease_fragment_route_and_page_structure_are_not_track_import() {
        let link = recognize_link("https://music.163.com/#/playlist?id=123456")
            .unwrap()
            .unwrap();
        assert_eq!(link.playlist_id.as_deref(), Some("123456"));
        assert_eq!(
            inspect_public_structure(
                br#"<script type="application/ld+json">{"@type":"MusicPlaylist"}</script>"#
            ),
            "schema_org_playlist_found_policy_unverified"
        );
        assert!(recognition_result(&link).preview_tracks.is_empty());
    }

    #[tokio::test]
    async fn public_page_reader_rejects_oversized_and_broken_bodies() {
        use tokio::io::AsyncWriteExt;
        for (body, content_length, limit, expected) in [
            ("complete", 8, 8, Some(b"complete".to_vec())),
            ("oversized", 9, 8, None),
            ("partial", 20, 32, None),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                use tokio::io::AsyncReadExt;
                let mut buffer = [0; 4096];
                let _ = stream.read(&mut buffer).await;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {content_length}\r\nConnection: close\r\n\r\n{body}"
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            });
            let mut response = Client::new()
                .get(format!("http://{address}"))
                .send()
                .await
                .unwrap();
            assert_eq!(read_public_page(&mut response, limit).await, expected);
            server.await.unwrap();
        }
    }

    #[tokio::test]
    #[ignore = "Explicit anonymous network acceptance; public page availability may change"]
    async fn netease_real_small_public_playlist_acceptance() {
        let result = PlatformService::new()
            .inspect_link("https://music.163.com/playlist?id=7299150850")
            .await
            .unwrap();
        assert_eq!(result.playlist_id.as_deref(), Some("7299150850"));
        assert_eq!(result.publicly_accessible, Some(true));
        assert!(result.declared_count.is_some_and(|count| count <= 20));
        assert!(result.visible_count >= result.imported_count);
        assert!(result.imported_count <= netease_tracks::MAX_DETAILS);
        assert_eq!(
            result.skipped_count,
            result
                .import_rows
                .iter()
                .filter(|row| row.import_status == "SKIPPED_DETAIL_UNAVAILABLE")
                .count()
        );
        assert_eq!(
            result.unexposed_count,
            result
                .declared_count
                .unwrap()
                .saturating_sub(result.visible_count)
        );
    }

    #[tokio::test]
    #[ignore = "Explicit anonymous network acceptance; public page availability may change"]
    async fn netease_real_partial_public_page_acceptance() {
        let result = PlatformService::new()
            .inspect_link("https://y.music.163.com/m/playlist?id=7736940069")
            .await
            .unwrap();
        assert_eq!(result.playlist_id.as_deref(), Some("7736940069"));
        assert_eq!(result.publicly_accessible, Some(true));
        assert!(
            result.playlist_name.is_some(),
            "No public metadata: {}",
            result.structured_data_status
        );
        assert_eq!(result.track_count, Some(1196));
        assert_eq!(result.declared_count, Some(1196));
        assert!(result.visible_count > 0 && result.visible_count < 1196);
        assert_eq!(result.imported_count, result.preview_tracks.len());
        assert_eq!(
            result.skipped_count,
            result
                .import_rows
                .iter()
                .filter(|row| row.import_status == "SKIPPED_DETAIL_UNAVAILABLE")
                .count()
        );
        assert_eq!(result.unexposed_count, 1196 - result.visible_count);
        assert!(result.partial_import);
        assert_eq!(
            result.capability,
            PublicLinkCapability::TrackImportAvailable
        );
        assert!(!result.preview_tracks.is_empty() && result.preview_tracks.len() <= 20);
        assert!(result.import_rows.len() <= 20);
        assert!(result.can_analyze);
        assert!(result.message.contains("公开页面仅提供部分歌曲"));
        // Aggregate public metadata only; never output raw pages or headers.
        println!(
            "declared={:?}; status={}; imported={}",
            result.track_count,
            result.structured_data_status,
            result.preview_tracks.len()
        );
    }
}
