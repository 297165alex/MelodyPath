# MelodyPath 答辩架构说明

本页解释当前代码，能力依据为 2026-09-08 的 README 与 [平台审计](platform_capability_audit.md)。历史验收不等于本次重新登录验证。

## 1. 整体架构

```text
Frontend · React / TypeScript
    ↓ REST 请求 / SSE 进度
Rust Backend · Axum，类型校验与权限边界
    ↓ 需要任务编排时
Agent · 结构化决策循环
    ↓ 白名单与依赖检查
Tools · 确定性解析、画像、评分、比较
    ↓ 按用途与平台能力调用
Music Platform Connectors · 官方 API / OAuth
```

这是职责分层，不是所有请求都必须经过 Agent。文件解析、公开链接检查和 Copy 也有独立 API；平台原始曲目不会经过 LLM。前端负责交互，Rust 保有计算与执行权。SQLite 保存 Agent 历史、设置和检查点；导入预览、分析登记和 Copy run 含进程内状态，Copy 不能跨后端重启恢复。

## 2. Platform Capability：统一描述，不强行统一权限

| 平台 | 当前实现 | 课程验收边界 |
|---|---|---|
| Spotify | OAuth + Track Import；账号与 URL 官方读取 | 既有真实读取验收通过；真实写入未验收 |
| YouTube / YouTube Music | Google OAuth + Track Import；官方分页读取 | 既有真实读取验收通过；真实写入未验收 |
| Apple Music | Official API + Credential Required；公开目录 reader | 需部署者配置、尚未真人验收；私人资料库与写入未支持 |
| 网易云、QQ | ACCESSIBILITY_CHECK_ONLY + 文件/文本 | 识别链接、检查公开可访问性，不能从链接读取完整歌曲 |
| 酷狗、汽水 | URL_RECOGNITION_ONLY + 文件/文本 | 只识别链接，不能从链接读取完整歌曲 |

`platforms.rs` 提供能力真相源。账号授权、URL 识别、页面可访问性、曲目读取、写入和数据用途是不同维度；能识别 ID 或收到 HTTP 成功响应，并不等于有完整歌曲数据或使用许可。前端按这些字段提供实际入口，能力不足时回退到用户提供文件或文本，而不是伪造 Connect 或曲目。

## 3. Import Pipeline

```text
File / 粘贴文本 → parse_import → ImportedTrack → Import Preview
                                                   ↓ 用户确认
                                             统一 Track Model
                                                   ↓
                                    Metadata / Analysis → Analysis ID

URL → Recognition → 按能力检查页面或官方读取
OAuth → 官方账号歌单读取 ────────────────┘
                      ↓ 只有实际返回曲目
               Track / Import Preview → 确认后的 Copy 流程
```

统一模型便于复用显示和匹配，但不会抹去来源与用途限制。当前平台 API 曲目不进入画像、推荐、跨平台比较或 LLM；本地文件/文本才走上述 Analysis 主线。中国平台链接无 tracks 时没有 Import Preview，用户需上传 CSV/TXT/JSON/M3U 或粘贴 `歌手 - 歌名`。缺少时长或 Genre 保持缺失，不生成虚假音乐事实。

## 4. Agent Workflow

```text
User Goal → Planning → Tool Selection → Execution → Explanation
                ↑                          │
                └──── ToolResult 反馈 ──────┘
```

用户绑定真实 Analysis（好友场景绑定两份），可选模型从当前依赖已满足的白名单工具中做结构化选择。Rust 校验 `AgentDecision` 并执行工具；结果决定继续、重规划或结束。例如惊喜候选不足时可选择 Genre Bridge 或 second-hop artist 分支。页面展示 User Request → Agent Decision → Tool Call → Tool Result → Final Explanation，内容是可核验状态与摘要，不是隐藏思维过程。

没有可用模型时显示 `DETERMINISTIC_FALLBACK`，仍可执行真实 Rust 工具；真实数据、Demo 与模型模式分别标识。既有 Agent 验收为 fallback，不能称为真实 LLM 验收。步骤、超时、重试和费用都有上限；SSE 更新进度，取消/恢复与历史保留执行依据。

## 5. 为什么不用 Cookie / Reverse API

稳定性：官方协议有明确文档和权限边界；页面内部状态、签名和逆向接口可能随时变化，不适合作为课程项目的可靠依赖。

安全性：用户只在平台官方 OAuth 页面登录，MelodyPath 不接收平台密码或用户提供的登录 Cookie，不模拟或自动登录。应用自身用于会话的不透明 HttpOnly Cookie 与索取平台登录 Cookie 是两件事；本轮没有新增认证机制。

合法性与用途：只在已获得的官方权限和允许用途内调用，避免绕过访问限制。网易云条件导入只读取匿名官方公开 HTML / JSON-LD 中明确暴露的数据，限制为 20 首，并保留失败降级；页面可访问本身仍不等于歌曲已读取。这是项目设计边界，不是对所有平台或所有地区法律的泛化判断。

外部写入还需独立确认：Spotify → YouTube MVP 新建私有目标，源歌单只读，歧义需人工处理，单首失败独立报告。只有真实获得新 YouTube 播放列表链接才算真实迁移验收通过。
