use crate::{
    demo,
    models::{AgentSettings, AgentTask, CreateAgentTaskRequest},
};
use anyhow::{Context, Result, bail};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{
    collections::HashMap,
    path::PathBuf,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{SystemTime, UNIX_EPOCH},
};
use tokio::{
    sync::RwLock,
    time::{Duration, Instant, sleep},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct AgentService {
    pool: SqlitePool,
    settings: Arc<RwLock<AgentSettings>>,
    cancellations: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
    allow_external_llm: bool,
}

impl AgentService {
    pub async fn new(database_path: PathBuf) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(database_path.to_string_lossy().as_ref())?
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        Self::from_pool(pool, true).await
    }

    #[cfg(test)]
    pub async fn in_memory() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        Self::from_pool(pool, false).await
    }

    async fn from_pool(pool: SqlitePool, allow_external_llm: bool) -> Result<Self> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS agent_tasks (
                id TEXT PRIMARY KEY,
                scenario TEXT NOT NULL,
                goal TEXT NOT NULL,
                status TEXT NOT NULL,
                current_step INTEGER NOT NULL,
                max_steps INTEGER NOT NULL,
                progress REAL NOT NULL,
                message TEXT NOT NULL,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                estimated_cost_usd REAL NOT NULL DEFAULT 0,
                result_json TEXT,
                error TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                revision INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&pool)
        .await?;
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        )
        .execute(&pool)
        .await?;
        sqlx::query("UPDATE agent_tasks SET status = 'failed', error = '服务重启导致任务中断', message = '任务已中断', updated_at = ?, revision = revision + 1 WHERE status IN ('queued', 'running')")
            .bind(now()).execute(&pool).await?;

        let settings = match sqlx::query_scalar::<_, String>(
            "SELECT value FROM app_settings WHERE key = 'agent_settings'",
        )
        .fetch_optional(&pool)
        .await?
        {
            Some(json) => serde_json::from_str(&json).unwrap_or_default(),
            None => AgentSettings::default(),
        };
        Ok(Self {
            pool,
            settings: Arc::new(RwLock::new(settings)),
            cancellations: Arc::new(RwLock::new(HashMap::new())),
            allow_external_llm,
        })
    }

    pub async fn settings(&self) -> AgentSettings {
        let mut value = self.settings.read().await.clone();
        value.api_key_available = std::env::var("OPENAI_API_KEY").is_ok();
        value
    }

    pub async fn update_settings(&self, mut value: AgentSettings) -> Result<AgentSettings> {
        validate_settings(&value)?;
        value.api_key_available = std::env::var("OPENAI_API_KEY").is_ok();
        let json = serde_json::to_string(&value)?;
        sqlx::query("INSERT INTO app_settings (key, value) VALUES ('agent_settings', ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
            .bind(json).execute(&self.pool).await?;
        *self.settings.write().await = value.clone();
        Ok(value)
    }

    pub async fn create_task(&self, request: CreateAgentTaskRequest) -> Result<AgentTask> {
        if !matches!(
            request.scenario.as_str(),
            "personal_exploration" | "friend_bridge"
        ) {
            bail!("scenario 仅支持 personal_exploration 或 friend_bridge");
        }
        let settings = self.settings().await;
        let id = Uuid::new_v4().to_string();
        let created = now();
        let goal = request.goal.unwrap_or_else(|| {
            if request.scenario == "personal_exploration" {
                "从 Korean R&B 逐步探索 Neo Soul".into()
            } else {
                "连接用户 A 与用户 B 的音乐偏好".into()
            }
        });
        sqlx::query("INSERT INTO agent_tasks (id, scenario, goal, status, current_step, max_steps, progress, message, created_at, updated_at, revision) VALUES (?, ?, ?, 'queued', 0, ?, 0, '等待 Rust Agent 调度', ?, ?, 0)")
            .bind(&id).bind(&request.scenario).bind(&goal).bind(settings.max_agent_steps as i64).bind(created).bind(created).execute(&self.pool).await?;
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancellations
            .write()
            .await
            .insert(id.clone(), cancel.clone());
        let service = self.clone();
        let task_id = id.clone();
        let scenario = request.scenario;
        tokio::spawn(async move {
            if let Err(error) = service.run_loop(&task_id, &scenario, cancel).await {
                let _ = service.fail_task(&task_id, &error.to_string()).await;
            }
            service.cancellations.write().await.remove(&task_id);
        });
        self.get_task(&id).await?.context("刚创建的任务不存在")
    }

    async fn run_loop(&self, id: &str, scenario: &str, cancelled: Arc<AtomicBool>) -> Result<()> {
        let settings = self.settings().await;
        let steps: &[&str] = if scenario == "personal_exploration" {
            &[
                "识别输入来源",
                "选择 DemoProvider",
                "读取歌单",
                "检查元数据完整度",
                "标准化歌曲与版本",
                "构建音乐品味画像",
                "理解探索目标",
                "生成跨 Genre 候选",
                "计算相关性与新颖性",
                "检查去重与歌手集中度",
                "使用可配置 LLM 增强解释（可选）",
                "构建可解释路线",
                "保存分析结果",
            ]
        } else {
            &[
                "识别双方输入",
                "选择跨平台 Provider",
                "统一 Track 模型",
                "计算共同歌曲与歌手",
                "计算 Genre 与年代兼容度",
                "识别双方特色",
                "寻找相邻风格连接",
                "生成桥梁候选",
                "计算 Bridge Score",
                "检查风格平滑与均衡",
                "使用可配置 LLM 增强解释（可选）",
                "生成关键歌曲解释",
                "保存桥梁歌单",
            ]
        };
        if settings.max_agent_steps < steps.len() as u32 {
            bail!(
                "最大 Agent 步数 {} 小于该场景所需的 {} 步，请在设置中提高上限",
                settings.max_agent_steps,
                steps.len()
            );
        }
        let started = Instant::now();
        for (index, message) in steps.iter().enumerate() {
            if cancelled.load(Ordering::SeqCst) {
                self.cancelled_task(id).await?;
                return Ok(());
            }
            if started.elapsed().as_secs() > settings.request_timeout_seconds.max(2) {
                bail!(
                    "Agent 超过配置的 {} 秒任务时限",
                    settings.request_timeout_seconds
                );
            }
            self.update_progress(id, (index + 1) as i64, steps.len() as f32, message)
                .await?;
            sleep(Duration::from_millis(260)).await;
        }
        let deterministic = if scenario == "personal_exploration" {
            serde_json::to_value(demo::demo_payload().personal)?
        } else {
            serde_json::to_value(demo::demo_payload().comparison)?
        };
        let llm = self
            .maybe_call_llm(scenario, &deterministic, &settings)
            .await;
        let (enhancement, input_tokens, output_tokens, cost, completion_message) = match llm {
            Ok(Some(usage)) => (
                serde_json::json!({ "status": "completed", "note": usage.note }),
                usage.input_tokens,
                usage.output_tokens,
                usage.cost,
                "Rust Agent 与可配置 LLM 已完成并保存结果",
            ),
            Ok(None) => (
                serde_json::json!({ "status": "skipped", "reason": "未配置 OPENAI_API_KEY，使用完全确定性的 Rust 结果" }),
                0,
                0,
                0.0,
                "Rust Agent 已完成并保存结果（离线模式）",
            ),
            Err(error) => (
                serde_json::json!({ "status": "failed", "reason": error.to_string(), "fallback": "确定性 Rust 结果仍然有效" }),
                0,
                0,
                0.0,
                "Rust Agent 已完成；LLM 增强失败并安全降级",
            ),
        };
        let result = serde_json::to_string(&serde_json::json!({
            "deterministic_result": deterministic,
            "llm_enhancement": enhancement,
        }))?;
        sqlx::query("UPDATE agent_tasks SET status = 'completed', progress = 1, message = ?, result_json = ?, input_tokens = ?, output_tokens = ?, estimated_cost_usd = ?, updated_at = ?, revision = revision + 1 WHERE id = ?")
            .bind(completion_message).bind(result).bind(input_tokens).bind(output_tokens).bind(cost).bind(now()).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    async fn maybe_call_llm(
        &self,
        scenario: &str,
        deterministic: &serde_json::Value,
        settings: &AgentSettings,
    ) -> Result<Option<LlmUsage>> {
        if !self.allow_external_llm {
            return Ok(None);
        }
        let api_key = match std::env::var("OPENAI_API_KEY") {
            Ok(value) if !value.trim().is_empty() => value,
            _ => return Ok(None),
        };
        let compact: String = deterministic.to_string().chars().take(6_000).collect();
        let prompt = format!(
            "场景：{scenario}\n以下是 Rust 确定性音乐引擎的结果：{compact}\n请用不超过 180 个中文字符总结这条音乐路径的连接逻辑。不得虚构曲目或元数据，不得推断年龄或心理状态。"
        );
        let estimated_input = (prompt.chars().count() as f64 / 3.0).ceil() as i64;
        let maximum_cost = estimated_input as f64 / 1_000_000.0 * settings.input_price_per_million
            + settings.max_tokens as f64 / 1_000_000.0 * settings.output_price_per_million;
        if maximum_cost > settings.max_cost_usd {
            bail!(
                "预计最大调用费用 ${maximum_cost:.4} 超过配置上限 ${:.4}",
                settings.max_cost_usd
            );
        }
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
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(settings.request_timeout_seconds))
            .build()?;
        let body = serde_json::json!({
            "model": settings.model,
            "temperature": settings.temperature,
            "max_tokens": settings.max_tokens,
            "messages": [
                { "role": "system", "content": "你是 MelodyPath 的音乐解释工具。只依据提供的结构化结果生成简洁、可核验的解释。" },
                { "role": "user", "content": prompt }
            ]
        });
        let mut last_error = None;
        for attempt in 0..=settings.retry_limit {
            let response = client
                .post(&endpoint)
                .bearer_auth(&api_key)
                .json(&body)
                .send()
                .await;
            match response {
                Ok(response) if response.status().is_success() => {
                    let payload: serde_json::Value = response.json().await?;
                    let note = payload
                        .pointer("/choices/0/message/content")
                        .and_then(serde_json::Value::as_str)
                        .context("LLM 响应缺少 choices[0].message.content")?
                        .to_string();
                    let input_tokens = payload
                        .pointer("/usage/prompt_tokens")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(estimated_input);
                    let output_tokens = payload
                        .pointer("/usage/completion_tokens")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or_else(|| (note.chars().count() as f64 / 3.0).ceil() as i64);
                    let cost = input_tokens as f64 / 1_000_000.0 * settings.input_price_per_million
                        + output_tokens as f64 / 1_000_000.0 * settings.output_price_per_million;
                    return Ok(Some(LlmUsage {
                        note,
                        input_tokens,
                        output_tokens,
                        cost,
                    }));
                }
                Ok(response) => {
                    let status = response.status();
                    let message = response.text().await.unwrap_or_default();
                    last_error = Some(anyhow::anyhow!(
                        "LLM HTTP {status}: {}",
                        message.chars().take(300).collect::<String>()
                    ));
                }
                Err(error) => last_error = Some(error.into()),
            }
            if attempt < settings.retry_limit {
                sleep(Duration::from_millis(200 * (attempt as u64 + 1))).await;
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("LLM 调用失败")))
    }

    async fn update_progress(&self, id: &str, step: i64, total: f32, message: &str) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET status = 'running', current_step = ?, progress = ?, message = ?, updated_at = ?, revision = revision + 1 WHERE id = ?")
            .bind(step).bind(step as f32 / total).bind(message).bind(now()).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    async fn fail_task(&self, id: &str, error: &str) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET status = 'failed', message = 'Agent 执行失败', error = ?, updated_at = ?, revision = revision + 1 WHERE id = ? AND status NOT IN ('completed', 'cancelled')")
            .bind(error).bind(now()).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    async fn cancelled_task(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET status = 'cancelled', message = '用户已中断任务', updated_at = ?, revision = revision + 1 WHERE id = ?")
            .bind(now()).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn cancel(&self, id: &str) -> Result<AgentTask> {
        let flag = self
            .cancellations
            .read()
            .await
            .get(id)
            .cloned()
            .context("任务不存在、已结束或服务已重启")?;
        flag.store(true, Ordering::SeqCst);
        // Update immediately for responsive UI; the loop observes the same flag before its next tool step.
        self.cancelled_task(id).await?;
        self.get_task(id).await?.context("任务不存在")
    }

    pub async fn get_task(&self, id: &str) -> Result<Option<AgentTask>> {
        Ok(
            sqlx::query_as::<_, AgentTask>("SELECT * FROM agent_tasks WHERE id = ?")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    pub async fn list_tasks(&self) -> Result<Vec<AgentTask>> {
        Ok(sqlx::query_as::<_, AgentTask>(
            "SELECT * FROM agent_tasks ORDER BY created_at DESC LIMIT 100",
        )
        .fetch_all(&self.pool)
        .await?)
    }
}

struct LlmUsage {
    note: String,
    input_tokens: i64,
    output_tokens: i64,
    cost: f64,
}

fn validate_settings(value: &AgentSettings) -> Result<()> {
    if !value.endpoint.starts_with("http://") && !value.endpoint.starts_with("https://") {
        bail!("模型 Endpoint 必须是 HTTP(S) URL");
    }
    if value.model.trim().is_empty() {
        bail!("模型名称不能为空");
    }
    if !(0.0..=2.0).contains(&value.temperature) {
        bail!("temperature 必须在 0 到 2 之间");
    }
    if !(12..=64).contains(&value.max_agent_steps) {
        bail!("最大 Agent 步数必须在 12 到 64 之间");
    }
    if !(2..=300).contains(&value.request_timeout_seconds) {
        bail!("超时必须在 2 到 300 秒之间");
    }
    if value.retry_limit > 5 {
        bail!("重试上限不能超过 5");
    }
    if value.max_cost_usd < 0.0 {
        bail!("费用上限不能为负数");
    }
    Ok(())
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn task_history_and_completion_are_persisted() {
        let service = AgentService::in_memory().await.unwrap();
        let task = service
            .create_task(CreateAgentTaskRequest {
                scenario: "personal_exploration".into(),
                goal: None,
            })
            .await
            .unwrap();
        for _ in 0..80 {
            let current = service.get_task(&task.id).await.unwrap().unwrap();
            if current.status == "completed" {
                break;
            }
            sleep(Duration::from_millis(50)).await;
        }
        let completed = service.get_task(&task.id).await.unwrap().unwrap();
        assert_eq!(completed.status, "completed");
        assert!(completed.result_json.is_some());
        assert_eq!(service.list_tasks().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn task_can_be_cancelled() {
        let service = AgentService::in_memory().await.unwrap();
        let task = service
            .create_task(CreateAgentTaskRequest {
                scenario: "friend_bridge".into(),
                goal: None,
            })
            .await
            .unwrap();
        let cancelled = service.cancel(&task.id).await.unwrap();
        assert_eq!(cancelled.status, "cancelled");
    }
}
