use crate::{
    demo, engine,
    models::{
        AgentDecision, AgentGoal, AgentIntent, AgentPlan, AgentRun, AgentRunStatus, AgentSettings,
        AgentState, AgentStep, AgentStepStatus, AgentTask, AgentToolCall, AgentToolResult,
        ComparisonReport, CreateAgentTaskRequest, PersonalDemo,
    },
    recommendation::{build_route, select_recommendation_seeds},
};
use anyhow::{Context, Result, bail};
use sqlx::{
    SqlitePool,
    sqlite::{SqliteConnectOptions, SqlitePoolOptions},
};
use std::{
    collections::{HashMap, HashSet},
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

pub type AnalysisRegistry = Arc<RwLock<HashMap<String, PersonalDemo>>>;

#[derive(Clone)]
pub struct AgentService {
    pool: SqlitePool,
    settings: Arc<RwLock<AgentSettings>>,
    cancellations: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
    analyses: AnalysisRegistry,
    allow_external_llm: bool,
    #[cfg(test)]
    transient_failures: Arc<RwLock<HashMap<String, u32>>>,
}

impl AgentService {
    pub async fn new(database_path: PathBuf, analyses: AnalysisRegistry) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(database_path.to_string_lossy().as_ref())?
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await?;
        Self::from_pool(pool, true, analyses).await
    }

    #[cfg(test)]
    pub async fn in_memory() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        Self::from_pool(pool, false, Arc::new(RwLock::new(HashMap::new()))).await
    }

    async fn from_pool(
        pool: SqlitePool,
        allow_external_llm: bool,
        analyses: AnalysisRegistry,
    ) -> Result<Self> {
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS agent_tasks (
                id TEXT PRIMARY KEY,
                scenario TEXT NOT NULL,
                goal TEXT NOT NULL,
                status TEXT NOT NULL,
                current_step INTEGER NOT NULL,
                max_steps INTEGER NOT NULL,
                progress REAL NOT NULL,
                message TEXT NOT NULL,
                analysis_id TEXT,
                analysis_a_id TEXT,
                analysis_b_id TEXT,
                transfer_preview_id TEXT,
                use_demo INTEGER NOT NULL DEFAULT 0,
                data_state TEXT NOT NULL DEFAULT 'NONE',
                decision_mode TEXT NOT NULL DEFAULT 'DETERMINISTIC_FALLBACK',
                decisions_json TEXT NOT NULL DEFAULT '[]',
                normalized_intent_json TEXT NOT NULL DEFAULT '{}',
                plan_json TEXT NOT NULL DEFAULT '{"scenario":"unknown","steps":[]}',
                completed_steps_json TEXT NOT NULL DEFAULT '[]',
                pending_steps_json TEXT NOT NULL DEFAULT '[]',
                tool_calls_json TEXT NOT NULL DEFAULT '[]',
                tool_results_json TEXT NOT NULL DEFAULT '[]',
                retries INTEGER NOT NULL DEFAULT 0,
                warnings_json TEXT NOT NULL DEFAULT '[]',
                checkpoint_json TEXT,
                input_tokens INTEGER NOT NULL DEFAULT 0,
                output_tokens INTEGER NOT NULL DEFAULT 0,
                estimated_cost_usd REAL NOT NULL DEFAULT 0,
                result_json TEXT,
                error TEXT,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                revision INTEGER NOT NULL DEFAULT 0
            )"#,
        )
        .execute(&pool)
        .await?;
        for migration in [
            "ALTER TABLE agent_tasks ADD COLUMN normalized_intent_json TEXT NOT NULL DEFAULT '{}'",
            "ALTER TABLE agent_tasks ADD COLUMN plan_json TEXT NOT NULL DEFAULT '{\"scenario\":\"unknown\",\"steps\":[]}'",
            "ALTER TABLE agent_tasks ADD COLUMN completed_steps_json TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE agent_tasks ADD COLUMN pending_steps_json TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE agent_tasks ADD COLUMN tool_calls_json TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE agent_tasks ADD COLUMN tool_results_json TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE agent_tasks ADD COLUMN retries INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE agent_tasks ADD COLUMN warnings_json TEXT NOT NULL DEFAULT '[]'",
            "ALTER TABLE agent_tasks ADD COLUMN checkpoint_json TEXT",
            "ALTER TABLE agent_tasks ADD COLUMN analysis_id TEXT",
            "ALTER TABLE agent_tasks ADD COLUMN analysis_a_id TEXT",
            "ALTER TABLE agent_tasks ADD COLUMN analysis_b_id TEXT",
            "ALTER TABLE agent_tasks ADD COLUMN transfer_preview_id TEXT",
            "ALTER TABLE agent_tasks ADD COLUMN use_demo INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE agent_tasks ADD COLUMN data_state TEXT NOT NULL DEFAULT 'NONE'",
            "ALTER TABLE agent_tasks ADD COLUMN decision_mode TEXT NOT NULL DEFAULT 'DETERMINISTIC_FALLBACK'",
            "ALTER TABLE agent_tasks ADD COLUMN decisions_json TEXT NOT NULL DEFAULT '[]'",
        ] {
            if let Err(error) = sqlx::query(migration).execute(&pool).await
                && !error
                    .to_string()
                    .to_lowercase()
                    .contains("duplicate column")
            {
                return Err(error.into());
            }
        }
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS app_settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        )
        .execute(&pool)
        .await?;
        sqlx::query("UPDATE agent_tasks SET status = 'FAILED', error = '服务重启导致任务中断；可从检查点恢复', message = '任务已中断，可恢复', updated_at = ?, revision = revision + 1 WHERE status IN ('queued', 'running', 'PLANNING', 'RUNNING')")
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
            analyses,
            allow_external_llm,
            #[cfg(test)]
            transient_failures: Arc::new(RwLock::new(HashMap::new())),
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
        let data_state = self.validate_binding(&request).await?;
        let settings = self.settings().await;
        let id = Uuid::new_v4().to_string();
        let created = now();
        let goal = request.goal.clone().unwrap_or_else(|| {
            if request.scenario == "personal_exploration" {
                "从 Korean R&B 逐步探索 Neo Soul".into()
            } else {
                "连接用户 A 与用户 B 的音乐偏好".into()
            }
        });
        let intent = normalize_intent(&request.scenario, &goal);
        let plan = build_plan(&request.scenario);
        let pending_steps: Vec<_> = plan.steps.iter().map(|step| step.step_id.clone()).collect();
        let llm_allowed_for_binding = self.binding_allows_llm(&request).await;
        let decision_mode = if self.allow_external_llm
            && llm_allowed_for_binding
            && std::env::var("OPENAI_API_KEY").is_ok_and(|value| !value.trim().is_empty())
        {
            "LLM"
        } else {
            "DETERMINISTIC_FALLBACK"
        };
        sqlx::query("INSERT INTO agent_tasks (id, scenario, goal, status, current_step, max_steps, progress, message, analysis_id, analysis_a_id, analysis_b_id, transfer_preview_id, use_demo, data_state, decision_mode, decisions_json, normalized_intent_json, plan_json, completed_steps_json, pending_steps_json, tool_calls_json, tool_results_json, retries, warnings_json, checkpoint_json, created_at, updated_at, revision) VALUES (?, ?, ?, 'PLANNING', 0, ?, 0, '已理解目标，准备执行计划', ?, ?, ?, ?, ?, ?, ?, '[]', ?, ?, '[]', ?, '[]', '[]', 0, '[]', ?, ?, ?, 0)")
            .bind(&id)
            .bind(&request.scenario)
            .bind(&goal)
            .bind(settings.max_agent_steps as i64)
            .bind(&request.analysis_id)
            .bind(&request.analysis_a_id)
            .bind(&request.analysis_b_id)
            .bind(&request.transfer_preview_id)
            .bind(request.use_demo)
            .bind(&data_state)
            .bind(decision_mode)
            .bind(serde_json::to_string(&intent)?)
            .bind(serde_json::to_string(&plan)?)
            .bind(serde_json::to_string(&pending_steps)?)
            .bind(serde_json::to_string(&serde_json::json!({ "next_step": 0 }))?)
            .bind(created)
            .bind(created)
            .execute(&self.pool)
            .await?;
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

    async fn validate_binding(&self, request: &CreateAgentTaskRequest) -> Result<String> {
        if request.use_demo {
            return Ok("DEMO".into());
        }
        let analyses = self.analyses.read().await;
        let validate_real = |id: Option<&String>| -> Result<&PersonalDemo> {
            let id = id.context("真实 Agent 任务必须绑定 analysis_id；请先分析歌单")?;
            let analysis = analyses
                .get(id)
                .context("绑定的分析不存在或服务已重启，请重新分析歌单")?;
            if analysis.report.is_demo || analysis.playlist.is_demo {
                bail!("真实 Agent 任务不能绑定 Demo 分析");
            }
            Ok(analysis)
        };
        if request.scenario == "personal_exploration" {
            let analysis = validate_real(request.analysis_id.as_ref())?;
            Ok(analysis_data_state(analysis))
        } else {
            let a = validate_real(request.analysis_a_id.as_ref())?;
            let b = validate_real(request.analysis_b_id.as_ref())?;
            Ok(
                if analysis_data_state(a) == "REAL_TEXT" && analysis_data_state(b) == "REAL_TEXT" {
                    "REAL_TEXT".into()
                } else {
                    "REAL_FILE".into()
                },
            )
        }
    }

    async fn binding_allows_llm(&self, request: &CreateAgentTaskRequest) -> bool {
        if request.use_demo {
            return true;
        }
        let analyses = self.analyses.read().await;
        [
            request.analysis_id.as_ref(),
            request.analysis_a_id.as_ref(),
            request.analysis_b_id.as_ref(),
        ]
        .into_iter()
        .flatten()
        .filter_map(|id| analyses.get(id))
        .all(|analysis| {
            !analysis
                .playlist
                .source
                .to_ascii_lowercase()
                .contains("spotify")
                && analysis
                    .playlist
                    .tracks
                    .iter()
                    .all(|track| !track.platform.to_ascii_lowercase().contains("spotify"))
        })
    }

    async fn run_loop(&self, id: &str, scenario: &str, cancelled: Arc<AtomicBool>) -> Result<()> {
        let settings = self.settings().await;
        let task = self.get_task(id).await?.context("AgentRun 不存在")?;
        let context = self.resolve_context(&task).await?;
        let mut plan: AgentPlan =
            serde_json::from_str(&task.plan_json).unwrap_or_else(|_| build_plan(scenario));
        if settings.max_agent_steps < plan.steps.len() as u32 {
            bail!(
                "最大 Agent 步数 {} 小于该场景所需的 {} 步，请在设置中提高上限",
                settings.max_agent_steps,
                plan.steps.len()
            );
        }
        let mut intent: AgentIntent = serde_json::from_str(&task.normalized_intent_json)
            .unwrap_or_else(|_| normalize_intent(scenario, &task.goal));
        let mut tool_calls: Vec<AgentToolCall> =
            serde_json::from_str(&task.tool_calls_json).unwrap_or_default();
        let mut tool_results: Vec<AgentToolResult> =
            serde_json::from_str(&task.tool_results_json).unwrap_or_default();
        let mut decisions: Vec<AgentDecision> =
            serde_json::from_str(&task.decisions_json).unwrap_or_default();
        let mut retries = task.retries.max(0) as u32;
        let mut warnings: Vec<String> =
            serde_json::from_str(&task.warnings_json).unwrap_or_default();
        let mut decision_mode = task.decision_mode.clone();
        let mut input_tokens = task.input_tokens;
        let mut output_tokens = task.output_tokens;
        let mut estimated_cost = task.estimated_cost_usd;
        let started = Instant::now();

        loop {
            if cancelled.load(Ordering::SeqCst) {
                self.persist_run_state(
                    id,
                    AgentRunStatus::Cancelled,
                    &plan,
                    &tool_calls,
                    &tool_results,
                    &decisions,
                    &decision_mode,
                    retries,
                    &warnings,
                    input_tokens,
                    output_tokens,
                    estimated_cost,
                    "用户已中断任务；检查点已保存",
                )
                .await?;
                return Ok(());
            }
            if started.elapsed().as_secs() > settings.request_timeout_seconds.max(2) {
                bail!(
                    "Agent 超过配置的 {} 秒任务时限",
                    settings.request_timeout_seconds
                );
            }
            if tool_calls.iter().filter(|call| call.attempt == 1).count()
                >= settings.max_agent_steps as usize
                && plan
                    .steps
                    .iter()
                    .any(|step| step.status != AgentStepStatus::Completed)
            {
                bail!("Agent 已达到最大步骤上限 {}", settings.max_agent_steps);
            }

            let allowed_tools = currently_allowed_tools(&plan);
            let all_complete = allowed_tools.is_empty();
            let mut decision = if decision_mode == "LLM" {
                match self
                    .request_llm_decision(
                        &task,
                        &intent,
                        &allowed_tools,
                        &tool_results,
                        all_complete,
                        &settings,
                        output_tokens,
                        estimated_cost,
                    )
                    .await
                {
                    Ok(usage) => {
                        input_tokens += usage.input_tokens;
                        output_tokens += usage.output_tokens;
                        estimated_cost += usage.cost;
                        if output_tokens > i64::from(settings.max_tokens) {
                            bail!("LLM 输出 Token 已达到配置上限 {}", settings.max_tokens);
                        }
                        if estimated_cost > settings.max_cost_usd {
                            bail!("LLM 费用已达到配置上限 ${:.4}", settings.max_cost_usd);
                        }
                        usage.decision
                    }
                    Err(error) => {
                        let message = error.to_string();
                        if message.contains("预算上限") || message.contains("Token") {
                            bail!("{message}");
                        }
                        decision_mode = "DETERMINISTIC_FALLBACK".into();
                        warnings.push(format!("LLM 决策失败，已明确切换确定性计划：{message}"));
                        fallback_decision(scenario, &allowed_tools, &tool_results, all_complete)
                    }
                }
            } else {
                fallback_decision(scenario, &allowed_tools, &tool_results, all_complete)
            };

            validate_agent_decision(&decision, &allowed_tools, all_complete)?;
            if let Some(llm_intent) = decision.arguments.get("normalized_intent") {
                if let Ok(parsed) = serde_json::from_value::<AgentIntent>(llm_intent.clone()) {
                    intent = parsed;
                    sqlx::query("UPDATE agent_tasks SET normalized_intent_json = ? WHERE id = ?")
                        .bind(serde_json::to_string(&intent)?)
                        .bind(id)
                        .execute(&self.pool)
                        .await?;
                }
            }
            if all_complete {
                decision.finish = true;
                decisions.push(decision);
                break;
            }
            let tool = decision.next_tool.clone().context("决策未选择工具")?;
            decisions.push(decision.clone());
            let index = plan
                .steps
                .iter()
                .position(|step| step.tool == tool && step.status == AgentStepStatus::Pending)
                .context("决策选择的工具不在当前待执行计划中")?;
            plan.steps[index].status = AgentStepStatus::Running;
            let step_label = plan.steps[index].label.clone();
            self.persist_run_state(
                id,
                AgentRunStatus::Running,
                &plan,
                &tool_calls,
                &tool_results,
                &decisions,
                &decision_mode,
                retries,
                &warnings,
                input_tokens,
                output_tokens,
                estimated_cost,
                &format!("{} · {}", decision.action, step_label),
            )
            .await?;

            let mut completed = false;
            for attempt in 1..=(settings.retry_limit + 1) {
                let call = AgentToolCall {
                    call_id: Uuid::new_v4().to_string(),
                    step_id: plan.steps[index].step_id.clone(),
                    tool: tool.clone(),
                    attempt,
                    started_at: now(),
                };
                tool_calls.push(call.clone());
                match self.execute_agent_tool(&call, &context).await {
                    Ok(output) => {
                        validate_tool_output(&output)?;
                        tool_results.push(AgentToolResult {
                            call_id: call.call_id,
                            success: true,
                            recoverable: false,
                            summary: tool_result_summary(&tool, &output),
                            output,
                            completed_at: now(),
                        });
                        plan.steps[index].status = AgentStepStatus::Completed;
                        plan.steps[index].attempts = attempt;
                        plan.steps[index].last_error = None;
                        completed = true;
                        break;
                    }
                    Err(error) => {
                        let can_retry = attempt <= settings.retry_limit;
                        tool_results.push(AgentToolResult {
                            call_id: call.call_id,
                            success: false,
                            recoverable: can_retry,
                            summary: error.to_string(),
                            output: serde_json::json!({}),
                            completed_at: now(),
                        });
                        plan.steps[index].attempts = attempt;
                        plan.steps[index].last_error = Some(error.to_string());
                        if can_retry {
                            retries += 1;
                            sleep(Duration::from_millis(80 * u64::from(attempt))).await;
                        } else {
                            plan.steps[index].status = AgentStepStatus::Failed;
                            bail!("工具 {tool} 执行失败：{error}");
                        }
                    }
                }
            }
            if !completed {
                bail!("步骤 {} 未产生有效工具结果", plan.steps[index].step_id);
            }
            prune_unselected_strategy(&mut plan, &tool);
            if tool == "fetch_lastfm_candidates"
                && tool_results
                    .last()
                    .and_then(|result| result.output.get("surprise_pool_count"))
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or_default()
                    < 4
                && !plan.steps.iter().any(|step| {
                    matches!(
                        step.tool.as_str(),
                        "genre_bridge_candidates" | "second_hop_artist_candidates"
                    )
                })
            {
                let insert_at = plan
                    .steps
                    .iter()
                    .position(|step| step.tool == "rank_recommendations")
                    .unwrap_or(plan.steps.len());
                for (offset, (label, recovery_tool)) in [
                    (
                        "惊喜候选不足：检查标签 / Genre Graph 两跳桥梁",
                        "genre_bridge_candidates",
                    ),
                    (
                        "惊喜候选不足：检查第二跳相似艺人网络",
                        "second_hop_artist_candidates",
                    ),
                ]
                .into_iter()
                .enumerate()
                {
                    plan.steps.insert(
                        insert_at + offset,
                        AgentStep {
                            step_id: format!("step_replan_{:02}_{offset}", decisions.len()),
                            label: label.into(),
                            tool: recovery_tool.into(),
                            status: AgentStepStatus::Pending,
                            attempts: 0,
                            last_error: None,
                        },
                    );
                }
            }
            if tool == "compare_playlists"
                && !plan.steps.iter().any(|step| {
                    matches!(
                        step.tool.as_str(),
                        "shared_affinity_strategy" | "complementary_bridge_strategy"
                    )
                })
            {
                let insert_at = plan
                    .steps
                    .iter()
                    .position(|step| step.tool == "score_compatibility")
                    .unwrap_or(plan.steps.len());
                for (offset, (label, strategy_tool)) in [
                    ("共同偏好优先的桥梁策略", "shared_affinity_strategy"),
                    ("互补偏好优先的桥梁策略", "complementary_bridge_strategy"),
                ]
                .into_iter()
                .enumerate()
                {
                    plan.steps.insert(
                        insert_at + offset,
                        AgentStep {
                            step_id: format!("step_strategy_{:02}_{offset}", decisions.len()),
                            label: label.into(),
                            tool: strategy_tool.into(),
                            status: AgentStepStatus::Pending,
                            attempts: 0,
                            last_error: None,
                        },
                    );
                }
            }
            self.persist_run_state(
                id,
                AgentRunStatus::Running,
                &plan,
                &tool_calls,
                &tool_results,
                &decisions,
                &decision_mode,
                retries,
                &warnings,
                input_tokens,
                output_tokens,
                estimated_cost,
                &format!("已完成：{step_label}；工具结果将参与下一次决策"),
            )
            .await?;
        }

        let deterministic = context.deterministic_result()?;
        let agent_run = AgentRun {
            run_id: id.into(),
            goal: AgentGoal {
                user_goal: task.goal.clone(),
                normalized_intent: intent.clone(),
            },
            plan: plan.clone(),
            state: AgentState {
                status: AgentRunStatus::Completed,
                current_step: plan.steps.len(),
                completed_steps: plan.steps.iter().map(|step| step.step_id.clone()).collect(),
                pending_steps: Vec::new(),
                retries,
                progress: 1.0,
                warnings: warnings.clone(),
            },
            tool_calls: tool_calls.clone(),
            tool_results: tool_results.clone(),
            decision_mode: decision_mode.clone(),
            decisions: decisions.clone(),
            input_tokens,
            output_tokens,
            estimated_cost_usd: estimated_cost,
            created_at: task.created_at,
            updated_at: now(),
        };
        let result = serde_json::to_string(&serde_json::json!({
            "data_state": task.data_state,
            "analysis_id": task.analysis_id,
            "analysis_a_id": task.analysis_a_id,
            "analysis_b_id": task.analysis_b_id,
            "decision_mode": decision_mode,
            "deterministic_result": deterministic,
            "normalized_intent": intent,
            "agent_run": agent_run,
            "plan": &plan,
            "decisions": &decisions,
            "tool_calls": &tool_calls,
            "tool_results": &tool_results,
            "retries": retries,
            "warnings": &warnings,
        }))?;
        sqlx::query("UPDATE agent_tasks SET status = 'COMPLETED', progress = 1, current_step = ?, message = ?, result_json = ?, normalized_intent_json = ?, decision_mode = ?, decisions_json = ?, input_tokens = ?, output_tokens = ?, estimated_cost_usd = ?, checkpoint_json = ?, updated_at = ?, revision = revision + 1 WHERE id = ?")
            .bind(plan.steps.len() as i64)
            .bind(if decision_mode == "LLM" { "LLM-assisted Agent 已完成真实工具循环" } else { "确定性 fallback Agent 已完成真实工具循环" })
            .bind(result)
            .bind(serde_json::to_string(&intent)?)
            .bind(&decision_mode)
            .bind(serde_json::to_string(&decisions)?)
            .bind(input_tokens)
            .bind(output_tokens)
            .bind(estimated_cost)
            .bind(serde_json::to_string(&serde_json::json!({ "next_step": plan.steps.len() }))?)
            .bind(now())
            .bind(id)
            .execute(&self.pool).await?;
        Ok(())
    }

    async fn request_llm_decision(
        &self,
        task: &AgentTask,
        intent: &AgentIntent,
        allowed_tools: &[String],
        tool_results: &[AgentToolResult],
        all_complete: bool,
        settings: &AgentSettings,
        used_output_tokens: i64,
        used_cost: f64,
    ) -> Result<LlmDecisionUsage> {
        let api_key = std::env::var("OPENAI_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .context("OPENAI_API_KEY 未配置")?;
        let remaining_tokens = i64::from(settings.max_tokens).saturating_sub(used_output_tokens);
        if remaining_tokens <= 0 {
            bail!("LLM Token 预算上限已耗尽");
        }
        let compact_results: Vec<_> = tool_results.iter().rev().take(6).rev().map(|result| {
            serde_json::json!({ "success": result.success, "summary": result.summary, "output": result.output })
        }).collect();
        let state = serde_json::json!({
            "scenario": task.scenario,
            "user_goal": task.goal,
            "normalized_intent": intent,
            "data_state": task.data_state,
            "allowed_tools": allowed_tools,
            "all_required_tools_complete": all_complete,
            "recent_tool_results": compact_results,
        });
        let prompt = format!(
            "根据这个紧凑状态选择下一步：{}\n只输出 JSON：{{\"action\":\"continue|replan|finish\",\"next_tool\":\"白名单工具或null\",\"arguments\":{{\"normalized_intent\":{{\"action\":\"...\",\"count\":null,\"novelty\":\"balanced\"}}}},\"reason\":\"可核验短理由\",\"finish\":false}}。若 all_required_tools_complete=true，必须 finish=true。不得输出思维链。",
            state
        );
        let estimated_input = (prompt.chars().count() as f64 / 3.0).ceil() as i64;
        let request_max_tokens = remaining_tokens.min(300);
        validate_llm_cost_budget(settings, used_cost, estimated_input, request_max_tokens)?;
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
            .timeout(Duration::from_secs(
                settings.request_timeout_seconds.min(30),
            ))
            .build()?;
        let body = serde_json::json!({
            "model": settings.model,
            "temperature": settings.temperature,
            "max_tokens": request_max_tokens,
            "messages": [
                { "role": "system", "content": "你是 MelodyPath Agent Controller。你只能选择提供的白名单 Rust 工具；歌曲分析、评分、去重和外部写入全部由 Rust 执行。只返回满足 schema 的 JSON，不输出隐藏推理。" },
                { "role": "user", "content": prompt }
            ]
        });
        let mut last_error = None;
        for attempt in 0..=settings.retry_limit {
            match client
                .post(&endpoint)
                .bearer_auth(&api_key)
                .json(&body)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    let payload: serde_json::Value = response.json().await?;
                    let content = payload
                        .pointer("/choices/0/message/content")
                        .and_then(serde_json::Value::as_str)
                        .context("LLM 响应缺少结构化 decision")?;
                    let decision = parse_agent_decision(content)?;
                    validate_agent_decision(&decision, allowed_tools, all_complete)?;
                    let input_tokens = payload
                        .pointer("/usage/prompt_tokens")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or(estimated_input);
                    let output_tokens = payload
                        .pointer("/usage/completion_tokens")
                        .and_then(serde_json::Value::as_i64)
                        .unwrap_or_else(|| (content.chars().count() as f64 / 3.0).ceil() as i64);
                    let cost = input_tokens as f64 / 1_000_000.0 * settings.input_price_per_million
                        + output_tokens as f64 / 1_000_000.0 * settings.output_price_per_million;
                    return Ok(LlmDecisionUsage {
                        decision,
                        input_tokens,
                        output_tokens,
                        cost,
                    });
                }
                Ok(response) => {
                    last_error = Some(anyhow::anyhow!("LLM HTTP {}", response.status()));
                }
                Err(error) => last_error = Some(error.into()),
            }
            if attempt < settings.retry_limit {
                sleep(Duration::from_millis(200 * (u64::from(attempt) + 1))).await;
            }
        }
        Err(last_error.unwrap_or_else(|| anyhow::anyhow!("LLM 决策调用失败")))
    }

    async fn persist_run_state(
        &self,
        id: &str,
        status: AgentRunStatus,
        plan: &AgentPlan,
        tool_calls: &[AgentToolCall],
        tool_results: &[AgentToolResult],
        decisions: &[AgentDecision],
        decision_mode: &str,
        retries: u32,
        warnings: &[String],
        input_tokens: i64,
        output_tokens: i64,
        estimated_cost: f64,
        message: &str,
    ) -> Result<()> {
        let completed_steps: Vec<_> = plan
            .steps
            .iter()
            .filter(|step| step.status == AgentStepStatus::Completed)
            .map(|step| step.step_id.clone())
            .collect();
        let pending_steps: Vec<_> = plan
            .steps
            .iter()
            .filter(|step| step.status != AgentStepStatus::Completed)
            .map(|step| step.step_id.clone())
            .collect();
        let completed_count = completed_steps.len();
        let progress = if plan.steps.is_empty() {
            0.0
        } else {
            completed_count as f32 / plan.steps.len() as f32
        };
        let next_step = plan
            .steps
            .iter()
            .position(|step| step.status != AgentStepStatus::Completed)
            .unwrap_or(plan.steps.len());
        sqlx::query("UPDATE agent_tasks SET status = ?, current_step = ?, progress = ?, message = ?, plan_json = ?, completed_steps_json = ?, pending_steps_json = ?, tool_calls_json = ?, tool_results_json = ?, decisions_json = ?, decision_mode = ?, retries = ?, warnings_json = ?, input_tokens = ?, output_tokens = ?, estimated_cost_usd = ?, checkpoint_json = ?, updated_at = ?, revision = revision + 1 WHERE id = ?")
            .bind(status.as_str())
            .bind(completed_count as i64)
            .bind(progress)
            .bind(message)
            .bind(serde_json::to_string(plan)?)
            .bind(serde_json::to_string(&completed_steps)?)
            .bind(serde_json::to_string(&pending_steps)?)
            .bind(serde_json::to_string(tool_calls)?)
            .bind(serde_json::to_string(tool_results)?)
            .bind(serde_json::to_string(decisions)?)
            .bind(decision_mode)
            .bind(i64::from(retries))
            .bind(serde_json::to_string(warnings)?)
            .bind(input_tokens)
            .bind(output_tokens)
            .bind(estimated_cost)
            .bind(serde_json::to_string(&serde_json::json!({ "next_step": next_step }))?)
            .bind(now())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn resolve_context(&self, task: &AgentTask) -> Result<AgentExecutionContext> {
        if task.use_demo {
            let payload = demo::demo_payload();
            return Ok(if task.scenario == "personal_exploration" {
                AgentExecutionContext::Personal(payload.personal, "DEMO".into())
            } else {
                AgentExecutionContext::Friend(
                    payload.personal.clone(),
                    payload.personal,
                    payload.comparison,
                    "DEMO".into(),
                )
            });
        }
        let analyses = self.analyses.read().await;
        if task.scenario == "personal_exploration" {
            let analysis = analyses
                .get(task.analysis_id.as_deref().unwrap_or_default())
                .cloned()
                .context("绑定的真实分析已不可用")?;
            return Ok(AgentExecutionContext::Personal(
                analysis,
                task.data_state.clone(),
            ));
        }
        let a = analyses
            .get(task.analysis_a_id.as_deref().unwrap_or_default())
            .cloned()
            .context("绑定的 Analysis A 已不可用")?;
        let b = analyses
            .get(task.analysis_b_id.as_deref().unwrap_or_default())
            .cloned()
            .context("绑定的 Analysis B 已不可用")?;
        let comparison = engine::compare_analyses(&a, &b);
        Ok(AgentExecutionContext::Friend(
            a,
            b,
            comparison,
            task.data_state.clone(),
        ))
    }

    async fn execute_agent_tool(
        &self,
        call: &AgentToolCall,
        context: &AgentExecutionContext,
    ) -> Result<serde_json::Value> {
        #[cfg(test)]
        {
            let mut failures = self.transient_failures.write().await;
            if let Some(remaining) = failures.get_mut(&call.tool)
                && *remaining > 0
            {
                *remaining -= 1;
                bail!("模拟可恢复的临时工具失败");
            }
        }
        sleep(Duration::from_millis(35)).await;
        let output = match context {
            AgentExecutionContext::Personal(analysis, data_state) => {
                let seeds = select_recommendation_seeds(&analysis.playlist, &analysis.report);
                match call.tool.as_str() {
                    "validate_input_source" | "select_provider" => serde_json::json!({
                        "data_state": data_state,
                        "analysis_id": analysis.analysis_id,
                        "is_demo": analysis.report.is_demo,
                        "source": analysis.report.source_label,
                        "provider": analysis.recommendation_summary.source_label,
                    }),
                    "parse_playlist" => serde_json::json!({
                        "parsed_tracks": analysis.playlist.tracks.len(),
                        "source": analysis.playlist.source,
                        "data_state": data_state,
                    }),
                    "validate_metadata" => serde_json::json!({
                        "track_count": analysis.report.track_count,
                        "genre_matched_count": analysis.report.genre_matched_count,
                        "energy_matched_count": analysis.report.energy_matched_count,
                        "unmatched_count": analysis.unmatched_tracks.len(),
                    }),
                    "normalize_tracks" => serde_json::json!({
                        "normalized_tracks": analysis.playlist.tracks.len(),
                        "version_semantics_preserved": true,
                    }),
                    "analyze_playlist" => {
                        let report = engine::analyze_playlist(&analysis.playlist);
                        serde_json::json!({
                            "track_count": report.track_count,
                            "core_preferences": report.core_preferences,
                            "genre_coverage": report.genre_coverage,
                            "energy_coverage": report.energy_coverage,
                        })
                    }
                    "select_seeds" => serde_json::json!({
                        "seed_count": seeds.len(),
                        "seeds": seeds,
                    }),
                    "fetch_lastfm_candidates" => serde_json::json!({
                        "raw_candidate_count": analysis.recommendation_summary.query_stats.raw_candidate_count,
                        "after_source_exclusion_count": analysis.recommendation_summary.query_stats.after_source_exclusion_count,
                        "successful_seed_count": analysis.recommendation_summary.query_stats.successful_seed_count,
                        "surprise_pool_count": analysis.recommendation_summary.surprise_pool.len(),
                        "tag_layer2_candidate_count": analysis.recommendation_summary.query_stats.tag_layer2_candidate_count,
                        "genre_bridge_candidate_count": analysis.recommendation_summary.query_stats.genre_bridge_candidate_count,
                        "second_hop_artist_candidate_count": analysis.recommendation_summary.query_stats.second_hop_artist_candidate_count,
                        "provider": analysis.recommendation_summary.source_label,
                        "status": analysis.recommendation_summary.status,
                    }),
                    "genre_bridge_candidates" => serde_json::json!({
                        "candidate_count": analysis.recommendation_summary.surprise_pool.len(),
                        "source": "real Last.fm tag.getTopTracks via Genre Graph bridge",
                        "used_demo": false,
                    }),
                    "second_hop_artist_candidates" => serde_json::json!({
                        "candidate_count": analysis.recommendation_summary.query_stats.second_hop_artist_candidate_count,
                        "source": "real Last.fm artist.getSimilar second hop → artist.getTopTracks",
                        "used_demo": false,
                    }),
                    "rank_recommendations" => serde_json::json!({
                        "recommendation_count": analysis.recommendations.len(),
                        "comfort_pool_count": analysis.recommendation_summary.comfort_pool.len(),
                        "expansion_pool_count": analysis.recommendation_summary.expansion_pool.len(),
                        "surprise_pool_count": analysis.recommendation_summary.surprise_pool.len(),
                    }),
                    "validate_recommendations" => serde_json::json!({
                        "validated": analysis.recommendations.iter().all(|item| !item.already_in_source_playlist),
                        "source_playlist_duplicates": analysis.recommendations.iter().filter(|item| item.already_in_source_playlist).count(),
                    }),
                    "build_route" => serde_json::json!({
                        "route": build_route(&analysis.report, &analysis.recommendations),
                    }),
                    "prepare_explanation" => serde_json::json!({
                        "summary": analysis.report.summary,
                        "recommendation_message": analysis.recommendation_summary.message,
                    }),
                    "export_report" => serde_json::json!({
                        "report_ready": true,
                        "analysis_id": analysis.analysis_id,
                        "data_state": data_state,
                    }),
                    _ => bail!("工具 {} 不属于 personal_exploration 白名单", call.tool),
                }
            }
            AgentExecutionContext::Friend(a, b, comparison, data_state) => match call.tool.as_str()
            {
                "validate_compare_inputs" => serde_json::json!({
                    "analysis_a_id": a.analysis_id,
                    "analysis_b_id": b.analysis_id,
                    "data_state": data_state,
                    "is_demo": comparison.is_demo,
                }),
                "parse_playlist_a" => {
                    serde_json::json!({ "parsed_tracks": a.playlist.tracks.len(), "source": a.playlist.source })
                }
                "parse_playlist_b" => {
                    serde_json::json!({ "parsed_tracks": b.playlist.tracks.len(), "source": b.playlist.source })
                }
                "normalize_tracks" => {
                    serde_json::json!({ "normalized_tracks": a.playlist.tracks.len() + b.playlist.tracks.len() })
                }
                "analyze_playlist_a" => {
                    serde_json::json!({ "track_count": a.report.track_count, "core_preferences": a.report.core_preferences })
                }
                "analyze_playlist_b" => {
                    serde_json::json!({ "track_count": b.report.track_count, "core_preferences": b.report.core_preferences })
                }
                "compare_playlists" | "score_compatibility" => serde_json::json!({
                    "metric_count": comparison.metrics.len(),
                    "shared_track_count": comparison.shared_track_count,
                    "shared_genres": comparison.shared_genres,
                    "shared_artist_count": comparison.shared_artists.len(),
                    "overlap_level": comparison_overlap_level(comparison),
                }),
                "recommend_for_two_people" | "validate_compare_zones" => serde_json::json!({
                    "bridge_count": comparison.bridge_playlist.len(),
                    "recommendations": comparison.bridge_playlist,
                }),
                "shared_affinity_strategy" => serde_json::json!({
                    "strategy": "shared_affinity",
                    "overlap_level": comparison_overlap_level(comparison),
                    "bridge_count": comparison.bridge_playlist.len(),
                    "recommendations": comparison.bridge_playlist,
                    "reason": "双方存在可验证的共同歌曲、艺人或 Genre，优先解释共同偏好",
                }),
                "complementary_bridge_strategy" => serde_json::json!({
                    "strategy": "complementary_bridge",
                    "overlap_level": comparison_overlap_level(comparison),
                    "bridge_count": comparison.bridge_playlist.len(),
                    "recommendations": comparison.bridge_playlist,
                    "reason": "直接重合较低，使用 Rust 比较结果中的可解释互补桥梁",
                }),
                "validate_recommendations" => serde_json::json!({
                    "validated": comparison.bridge_playlist.iter().all(|item| !item.already_in_a && !item.already_in_b),
                }),
                "prepare_explanation" => serde_json::json!({ "summary": comparison.summary }),
                "export_report" => {
                    serde_json::json!({ "report_ready": true, "data_state": data_state })
                }
                _ => bail!("工具 {} 不属于 friend_bridge 白名单", call.tool),
            },
        };
        Ok(output)
    }

    #[cfg(test)]
    async fn set_transient_failures(&self, tool: &str, count: u32) {
        self.transient_failures
            .write()
            .await
            .insert(tool.to_string(), count);
    }

    pub async fn resume(&self, id: &str) -> Result<AgentTask> {
        if self.cancellations.read().await.contains_key(id) {
            bail!("任务仍在结束中，请稍后再恢复");
        }
        let task = self.get_task(id).await?.context("任务不存在")?;
        if !matches!(task.status.as_str(), "FAILED" | "CANCELLED") {
            bail!("只有 FAILED 或 CANCELLED 任务可以从检查点恢复");
        }
        let mut plan: AgentPlan =
            serde_json::from_str(&task.plan_json).unwrap_or_else(|_| build_plan(&task.scenario));
        for step in &mut plan.steps {
            if step.status != AgentStepStatus::Completed {
                step.status = AgentStepStatus::Pending;
                step.last_error = None;
            }
        }
        sqlx::query("UPDATE agent_tasks SET status = 'PLANNING', message = '正在从检查点恢复', error = NULL, plan_json = ?, updated_at = ?, revision = revision + 1 WHERE id = ?")
            .bind(serde_json::to_string(&plan)?)
            .bind(now())
            .bind(id)
            .execute(&self.pool)
            .await?;
        let cancelled = Arc::new(AtomicBool::new(false));
        self.cancellations
            .write()
            .await
            .insert(id.to_string(), cancelled.clone());
        let service = self.clone();
        let task_id = id.to_string();
        let scenario = task.scenario;
        tokio::spawn(async move {
            if let Err(error) = service.run_loop(&task_id, &scenario, cancelled).await {
                let _ = service.fail_task(&task_id, &error.to_string()).await;
            }
            service.cancellations.write().await.remove(&task_id);
        });
        self.get_task(id).await?.context("任务不存在")
    }

    async fn fail_task(&self, id: &str, error: &str) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET status = 'FAILED', message = 'Agent 执行失败；可从检查点恢复', error = ?, updated_at = ?, revision = revision + 1 WHERE id = ? AND status NOT IN ('COMPLETED', 'CANCELLED')")
            .bind(error).bind(now()).bind(id).execute(&self.pool).await?;
        Ok(())
    }

    async fn cancelled_task(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE agent_tasks SET status = 'CANCELLED', message = '用户已中断任务；检查点已保留', updated_at = ?, revision = revision + 1 WHERE id = ?")
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

fn normalize_intent(scenario: &str, goal: &str) -> AgentIntent {
    let count = goal
        .split(|character: char| !character.is_ascii_digit())
        .find_map(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0);
    let novelty = if goal.contains("冒险") || goal.to_lowercase().contains("adventure") {
        Some("adventurous".into())
    } else if goal.contains("保守") || goal.contains("熟悉") {
        Some("safe".into())
    } else {
        Some("balanced".into())
    };
    AgentIntent {
        action: match scenario {
            "friend_bridge" => "compare_and_recommend",
            _ => "analyze_and_discover",
        }
        .into(),
        count,
        novelty,
    }
}

fn build_plan(scenario: &str) -> AgentPlan {
    let definitions: &[(&str, &str)] = if scenario == "personal_exploration" {
        &[
            ("识别输入来源", "validate_input_source"),
            ("选择明确的数据 Provider", "select_provider"),
            ("读取歌单", "parse_playlist"),
            ("检查元数据完整度", "validate_metadata"),
            ("标准化歌曲、艺人和版本", "normalize_tracks"),
            ("构建音乐品味画像", "analyze_playlist"),
            ("选择真实代表种子", "select_seeds"),
            ("获取可解释候选", "fetch_lastfm_candidates"),
            ("计算相关性与新颖性", "rank_recommendations"),
            ("验证去重与艺人上限", "validate_recommendations"),
            ("生成可选解释增强", "prepare_explanation"),
            ("构建探索路线", "build_route"),
            ("保存结果和执行记录", "export_report"),
        ]
    } else {
        &[
            ("识别双方输入", "validate_compare_inputs"),
            ("读取歌单 A", "parse_playlist_a"),
            ("读取歌单 B", "parse_playlist_b"),
            ("统一 Track 模型", "normalize_tracks"),
            ("分析歌单 A", "analyze_playlist_a"),
            ("分析歌单 B", "analyze_playlist_b"),
            ("计算共同歌曲与艺人", "compare_playlists"),
            ("计算 Genre、Tag 与互补度", "score_compatibility"),
            ("排除两份源歌单", "validate_recommendations"),
            ("验证三类共同探索区域", "validate_compare_zones"),
            ("生成可解释比较摘要", "prepare_explanation"),
            ("保存比较与执行记录", "export_report"),
        ]
    };
    AgentPlan {
        scenario: scenario.into(),
        steps: definitions
            .iter()
            .enumerate()
            .map(|(index, (label, tool))| AgentStep {
                step_id: format!("step_{:02}", index + 1),
                label: (*label).into(),
                tool: (*tool).into(),
                status: AgentStepStatus::Pending,
                attempts: 0,
                last_error: None,
            })
            .collect(),
    }
}

fn validate_tool_output(output: &serde_json::Value) -> Result<()> {
    if output.is_null() {
        bail!("工具返回空结果");
    }
    Ok(())
}

struct LlmDecisionUsage {
    decision: AgentDecision,
    input_tokens: i64,
    output_tokens: i64,
    cost: f64,
}

#[derive(Clone)]
enum AgentExecutionContext {
    Personal(PersonalDemo, String),
    Friend(PersonalDemo, PersonalDemo, ComparisonReport, String),
}

impl AgentExecutionContext {
    fn deterministic_result(&self) -> Result<serde_json::Value> {
        match self {
            Self::Personal(analysis, _) => Ok(serde_json::to_value(analysis)?),
            Self::Friend(_, _, comparison, _) => Ok(serde_json::to_value(comparison)?),
        }
    }
}

fn analysis_data_state(analysis: &PersonalDemo) -> String {
    if analysis.report.is_demo || analysis.playlist.is_demo {
        return "DEMO".into();
    }
    match analysis
        .import_summary
        .as_ref()
        .map(|summary| &summary.data_state)
    {
        Some(crate::models::DataState::RealText) => "REAL_TEXT".into(),
        Some(crate::models::DataState::RealFile) => "REAL_FILE".into(),
        Some(crate::models::DataState::RealAccount) => "REAL_ACCOUNT".into(),
        Some(crate::models::DataState::RealPublicLink) => "REAL_PUBLIC_LINK".into(),
        _ if analysis.playlist.source.contains("文本")
            || analysis.playlist.source.eq_ignore_ascii_case("manual") =>
        {
            "REAL_TEXT".into()
        }
        _ => "REAL_FILE".into(),
    }
}

fn parse_agent_decision(raw: &str) -> Result<AgentDecision> {
    let trimmed = raw.trim();
    let json = trimmed
        .strip_prefix("```json")
        .or_else(|| trimmed.strip_prefix("```"))
        .unwrap_or(trimmed)
        .strip_suffix("```")
        .unwrap_or(trimmed)
        .trim();
    let decision: AgentDecision =
        serde_json::from_str(json).context("LLM decision 不符合 AgentDecision JSON schema")?;
    if decision.action.trim().is_empty() || decision.reason.trim().is_empty() {
        bail!("LLM decision 缺少 action 或可核验 reason");
    }
    Ok(decision)
}

fn tool_registry() -> HashSet<&'static str> {
    [
        "validate_input_source",
        "select_provider",
        "parse_playlist",
        "validate_metadata",
        "normalize_tracks",
        "analyze_playlist",
        "select_seeds",
        "fetch_lastfm_candidates",
        "genre_bridge_candidates",
        "second_hop_artist_candidates",
        "rank_recommendations",
        "validate_recommendations",
        "prepare_explanation",
        "build_route",
        "export_report",
        "validate_compare_inputs",
        "parse_playlist_a",
        "parse_playlist_b",
        "analyze_playlist_a",
        "analyze_playlist_b",
        "compare_playlists",
        "score_compatibility",
        "recommend_for_two_people",
        "shared_affinity_strategy",
        "complementary_bridge_strategy",
        "validate_compare_zones",
    ]
    .into_iter()
    .collect()
}

fn validate_agent_decision(
    decision: &AgentDecision,
    allowed_tools: &[String],
    all_complete: bool,
) -> Result<()> {
    if all_complete {
        if !decision.finish || decision.next_tool.is_some() {
            bail!("所有工具完成后 decision 必须 finish=true 且 next_tool=null");
        }
        return Ok(());
    }
    if decision.finish {
        bail!("仍有必需工具未完成，拒绝提前 finish");
    }
    let tool = decision
        .next_tool
        .as_deref()
        .context("decision.next_tool 不能为空")?;
    if !tool_registry().contains(tool) {
        bail!("工具 {tool} 不在 Tool Registry 白名单");
    }
    if !allowed_tools.iter().any(|allowed| allowed == tool) {
        bail!("工具 {tool} 当前不可执行");
    }
    Ok(())
}

fn fallback_decision(
    scenario: &str,
    allowed_tools: &[String],
    tool_results: &[AgentToolResult],
    all_complete: bool,
) -> AgentDecision {
    if all_complete {
        return AgentDecision {
            action: "finish".into(),
            next_tool: None,
            arguments: serde_json::json!({}),
            reason: "所有白名单 Rust 工具均已返回并通过校验".into(),
            finish: true,
        };
    }
    let is_recommendation_recovery = allowed_tools.iter().any(|tool| {
        matches!(
            tool.as_str(),
            "genre_bridge_candidates" | "second_hop_artist_candidates"
        )
    });
    let genre_bridge_count = latest_u64(tool_results, "genre_bridge_candidate_count");
    let tag_layer2_count = latest_u64(tool_results, "tag_layer2_candidate_count");
    let next_tool = if is_recommendation_recovery {
        if genre_bridge_count > 0 || tag_layer2_count > 0 {
            "genre_bridge_candidates".to_string()
        } else {
            "second_hop_artist_candidates".to_string()
        }
    } else if scenario == "friend_bridge"
        && allowed_tools
            .iter()
            .any(|tool| tool == "shared_affinity_strategy")
    {
        let overlap = tool_results.iter().rev().find_map(|result| {
            result
                .output
                .get("overlap_level")
                .and_then(serde_json::Value::as_str)
        });
        if overlap == Some("high") {
            "shared_affinity_strategy".to_string()
        } else {
            "complementary_bridge_strategy".to_string()
        }
    } else {
        allowed_tools.first().cloned().unwrap_or_default()
    };
    let replanned = is_recommendation_recovery
        || matches!(
            next_tool.as_str(),
            "shared_affinity_strategy" | "complementary_bridge_strategy"
        );
    AgentDecision {
        action: if replanned { "replan" } else { "continue" }.into(),
        next_tool: Some(next_tool),
        arguments: serde_json::json!({}),
        reason: if is_recommendation_recovery {
            "惊喜候选不足；根据真实标签层级和第二跳艺人计数选择恢复工具"
        } else if replanned {
            "根据双方真实重合度选择共同偏好或互补桥梁策略"
        } else {
            "按依赖顺序执行下一项确定性 Rust 工具"
        }
        .into(),
        finish: false,
    }
}

fn latest_u64(tool_results: &[AgentToolResult], key: &str) -> u64 {
    tool_results
        .iter()
        .rev()
        .find_map(|result| result.output.get(key).and_then(serde_json::Value::as_u64))
        .unwrap_or_default()
}

fn currently_allowed_tools(plan: &AgentPlan) -> Vec<String> {
    let pending: Vec<_> = plan
        .steps
        .iter()
        .filter(|step| step.status == AgentStepStatus::Pending)
        .map(|step| step.tool.clone())
        .collect();
    let Some(first) = pending.first().cloned() else {
        return Vec::new();
    };
    let alternative_pair = match first.as_str() {
        "genre_bridge_candidates" | "second_hop_artist_candidates" => {
            Some(["genre_bridge_candidates", "second_hop_artist_candidates"])
        }
        "shared_affinity_strategy" | "complementary_bridge_strategy" => {
            Some(["shared_affinity_strategy", "complementary_bridge_strategy"])
        }
        _ => None,
    };
    alternative_pair.map_or_else(
        || vec![first],
        |pair| {
            pending
                .into_iter()
                .filter(|tool| pair.contains(&tool.as_str()))
                .collect()
        },
    )
}

fn prune_unselected_strategy(plan: &mut AgentPlan, selected: &str) {
    let other = match selected {
        "genre_bridge_candidates" => Some("second_hop_artist_candidates"),
        "second_hop_artist_candidates" => Some("genre_bridge_candidates"),
        "shared_affinity_strategy" => Some("complementary_bridge_strategy"),
        "complementary_bridge_strategy" => Some("shared_affinity_strategy"),
        _ => None,
    };
    if let Some(other) = other {
        plan.steps
            .retain(|step| step.tool != other || step.status != AgentStepStatus::Pending);
    }
}

fn comparison_overlap_level(comparison: &ComparisonReport) -> &'static str {
    if comparison.shared_track_count > 0
        || comparison.shared_artists.len() + comparison.shared_genres.len() >= 2
    {
        "high"
    } else {
        "low"
    }
}

fn tool_result_summary(tool: &str, output: &serde_json::Value) -> String {
    match tool {
        "parse_playlist" | "parse_playlist_a" | "parse_playlist_b" => format!(
            "解析 {} 首真实歌曲",
            output
                .get("parsed_tracks")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default()
        ),
        "select_seeds" => format!(
            "选择 {} 首真实种子",
            output
                .get("seed_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default()
        ),
        "fetch_lastfm_candidates" => format!(
            "Last.fm 返回 {} 首原始候选",
            output
                .get("raw_candidate_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default()
        ),
        "rank_recommendations" => format!(
            "Rust 评分得到 {} 首首屏推荐",
            output
                .get("recommendation_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default()
        ),
        "compare_playlists" => format!(
            "比较得到 {} 首共同歌曲",
            output
                .get("shared_track_count")
                .and_then(serde_json::Value::as_u64)
                .unwrap_or_default()
        ),
        _ => format!("{tool} 返回结构化结果"),
    }
}

fn validate_llm_cost_budget(
    settings: &AgentSettings,
    used_cost: f64,
    estimated_input_tokens: i64,
    requested_output_tokens: i64,
) -> Result<()> {
    let maximum_cost = used_cost
        + estimated_input_tokens as f64 / 1_000_000.0 * settings.input_price_per_million
        + requested_output_tokens as f64 / 1_000_000.0 * settings.output_price_per_million;
    if maximum_cost > settings.max_cost_usd {
        bail!("LLM 预计费用超过预算上限 ${:.4}", settings.max_cost_usd);
    }
    Ok(())
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
    use crate::{
        engine,
        models::{Playlist, RecommendationSummary, Track, VersionType},
    };

    async fn wait_for_status(service: &AgentService, id: &str, expected: &[&str]) -> AgentTask {
        for _ in 0..160 {
            let current = service.get_task(id).await.unwrap().unwrap();
            if expected.contains(&current.status.as_str()) {
                return current;
            }
            sleep(Duration::from_millis(30)).await;
        }
        service.get_task(id).await.unwrap().unwrap()
    }

    #[tokio::test]
    async fn task_history_and_completion_are_persisted() {
        let service = AgentService::in_memory().await.unwrap();
        let task = service
            .create_task(CreateAgentTaskRequest {
                scenario: "personal_exploration".into(),
                goal: None,
                analysis_id: None,
                analysis_a_id: None,
                analysis_b_id: None,
                transfer_preview_id: None,
                use_demo: true,
            })
            .await
            .unwrap();
        let completed = wait_for_status(&service, &task.id, &["COMPLETED"]).await;
        assert_eq!(completed.status, "COMPLETED");
        assert!(completed.result_json.is_some());
        let plan: AgentPlan = serde_json::from_str(&completed.plan_json).unwrap();
        assert!(
            plan.steps
                .iter()
                .all(|step| step.status == AgentStepStatus::Completed)
        );
        let calls: Vec<AgentToolCall> = serde_json::from_str(&completed.tool_calls_json).unwrap();
        let results: Vec<AgentToolResult> =
            serde_json::from_str(&completed.tool_results_json).unwrap();
        assert_eq!(calls.len(), plan.steps.len());
        assert!(results.iter().all(|result| result.success));
        assert_eq!(service.list_tasks().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn task_can_be_cancelled() {
        let service = AgentService::in_memory().await.unwrap();
        let task = service
            .create_task(CreateAgentTaskRequest {
                scenario: "friend_bridge".into(),
                goal: None,
                analysis_id: None,
                analysis_a_id: None,
                analysis_b_id: None,
                transfer_preview_id: None,
                use_demo: true,
            })
            .await
            .unwrap();
        let cancelled = service.cancel(&task.id).await.unwrap();
        assert_eq!(cancelled.status, "CANCELLED");
    }

    #[tokio::test]
    async fn transient_tool_failure_is_retried_and_recorded() {
        let service = AgentService::in_memory().await.unwrap();
        service.set_transient_failures("parse_playlist", 1).await;
        let task = service
            .create_task(CreateAgentTaskRequest {
                scenario: "personal_exploration".into(),
                goal: Some("推荐 10 首平衡探索歌曲".into()),
                analysis_id: None,
                analysis_a_id: None,
                analysis_b_id: None,
                transfer_preview_id: None,
                use_demo: true,
            })
            .await
            .unwrap();
        let completed = wait_for_status(&service, &task.id, &["COMPLETED"]).await;
        assert_eq!(completed.status, "COMPLETED");
        assert_eq!(completed.retries, 1);
        let results: Vec<AgentToolResult> =
            serde_json::from_str(&completed.tool_results_json).unwrap();
        assert!(
            results
                .iter()
                .any(|result| !result.success && result.recoverable)
        );
        assert!(results.iter().any(|result| result.success));
    }

    #[tokio::test]
    async fn cancelled_task_can_resume_from_completed_checkpoint() {
        let service = AgentService::in_memory().await.unwrap();
        let task = service
            .create_task(CreateAgentTaskRequest {
                scenario: "friend_bridge".into(),
                goal: None,
                analysis_id: None,
                analysis_a_id: None,
                analysis_b_id: None,
                transfer_preview_id: None,
                use_demo: true,
            })
            .await
            .unwrap();
        sleep(Duration::from_millis(150)).await;
        let before_cancel = service.get_task(&task.id).await.unwrap().unwrap();
        service.cancel(&task.id).await.unwrap();
        let cancelled = wait_for_status(&service, &task.id, &["CANCELLED"]).await;
        assert!(cancelled.current_step >= before_cancel.current_step);
        for _ in 0..80 {
            if !service.cancellations.read().await.contains_key(&task.id) {
                break;
            }
            sleep(Duration::from_millis(20)).await;
        }
        let resumed = service.resume(&task.id).await.unwrap();
        assert_eq!(resumed.status, "PLANNING");
        let completed = wait_for_status(&service, &task.id, &["COMPLETED"]).await;
        assert_eq!(completed.status, "COMPLETED");
        assert_eq!(completed.current_step, 13);
    }

    #[tokio::test]
    async fn max_step_limit_fails_before_tool_execution() {
        let service = AgentService::in_memory().await.unwrap();
        let mut settings = service.settings().await;
        settings.max_agent_steps = 12;
        service.update_settings(settings).await.unwrap();
        let task = service
            .create_task(CreateAgentTaskRequest {
                scenario: "personal_exploration".into(),
                goal: None,
                analysis_id: None,
                analysis_a_id: None,
                analysis_b_id: None,
                transfer_preview_id: None,
                use_demo: true,
            })
            .await
            .unwrap();
        let failed = wait_for_status(&service, &task.id, &["FAILED"]).await;
        assert_eq!(failed.status, "FAILED");
        assert!(failed.error.unwrap_or_default().contains("最大 Agent 步数"));
        let calls: Vec<AgentToolCall> = serde_json::from_str(&failed.tool_calls_json).unwrap();
        assert!(calls.is_empty());
    }

    fn real_analysis(id: &str) -> PersonalDemo {
        let playlist = Playlist {
            id: id.into(),
            name: "Happy Mix bound analysis".into(),
            owner_label: "test".into(),
            source: "真实文件 · Happy_Mix.csv".into(),
            is_demo: false,
            tracks: vec![Track {
                id: "real-track-1".into(),
                title: "Super Shy".into(),
                normalized_title: "super shy".into(),
                artists: vec!["NewJeans".into()],
                album: None,
                genres: vec!["K-Pop".into()],
                release_year: Some(2023),
                language: Some("韩语".into()),
                duration_ms: Some(149_000),
                platform: "csv".into(),
                platform_url: None,
                external_ids: HashMap::new(),
                version_type: VersionType::Original,
                mood_tags: vec!["bright".into()],
                energy_score: Some(0.74),
                popularity: None,
                metadata_confidence: 1.0,
            }],
        };
        let report = engine::analyze_playlist(&playlist);
        let taste_profile = engine::build_taste_profile(playlist.tracks.iter());
        PersonalDemo {
            analysis_id: id.into(),
            playlist,
            report,
            recommendations: Vec::new(),
            route: Vec::new(),
            recommendation_summary: RecommendationSummary {
                source_label: "Mock-free bound analysis fixture".into(),
                status: "no_candidates".into(),
                message: "No candidates".into(),
                candidate_count: 0,
                zones: Vec::new(),
                seeds: Vec::new(),
                query_stats: Default::default(),
                comfort_pool: Vec::new(),
                expansion_pool: Vec::new(),
                surprise_pool: Vec::new(),
            },
            import_summary: None,
            unmatched_tracks: Vec::new(),
            metadata_resolutions: Vec::new(),
            taste_profile,
        }
    }

    #[tokio::test]
    async fn real_agent_task_never_uses_demo_payload_and_matches_bound_analysis() {
        let service = AgentService::in_memory().await.unwrap();
        let analysis = real_analysis("analysis-real-1");
        service
            .analyses
            .write()
            .await
            .insert(analysis.analysis_id.clone(), analysis.clone());
        let task = service
            .create_task(CreateAgentTaskRequest {
                scenario: "personal_exploration".into(),
                goal: Some("分析 Happy Mix 的真实偏好".into()),
                analysis_id: Some(analysis.analysis_id.clone()),
                analysis_a_id: None,
                analysis_b_id: None,
                transfer_preview_id: None,
                use_demo: false,
            })
            .await
            .unwrap();
        let completed = wait_for_status(&service, &task.id, &["COMPLETED"]).await;
        assert_eq!(completed.data_state, "REAL_FILE");
        assert!(!completed.use_demo);
        let results: Vec<AgentToolResult> =
            serde_json::from_str(&completed.tool_results_json).unwrap();
        let parse = results
            .iter()
            .find(|result| result.output.get("parsed_tracks").is_some())
            .unwrap();
        assert_eq!(parse.output["parsed_tracks"], 1);
        assert_ne!(parse.output["source"], "explicit_demo_scenario");
        let output: serde_json::Value =
            serde_json::from_str(completed.result_json.as_deref().unwrap()).unwrap();
        assert_eq!(
            output["deterministic_result"]["analysis_id"],
            "analysis-real-1"
        );
        assert_eq!(output["deterministic_result"]["report"]["track_count"], 1);
        assert_eq!(output["deterministic_result"]["report"]["is_demo"], false);
    }

    #[tokio::test]
    async fn friend_agent_uses_both_bound_real_analyses() {
        let service = AgentService::in_memory().await.unwrap();
        let analysis_a = real_analysis("analysis-friend-a");
        let mut analysis_b = real_analysis("analysis-friend-b");
        analysis_b.playlist.tracks[0].id = "real-track-2".into();
        analysis_b.playlist.tracks[0].title = "Ditto".into();
        analysis_b.playlist.tracks[0].normalized_title = "ditto".into();
        analysis_b.report = engine::analyze_playlist(&analysis_b.playlist);
        {
            let mut analyses = service.analyses.write().await;
            analyses.insert(analysis_a.analysis_id.clone(), analysis_a.clone());
            analyses.insert(analysis_b.analysis_id.clone(), analysis_b.clone());
        }
        let task = service
            .create_task(CreateAgentTaskRequest {
                scenario: "friend_bridge".into(),
                goal: Some("比较两份真实歌单并生成共同桥梁".into()),
                analysis_id: None,
                analysis_a_id: Some(analysis_a.analysis_id.clone()),
                analysis_b_id: Some(analysis_b.analysis_id.clone()),
                transfer_preview_id: None,
                use_demo: false,
            })
            .await
            .unwrap();
        let completed = wait_for_status(&service, &task.id, &["COMPLETED"]).await;
        assert!(!completed.use_demo);
        assert_eq!(completed.data_state, "REAL_FILE");
        let results: Vec<AgentToolResult> =
            serde_json::from_str(&completed.tool_results_json).unwrap();
        let input = results
            .iter()
            .find(|result| result.output.get("analysis_a_id").is_some())
            .unwrap();
        assert_eq!(input.output["analysis_a_id"], "analysis-friend-a");
        assert_eq!(input.output["analysis_b_id"], "analysis-friend-b");
        assert_eq!(input.output["is_demo"], false);
        assert!(
            results
                .iter()
                .filter_map(|result| result.output.get("source"))
                .all(|source| source != "explicit_demo_scenario")
        );
        let output: serde_json::Value =
            serde_json::from_str(completed.result_json.as_deref().unwrap()).unwrap();
        assert_eq!(output["deterministic_result"]["is_demo"], false);
    }

    #[test]
    fn structured_llm_decision_schema_is_validated_and_invalid_tool_is_rejected() {
        let valid = parse_agent_decision(r#"{"action":"continue","next_tool":"parse_playlist","arguments":{},"reason":"读取已绑定歌单","finish":false}"#).unwrap();
        validate_agent_decision(&valid, &["parse_playlist".into()], false).unwrap();
        let invalid = AgentDecision {
            next_tool: Some("delete_files".into()),
            ..valid
        };
        assert!(validate_agent_decision(&invalid, &["delete_files".into()], false).is_err());
        assert!(parse_agent_decision("not json").is_err());
    }

    #[test]
    fn tool_result_changes_next_fallback_decision() {
        let tools = vec![
            "genre_bridge_candidates".into(),
            "second_hop_artist_candidates".into(),
        ];
        let empty = vec![AgentToolResult {
            call_id: "1".into(),
            success: true,
            recoverable: false,
            summary: "empty".into(),
            output: serde_json::json!({"genre_bridge_candidate_count": 0, "tag_layer2_candidate_count": 0}),
            completed_at: 0,
        }];
        let populated = vec![AgentToolResult {
            call_id: "2".into(),
            success: true,
            recoverable: false,
            summary: "ready".into(),
            output: serde_json::json!({"genre_bridge_candidate_count": 3, "tag_layer2_candidate_count": 3}),
            completed_at: 0,
        }];
        assert_eq!(
            fallback_decision("personal_exploration", &tools, &empty, false)
                .next_tool
                .as_deref(),
            Some("second_hop_artist_candidates")
        );
        assert_eq!(
            fallback_decision("personal_exploration", &tools, &populated, false)
                .next_tool
                .as_deref(),
            Some("genre_bridge_candidates")
        );
    }

    #[test]
    fn friend_overlap_changes_next_strategy() {
        let tools = vec![
            "shared_affinity_strategy".into(),
            "complementary_bridge_strategy".into(),
        ];
        let result = |level: &str| {
            vec![AgentToolResult {
                call_id: level.into(),
                success: true,
                recoverable: false,
                summary: level.into(),
                output: serde_json::json!({"overlap_level": level}),
                completed_at: 0,
            }]
        };
        assert_eq!(
            fallback_decision("friend_bridge", &tools, &result("high"), false)
                .next_tool
                .as_deref(),
            Some("shared_affinity_strategy")
        );
        assert_eq!(
            fallback_decision("friend_bridge", &tools, &result("low"), false)
                .next_tool
                .as_deref(),
            Some("complementary_bridge_strategy")
        );
    }

    #[test]
    fn only_dependency_frontier_is_available_to_the_model() {
        let mut plan = build_plan("personal_exploration");
        assert_eq!(
            currently_allowed_tools(&plan),
            vec!["validate_input_source"]
        );
        plan.steps[0].status = AgentStepStatus::Completed;
        assert_eq!(currently_allowed_tools(&plan), vec!["select_provider"]);
    }

    #[test]
    fn cost_limit_protection_stops_before_call() {
        let settings = AgentSettings {
            max_cost_usd: 0.001,
            input_price_per_million: 10.0,
            output_price_per_million: 20.0,
            ..Default::default()
        };
        assert!(validate_llm_cost_budget(&settings, 0.0, 10_000, 1_000).is_err());
    }

    #[tokio::test]
    async fn llm_unavailable_is_explicit_deterministic_fallback() {
        let service = AgentService::in_memory().await.unwrap();
        let task = service
            .create_task(CreateAgentTaskRequest {
                scenario: "personal_exploration".into(),
                goal: None,
                analysis_id: None,
                analysis_a_id: None,
                analysis_b_id: None,
                transfer_preview_id: None,
                use_demo: true,
            })
            .await
            .unwrap();
        let completed = wait_for_status(&service, &task.id, &["COMPLETED"]).await;
        assert_eq!(completed.decision_mode, "DETERMINISTIC_FALLBACK");
        let decisions: Vec<AgentDecision> =
            serde_json::from_str(&completed.decisions_json).unwrap();
        assert!(!decisions.is_empty());
    }
}
