use crate::models::{AppleMusicBootstrap, ProviderConfigurationStatus};
use base64::Engine;
use serde_json::Value;

pub const APPLE_MUSICKIT_SCRIPT: &str = "https://js-cdn.music.apple.com/musickit/v3/musickit.js";

#[derive(Clone, Default)]
pub struct AppleMusicConnector {
    developer_token: Option<String>,
}

impl AppleMusicConnector {
    pub fn new() -> Self {
        Self {
            developer_token: nonempty_env("APPLE_MUSIC_DEVELOPER_TOKEN"),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.configuration_status().configured
    }

    pub fn configuration_status(&self) -> ProviderConfigurationStatus {
        let variables = [
            "APPLE_TEAM_ID",
            "APPLE_KEY_ID",
            "APPLE_PRIVATE_KEY_PATH",
            "APPLE_MUSIC_DEVELOPER_TOKEN",
        ];
        let present: Vec<String> = variables
            .iter()
            .filter(|name| nonempty_env(name).is_some())
            .map(|item| (*item).into())
            .collect();
        let team_ready = present.iter().any(|item| item == "APPLE_TEAM_ID");
        let key_ready = present.iter().any(|item| item == "APPLE_KEY_ID");
        let token_ready = self
            .developer_token
            .as_deref()
            .is_some_and(developer_token_looks_valid);
        let configured = team_ready && key_ready && token_ready;
        let missing: Vec<String> = variables
            .iter()
            .filter(|name| {
                if **name == "APPLE_PRIVATE_KEY_PATH" && token_ready {
                    false
                } else {
                    !present.iter().any(|item| item == **name)
                }
            })
            .map(|item| (*item).into())
            .collect();
        ProviderConfigurationStatus {
            platform: "apple_music".into(),
            display_name: "Apple Music".into(),
            configured,
            validation_status: if !team_ready || !key_ready {
                "missing_developer_identity"
            } else if self.developer_token.is_none() {
                "missing_developer_token"
            } else if !token_ready {
                "invalid_developer_token_shape"
            } else {
                "musickit_ready_unverified"
            }
            .into(),
            required_environment_variables: variables.iter().map(|item| (*item).into()).collect(),
            present_environment_variables: present,
            missing_environment_variables: missing,
            redirect_uri: None,
            dashboard_url: "https://developer.apple.com/account/resources/identifiers/list/mediaServiceId".into(),
            setup_steps: vec![
                "加入 Apple Developer Program，在 Certificates, Identifiers & Profiles 创建 Media ID 并启用 MusicKit。".into(),
                "创建 MusicKit 私钥，记录 Team ID 与 Key ID；私钥文件必须位于仓库之外。".into(),
                "在后端用 ES256 生成 Developer Token（最长 6 个月，可限制 origin），或安全设置预生成的 APPLE_MUSIC_DEVELOPER_TOKEN。".into(),
                "MusicKit on the Web 在 Apple 官方界面获取并自动管理 Music User Token；不要把用户 token 写入 Git 或通用环境文件。".into(),
                "在真实订阅账号授权、歌单读取与写回验收完成前，页面保持“尚未完成真实账号验收”。".into(),
            ],
            secrets_exposed_to_frontend: false,
            message: if configured {
                "MusicKit 初始化配置已就绪，但本机尚未完成真实 Apple Music 订阅账号验收。Developer Token 会按 MusicKit Web 官方机制发送到浏览器，私钥永不发送。".into()
            } else {
                "MusicKit 配置不完整；私钥只能保存在仓库外的后端安全路径。".into()
            },
        }
    }

    pub fn bootstrap(&self) -> AppleMusicBootstrap {
        let configured = self.is_configured();
        AppleMusicBootstrap {
            configured,
            developer_token: configured.then(|| self.developer_token.clone()).flatten(),
            app_name: "MelodyPath".into(),
            app_build: env!("CARGO_PKG_VERSION").into(),
            real_account_validation: "not_completed".into(),
            message: if configured {
                format!(
                    "MusicKit Web 可使用 Apple 官方脚本 {APPLE_MUSICKIT_SCRIPT} 初始化；仍需用户本人在 Apple 授权界面完成真实验收。"
                )
            } else {
                "MusicKit 尚未配置；未显示或伪造账号连接。".into()
            },
        }
    }
}

fn developer_token_looks_valid(token: &str) -> bool {
    let mut parts = token.split('.');
    let (Some(_header), Some(payload), Some(_signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let Ok(bytes) = base64::engine::general_purpose::URL_SAFE_NO_PAD.decode(payload) else {
        return false;
    };
    let Ok(payload): Result<Value, _> = serde_json::from_slice(&bytes) else {
        return false;
    };
    payload.get("iss").and_then(Value::as_str).is_some()
        && payload.get("exp").and_then(Value::as_u64).is_some()
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
    fn rejects_placeholder_or_malformed_developer_token() {
        assert!(!developer_token_looks_valid("replace-me"));
        assert!(!developer_token_looks_valid("a.b.c"));
    }

    #[test]
    fn unconfigured_bootstrap_never_returns_a_token() {
        let connector = AppleMusicConnector {
            developer_token: None,
        };
        let bootstrap = connector.bootstrap();
        assert!(!bootstrap.configured);
        assert!(bootstrap.developer_token.is_none());
    }
}
