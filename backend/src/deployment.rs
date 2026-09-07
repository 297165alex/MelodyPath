//! Deployment-only configuration. Local development retains its existing defaults.
use anyhow::{Context, Result, bail};
use std::{collections::HashMap, path::PathBuf};

#[derive(Clone)]
pub struct Deployment {
    pub production: bool,
    pub origin: String,
    pub bind: String,
    pub data_dir: PathBuf,
    pub static_dir: Option<PathBuf>,
    pub demo_expires_at: Option<String>,
}

pub fn production() -> bool {
    std::env::var("MELODYPATH_ENV").is_ok_and(|v| v == "production")
}

impl Deployment {
    pub fn from_env() -> Result<Self> {
        Self::parse(&std::env::vars().collect())
    }

    fn parse(env: &HashMap<String, String>) -> Result<Self> {
        let get = |key: &str| {
            env.get(key)
                .map(String::as_str)
                .filter(|v| !v.trim().is_empty())
        };
        let production = get("MELODYPATH_ENV") == Some("production");
        let demo_expires_at = get("PUBLIC_DEMO_EXPIRES_AT").filter(|_| production);
        if let Some(date) = demo_expires_at {
            let parts: Vec<_> = date.split('-').collect();
            let valid = parts.len() == 3
                && parts[0].len() == 4
                && parts[1].len() == 2
                && parts[2].len() == 2
                && date.bytes().all(|b| b.is_ascii_digit() || b == b'-');
            let year = parts
                .first()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            let month = parts
                .get(1)
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            let day = parts
                .get(2)
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(0);
            let days = match month {
                4 | 6 | 9 | 11 => 30,
                2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
                2 => 28,
                1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                _ => 0,
            };
            if !valid || year == 0 || day == 0 || day > days {
                bail!("CONFIG_REQUIRED: PUBLIC_DEMO_EXPIRES_AT must be a valid YYYY-MM-DD date");
            }
        }
        if get("MELODYPATH_ENV").is_some_and(|v| !matches!(v, "production" | "development")) {
            bail!("CONFIG_REQUIRED: MELODYPATH_ENV must be production or development");
        }
        let origin = get("PUBLIC_BASE_URL")
            .unwrap_or("")
            .trim_end_matches('/')
            .to_string();
        if production {
            let public = reqwest::Url::parse(&origin)
                .context("CONFIG_REQUIRED: PUBLIC_BASE_URL must be an HTTPS origin")?;
            if public.scheme() != "https"
                || public.host_str().is_none()
                || public
                    .host_str()
                    .is_some_and(|h| h == "localhost" || h.parse::<std::net::IpAddr>().is_ok())
                || public.path() != "/"
                || public.query().is_some()
                || public.fragment().is_some()
                || !public.username().is_empty()
                || public.password().is_some()
            {
                bail!(
                    "CONFIG_REQUIRED: PUBLIC_BASE_URL must be a public HTTPS origin without path or credentials"
                );
            }
            if get("FRONTEND_URL").is_some_and(|v| v.trim_end_matches('/') != origin) {
                bail!(
                    "CONFIG_REQUIRED: FRONTEND_URL must equal PUBLIC_BASE_URL for same-origin deployment"
                );
            }
            for (name, path) in [
                ("SPOTIFY_REDIRECT_URI", "/api/spotify/callback"),
                ("GOOGLE_REDIRECT_URI", "/api/youtube/callback"),
            ] {
                if get(name).is_some_and(|v| v != format!("{origin}{path}")) {
                    bail!("CONFIG_REQUIRED: {name} must match the public callback exactly");
                }
            }
            if get("OAUTH_COOKIE_SECURE").is_some_and(|v| !matches!(v, "true" | "1" | "yes")) {
                bail!("CONFIG_REQUIRED: production cookies must be Secure");
            }
            if get("OAUTH_TOKEN_STORE") != Some("server_encrypted") {
                bail!("CONFIG_REQUIRED: production requires OAUTH_TOKEN_STORE=server_encrypted");
            }
            if get("MELODYPATH_DATA_DIR").is_none() || get("MELODYPATH_STATIC_DIR").is_none() {
                bail!(
                    "CONFIG_REQUIRED: production requires persistent MELODYPATH_DATA_DIR and MELODYPATH_STATIC_DIR"
                );
            }
        }
        let port = get("PORT")
            .unwrap_or("3000")
            .parse::<u16>()
            .context("CONFIG_REQUIRED: PORT must be a valid TCP port")?;
        let bind = get("MELODYPATH_BIND")
            .map(str::to_string)
            .unwrap_or_else(|| {
                format!(
                    "{}:{port}",
                    if production { "0.0.0.0" } else { "127.0.0.1" }
                )
            });
        let address: std::net::SocketAddr = bind
            .parse()
            .context("CONFIG_REQUIRED: invalid MELODYPATH_BIND")?;
        if production && address.ip().is_loopback() {
            bail!("CONFIG_REQUIRED: production backend must bind to an external interface");
        }
        Ok(Self {
            production,
            origin,
            bind,
            data_dir: PathBuf::from(get("MELODYPATH_DATA_DIR").unwrap_or(".")),
            static_dir: get("MELODYPATH_STATIC_DIR").map(PathBuf::from),
            demo_expires_at: demo_expires_at.map(str::to_string),
        })
    }
}

pub fn frontend_url() -> String {
    std::env::var("FRONTEND_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .or_else(|| {
            std::env::var("PUBLIC_BASE_URL")
                .ok()
                .filter(|v| !v.trim().is_empty())
        })
        .unwrap_or_else(|| {
            if production() {
                String::new()
            } else {
                "http://127.0.0.1:5173".into()
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn settings() -> HashMap<String, String> {
        [
            ("MELODYPATH_ENV", "production"),
            ("PUBLIC_BASE_URL", "https://music.example.com"),
            ("MELODYPATH_DATA_DIR", "/data"),
            ("MELODYPATH_STATIC_DIR", "/app/static"),
            ("OAUTH_TOKEN_STORE", "server_encrypted"),
            ("PORT", "10000"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect()
    }
    #[test]
    fn production_has_no_localhost_fallback_and_rejects_mismatched_callbacks() {
        let env = settings();
        let config = Deployment::parse(&env).unwrap();
        assert_eq!(config.bind, "0.0.0.0:10000");
        for (key, value) in [
            ("PUBLIC_BASE_URL", ""),
            ("PUBLIC_BASE_URL", "http://localhost:3000"),
            ("PUBLIC_BASE_URL", "https://music.example.com/path"),
            ("FRONTEND_URL", "http://127.0.0.1:5174"),
            (
                "SPOTIFY_REDIRECT_URI",
                "http://127.0.0.1:3000/api/spotify/callback",
            ),
            ("GOOGLE_REDIRECT_URI", "https://evil.example/callback"),
            ("OAUTH_COOKIE_SECURE", "false"),
            ("OAUTH_TOKEN_STORE", "windows_dpapi"),
        ] {
            let mut invalid = env.clone();
            invalid.insert(key.into(), value.into());
            assert!(Deployment::parse(&invalid).is_err(), "{key}");
        }
        for (key, path) in [
            ("SPOTIFY_REDIRECT_URI", "/api/spotify/callback"),
            ("GOOGLE_REDIRECT_URI", "/api/youtube/callback"),
        ] {
            let mut valid = env.clone();
            valid.insert(key.into(), format!("{}{path}", config.origin));
            assert!(Deployment::parse(&valid).is_ok());
        }
    }
    #[test]
    fn demo_date_is_validated_and_never_enables_local_notice() {
        let mut env = settings();
        for date in ["2026-02-29", "2026-13-01", "bad", "2026-01-00"] {
            env.insert("PUBLIC_DEMO_EXPIRES_AT".into(), date.into());
            assert!(Deployment::parse(&env).is_err());
        }
        env.insert("PUBLIC_DEMO_EXPIRES_AT".into(), "2024-02-29".into());
        assert_eq!(
            Deployment::parse(&env).unwrap().demo_expires_at.as_deref(),
            Some("2024-02-29")
        );
        env.insert("MELODYPATH_ENV".into(), "development".into());
        assert!(Deployment::parse(&env).unwrap().demo_expires_at.is_none());
    }
    #[test]
    fn local_defaults_remain_available() {
        let config = Deployment::parse(&HashMap::new()).unwrap();
        assert!(!config.production);
        assert_eq!(config.bind, "127.0.0.1:3000");
    }
}
