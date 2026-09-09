use crate::models::{AgentSettings, StructuredTextTrack};
use reqwest::{Client, redirect::Policy};
use serde::Deserialize;
use std::time::Duration;

#[derive(Debug, Deserialize)]
struct StructuredTrackEnvelope {
    tracks: Vec<StructuredTextTrack>,
}

pub async fn parse_low_confidence_text(
    content: &str,
    settings: &AgentSettings,
) -> Result<Vec<StructuredTextTrack>, String> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| "LLM parser fallback 未配置；请补充分隔符后重试".to_string())?;
    parse_with_key(content, settings, &api_key).await
}

async fn parse_with_key(
    content: &str,
    settings: &AgentSettings,
    api_key: &str,
) -> Result<Vec<StructuredTextTrack>, String> {
    let lines = content
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    if lines.is_empty() || lines.len() > 100 || content.chars().count() > 20_000 {
        return Err("LLM parser fallback 仅处理 1–100 行且不超过 20,000 字符".into());
    }
    let numbered = lines
        .iter()
        .enumerate()
        .map(|(index, line)| format!("{}\t{}", index + 1, line))
        .collect::<Vec<_>>()
        .join("\n");
    let prompt = format!(
        "逐行识别下面文本中已经存在的歌曲标题和歌手。不得补充、翻译、纠正或生成歌曲；无法确定时 confidence 必须低于 0.8。保持原顺序和相同行数。只返回 JSON：{{\"tracks\":[{{\"title\":\"原文片段\",\"artist\":\"原文片段\",\"confidence\":0.0}}]}}。\n{numbered}"
    );
    let endpoint = if settings
        .endpoint
        .trim_end_matches('/')
        .ends_with("chat/completions")
    {
        settings.endpoint.clone()
    } else {
        format!(
            "{}/chat/completions",
            settings.endpoint.trim_end_matches('/')
        )
    };
    let client = Client::builder()
        .timeout(Duration::from_secs(
            settings.request_timeout_seconds.clamp(2, 30),
        ))
        .redirect(Policy::none())
        .build()
        .map_err(|_| "LLM parser fallback 客户端初始化失败".to_string())?;
    let body = serde_json::json!({
        "model": settings.model,
        "temperature": 0,
        "max_tokens": settings.max_tokens.min(800),
        "response_format": { "type": "json_object" },
        "messages": [
            { "role": "system", "content": "你是严格的多语言文本结构化器。你只能复制输入中的 title 和 artist 片段，不得生成音乐事实。" },
            { "role": "user", "content": prompt }
        ]
    });
    let response = client
        .post(endpoint)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
        .map_err(|_| "LLM parser fallback 请求失败；Need confirmation".to_string())?;
    if !response.status().is_success() {
        return Err(format!(
            "LLM parser fallback 返回 HTTP {}；Need confirmation",
            response.status()
        ));
    }
    let payload: serde_json::Value = response
        .json()
        .await
        .map_err(|_| "LLM parser fallback 响应不是 JSON；Need confirmation".to_string())?;
    let content = payload
        .pointer("/choices/0/message/content")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "LLM parser fallback 缺少结构化结果；Need confirmation".to_string())?;
    parse_response(content)
}

fn parse_response(content: &str) -> Result<Vec<StructuredTextTrack>, String> {
    let trimmed = content.trim();
    if !trimmed.starts_with('{') || !trimmed.ends_with('}') {
        return Err("LLM parser fallback 未返回严格 JSON；Need confirmation".into());
    }
    serde_json::from_str::<StructuredTrackEnvelope>(trimmed)
        .map(|value| value.tracks)
        .map_err(|_| "LLM parser fallback schema 无效；Need confirmation".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Json, Router, routing::post};
    use tokio::net::TcpListener;

    #[test]
    fn strict_response_parser_rejects_non_json_or_wrong_schema() {
        assert!(parse_response("```json\n{}\n```").is_err());
        assert!(parse_response(r#"{"songs":[]}"#).is_err());
        assert_eq!(
            parse_response(
                r#"{"tracks":[{"title":"Love Story","artist":"Taylor Swift","confidence":0.96}]}"#
            )
            .unwrap()[0]
                .artist,
            "Taylor Swift"
        );
    }

    #[tokio::test]
    async fn llm_fallback_uses_structured_chat_response_without_generating_tracks() {
        let app = Router::new().route(
            "/chat/completions",
            post(|| async {
                Json(serde_json::json!({
                    "choices": [{"message": {"content": "{\"tracks\":[{\"title\":\"Love Story\",\"artist\":\"Taylor Swift\",\"confidence\":0.97}]}"}}]
                }))
            }),
        );
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let settings = AgentSettings {
            endpoint: format!("http://{address}"),
            retry_limit: 0,
            ..AgentSettings::default()
        };
        let tracks = parse_with_key("Love Story Taylor Swift", &settings, "synthetic-key")
            .await
            .unwrap();
        assert_eq!(tracks.len(), 1);
        assert_eq!(tracks[0].title, "Love Story");
        assert_eq!(tracks[0].artist, "Taylor Swift");
    }
}
