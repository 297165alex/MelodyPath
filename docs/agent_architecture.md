# MelodyPath Agent 架构

更新时间：2026-09-05

## 设计目标

MelodyPath 的 Agent 是 Rust 后端中的可审计任务编排器，不是把歌单交给 LLM 后等待一段自然语言。确定性工具负责导入、规范化、分析、Provider 调用、候选评分、约束验证和导出；可选模型负责把自然语言目标整理成结构化意图、从白名单中选择下一工具，并在每个 `ToolResult` 返回后决定继续、重规划或结束。模型失败时会明确切换为确定性 fallback，不会改变基础结果或切换到 Demo。

## 核心模型

`backend/src/agent.rs` 与 `backend/src/models.rs` 定义并持久化：

- `AgentGoal`：用户希望完成的音乐任务。
- `AgentIntent`：规范化后的结构化意图。
- `AgentPlan` / `AgentStep`：有限、可显示的执行计划。
- `AgentToolCall` / `AgentToolResult`：工具输入摘要、尝试次数、状态和结果。
- `AgentDecision`：经过 serde schema 校验的 `action`、`next_tool`、`arguments`、`reason` 与 `finish`。
- `AgentState`：当前步骤、已完成/待执行步骤、重试、警告和检查点。
- `AgentRun`：一次可查询、取消或恢复的任务记录。

真实任务必须绑定现有分析：个人探索使用 `analysis_id`，好友桥梁使用 `analysis_a_id` 与 `analysis_b_id`。只有显式设置 `use_demo=true` 才能读取 `demo::demo_payload()`；所有输出同时携带 `REAL_FILE`、`REAL_TEXT` 或 `DEMO` 数据状态。

对外状态统一为：

`PLANNING → RUNNING → WAITING_USER_CONFIRMATION | BLOCKED_EXTERNAL_AUTH | COMPLETED | FAILED | CANCELLED`

页面只显示计划、工具、结果、错误和检查点，不显示或编造隐藏思维过程。

## 工具执行原则

个人 Genre 探索计划覆盖输入来源验证、Provider 选择、导入、元数据检查、规范化、画像、种子、真实 Last.fm 候选、Rust 评分、约束验证、解释准备、路线和报告。双歌单比较使用绑定的两份真实分析执行 `compare_analyses` 并生成共同桥梁。

每步遵守：

1. Tool Registry 是白名单；模型不能执行 shell、删除文件或调用未注册工具。
2. 每个结果经过类型和业务校验后以紧凑结构重新提供给模型；Spotify 账号数据绑定的任务强制使用确定性执行，不进入 LLM。
3. 临时错误只做有上限的重试；超时、最大步骤和费用边界有明确终态。
4. Provider/OAuth 缺失写成阻塞或降级说明，不得伪造调用成功。
5. 取消会保存已完成步骤；恢复从检查点继续，不重复完成步骤。

## 持久化与 API

SQLite 只保存任务领域状态：结构化意图、计划、已完成和待执行步骤、调用/结果、重试、警告和检查点。OAuth token、API Key 与 Client Secret 不写入 SQLite。

主要接口：

- `POST /api/tasks`：创建任务。
- `GET /api/tasks`、`GET /api/tasks/{id}`：列表和详情。
- `GET /api/tasks/{id}/events`：SSE 进度。
- `POST /api/tasks/{id}/cancel`：取消。
- `POST /api/tasks/{id}/resume`：从检查点恢复。

## 与产品流程的关系

- 本地分析、推荐和 Compare 可作为 Agent 的确定性工具，但仍保留各自独立用户流程。
- `/transfer` 是独立的高价值 Agent 执行场景；Copy 执行使用独立 run id，提供状态查询、SSE、取消与恢复，并逐首隔离错误。run 登记当前仍是进程内状态；不得把 Agent 页面 `COMPLETED` 等同于真实 OAuth Copy 成功。
- Playlist Writer 和 Transfer 的写入前确认属于安全闸门，Agent 不得自动越过。

## 验证状态与限制

- 自动测试覆盖真实分析绑定不读 Demo、两份分析绑定、结构化 decision schema、非法工具拒绝、ToolResult 改变下一决策、最大步骤、费用限制和模型失败时的明确 fallback。
- 本地浏览器已用 Happy_Mix.csv 的 50 首真实输入运行个人探索 Agent；页面数据状态为 `REAL_FILE`，13 个白名单工具完成且没有 Demo 来源。好友场景以两份真实文本运行，按高重合结果选择了 `shared_affinity_strategy`。
- 当前环境没有可验证的 LLM Controller 配置，所以上述浏览器运行明确标记为 `DETERMINISTIC_FALLBACK`，不宣称真实 LLM 调用已经通过。

## 动态决策前沿

LLM 只能看到当前依赖前沿，不会跳过尚未完成的 Rust 工具。惊喜候选不足时，前沿同时提供 `genre_bridge_candidates` 与 `second_hop_artist_candidates`；双方比较完成后，前沿同时提供 `shared_affinity_strategy` 与 `complementary_bridge_strategy`。选择后未采用的互斥分支会从计划中移除。不同 ToolResult 导致不同 `next_tool` 的单元测试已覆盖。
- Spotify 与 YouTube OAuth 仍未配置；Version Radar 的账号扫描和跨平台复制属于 `BLOCKED BY OAUTH`，不得用 Mock 结果冒充真实写入。
