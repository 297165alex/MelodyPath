# MelodyPath 最终课程就绪报告

更新时间：2026-09-06

## 最终收尾复核（2026-09-06）

- `REAL VERIFIED`：五份真实歌单 Last.fm 回归保留；本次再次通过浏览器上传 Happy_Mix.csv，50/50 解析、50 首参与分析、22 完整/28 部分/0 未匹配、`is_demo=false`。
- Happy_Mix Agent：`REAL_FILE`、`use_demo=false`、`COMPLETED`、13 次 Rust 工具调用全部成功；10 首种子、272 原始候选、12 首推荐。运行绑定的 analysis id 与最终结果一致，推荐标题集合指纹与页面逐项一致（仅记录相等结果，不记录私人曲目内容）。真实路径的 Demo 隔离同时由代码分支和 Rust 回归测试证明。
- `IMPLEMENTED BUT CONFIG BLOCKED`：LLM Agent Controller 已实现，当前真实运行是 `DETERMINISTIC_FALLBACK`；input/output tokens 均为 0，cost 为 0，未宣称真实 LLM 验收。
- Spotify/YouTube：官方代码与 Mock 保持通过，真实平台仍为 `BLOCKED_BY_CONFIGURATION / MANUAL_AUTH_REQUIRED`，没有真实目标播放列表链接。
- Apple：当前 `IMPORT ONLY / CONNECTOR NOT IMPLEMENTED`；目标为官方 MusicKit / Apple Music API 账号连接和歌单读写。QQ/网易云为 `IMPORT ONLY`；酷狗为 `PARTNERSHIP_REQUIRED / IMPORT ONLY`。未实现的公开链接曲目读取与 writer 均为 `UNSUPPORTED`。
- 六个直达路径 `/`、`/discover`、`/compare`、`/versions`、`/transfer`、`/agent` 顺序浏览器复查通过；两个缺失的路由映射已修复。无绑定时 Discover 显示导入提示，未回退首页或 Demo。新页面控制台错误 0，Happy_Mix Agent 页面错误 0。
- 最终 Rust 格式/编译/测试通过（105/105）；路由小修后再次运行 frontend lint/build 通过。Rust 未使用代码警告保留，未删除或弱化测试。
- 当前本地前端为 `http://127.0.0.1:6174/`，后端 `/health` 为 200。Windows 将原 5174 所在范围列为保留端口，故本次用 6174 验收；未改系统保留端口。公网部署仍为 `PARTIAL`。
- 安全交付：按明确白名单打包 61 个源码、Markdown 与必要配置文件到 `C:\Users\user\Downloads\melody-path-final-audit.zip`。归档创建与复核只检查路径和配置引用，不读取环境 Secret 或私人歌单内容；不包含数据库、私人 CSV、token 文件、环境文件（`.env.example` 除外）、构建产物或缓存。未修改、删除、移动任何源项目文件以进行打包。

## 1. 产品定位与专用 Agent 价值

MelodyPath 是一个以 Rust 为可信计算核心的专用音乐 Agent。它接收真实歌单，完成统一导入、可解释分析、三段探索、好友临时比较、版本探索，以及经过预览和确认的跨平台复制。它不是“让 LLM 猜歌曲”的聊天壳：音乐事实、匹配、评分、身份、权限和外部写入全部由确定性 Rust 代码控制。

当前可完整演示的真实主线是：真实文件/文本 → 导入预览 → 本地分析 → 真实 Last.fm 候选 → 舒适/拓展/惊喜 → 换一批 → 真实数据 Agent → Compare。Version Radar 和 Spotify → YouTube Copy 已完成代码、Mock 和安全闸门，但真实平台结果仍需要开发者配置和真人 OAuth，不能标成真实成功。

## 2. LLM 与 Rust 的职责边界

LLM 只允许：解析自然语言 Goal 为结构化 Intent；在当前依赖前沿的 Tool Registry 白名单中选择下一工具；根据紧凑 ToolResult 决定 Continue、Replan 或 Finish；生成最终解释。`AgentDecision` 通过 serde 校验，未知工具和非法参数在执行前被拒绝。LLM 没有 shell、任意文件、任意 HTTP、任意 SQL 或未注册工具权限。

Rust 保留：playlist parsing、normalization、track/artist identity、Genre、metadata confidence、seed selection、Last.fm 调用、recommendation scoring、dedup、version classification、compare metrics、transfer matching、OAuth capability、确认闸门和所有外部写入。Spotify API 数据不会发送给 LLM。

当前环境没有配置 LLM 服务，因此真实浏览器 Agent 的 Decision Mode 为 `DETERMINISTIC_FALLBACK`，Token 和费用均如实为零。这证明 fallback 与真实数据绑定，不构成 `REAL LLM VERIFIED`。

## 3. Agent Loop 与 Tool Registry

实际控制环为：Decision → schema/registry validation → Rust Tool → ToolResult → 新 Decision → Continue/Replan/Finish。每次只向决策器开放依赖已满足的工具前沿。Personal 场景会依据 surprise 候选遥测在 `genre_bridge_candidates` 与 `second_hop_artist_candidates` 之间动态选择；Friend Bridge 会根据真实 overlap 选择 shared-affinity 或 complementary-bridge 策略。

每次运行保存 Goal、Intent、data state、decision mode、计划、步骤、ToolCall、ToolResult 摘要、状态、时间、Token、费用、警告和 checkpoint。SSE 推送进度，支持 cancel、timeout 和 resume；最大步骤、重试、Token 和费用上限由 Rust 强制执行。真实任务只绑定真实 Analysis/Comparison；仅用户明确选择 Demo 时才可调用 Demo payload。

## 4. 推荐、Surprise 与换批

真实推荐由 Last.fm Provider 生成候选，Rust 本地完成规范化、版本过滤、跨语言艺术家身份、源歌单排除、去重、同艺人上限、三区评分与路线构建。Apple 目录不负责推荐候选，真实模式没有固定候选池或 Demo 兜底。

Controlled Serendipity 可使用 tag depth 2、Genre Graph 两跳、second-hop similar artist 与 bridge candidate；每个惊喜候选必须保留来源和桥梁理由。Ballad、Korean Ballad、J-Pop Ballad、Mandopop Ballad 纳入规范化与桥梁关系。遥测包含 core tags、tag 请求状态、layer 1/2、tag top tracks、Genre bridge、second-hop artist、请求预算、预算耗尽和 retry。

每区首屏最多 4 首，但候选池不会在首屏裁切后丢弃。`comfort_pool`、`expansion_pool`、`surprise_pool` 分别维护 cursor 和 seen ids；换批优先消费已有池，不重新请求 Last.fm，耗尽后明确提示。

## 5. 真实 Last.fm 回归

以下为本机真实 Provider、串行浏览器验收的聚合记录；私人曲目名称不写入文档或 ZIP。

| 文件 | tracks | seeds 成功/失败 | raw | 版本后 | 去重后 | 排除源歌单后 | 艺人上限后 | comfort/expansion/surprise pool | tag L1/L2 | Genre bridge / second-hop | 请求/重试 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Japanese.csv | 76 | 10/0 | 234 | 233 | 221 | 209 | 168 | 127/16/25 | 0/20 | 20/12 | 43/0 |
| Korean.csv | 224 | 10/0 | 291 | 291 | 235 | 178 | 136 | 104/16/16 | 0/20 | 20/12 | 42/0 |
| English.csv | 435 | 10/0 | 262 | 252 | 236 | 178 | 130 | 106/13/11 | 0/10 | 10/12 | 39/0 |
| Chinese.csv | 781 | 10/0 | 174 | 173 | 148 | 120 | 80 | 46/14/20 | 0/20 | 20/12 | 42/0 |
| Happy_Mix.csv | 50 | 10/0 | 272 | 267 | 252 | 240 | 175 | 138/15/22 | 0/20 | 20/12 | 4/0（缓存命中） |

五份均为 `is_demo=false`，首屏三区各 4 首，结果集合明显不同；默认版本过滤和源歌单排除生效。第二批 comfort 与首批无重叠。详细验收边界见 `recommendation_acceptance_report.md`。

## 6. Compare 与 Version Radar

`/compare` 接收两份统一来源，各自先预览确认，再计算 Track、Artist、Genre、Tag 和互补度；真实临时比较已在浏览器验证为 `REAL_TEXT / is_demo=false`。共同候选排除双方源歌单，空结果不补 Demo。平台账号数据能否用于 Compare 由用途级 Capability 独立控制。

`/versions` 复用统一版本身份与 YouTube 官方搜索，按批次、请求上限、超时、重试执行并支持取消。结果按源歌曲分组，显示版本、平台、官方/Topic 信号、confidence 与理由；Add 必须经过 preview → confirm → write。页面已绑定真实当前 Analysis，但真实 YouTube 搜索仍是 `BLOCKED_EXTERNAL_AUTH`，只有显式 Mock 测试通过。

## 7. Copy Playlist

第一版产品边界为 Spotify → YouTube：读取用户拥有或协作的 Spotify 歌单，规范化为 `TransferTrack`，用 YouTube 官方搜索获取每首最多 5 个候选，确定性计算标题、艺人、时长、官方频道和版本词，区分 high/ambiguous/unmatched；用户解决歧义并明确确认后，创建新的私有 YouTube 播放列表并逐首写入。源 Spotify 歌单没有删除或修改路径。

执行采用 run id、状态查询、SSE、cancel/resume 和逐首结果；单曲失败继续，CSV/JSON 报告可下载。当前 run registry 为后端进程内状态，尚不支持服务重启后恢复。两个平台真实 OAuth 尚未配置和真人授权，因此状态为 `MANUAL_AUTH_REQUIRED`，没有真实目标链接。

## 8. Platform Capability 矩阵

| 平台 | 账号连接 | Copy source | Copy destination | Compare/Recommendation | 当前状态 |
|---|---|---|---|---|---|
| Spotify | 官方 OAuth 代码完成 | 第一版来源 | writer 代码存在但非本版验收目标 | 账号 API 数据禁用 | `IMPLEMENTED BUT UNCONFIGURED` / `MANUAL_AUTH_REQUIRED` |
| YouTube | Google OAuth 代码完成 | 统一模型可扩展 | 第一版目标 | 只按允许用途；版本搜索需授权 | `IMPLEMENTED BUT UNCONFIGURED` / `MANUAL_AUTH_REQUIRED` |
| Apple Music | 本项目未实现 MusicKit 连接器 | 否 | 否 | 文件/文本导入 | `IMPORT_ONLY` / `BLOCKED` |
| QQ音乐 | 无已验证通用个人 OAuth | 否 | 否 | 文件/文本导入 | `IMPORT_ONLY` |
| 网易云音乐 | 无已验证通用个人 OAuth | 否 | 否 | 文件/文本导入 | `IMPORT_ONLY` |
| 酷狗音乐 | 需要正式合作资格 | 否 | 否 | 文件/文本导入 | `PARTNERSHIP_REQUIRED` / `IMPORT_ONLY` |

用途级字段区分 `playlist_read_for_copy`、`playlist_read_for_compare`、`playlist_read_for_recommendation`，前端按后端能力渲染，不显示无实现的 Connect 或 Writer。官方依据见 `platform_capabilities.md` 与 `chinese_platform_official_api_audit.md`。

Apple Music 的 **CURRENT IMPLEMENTATION** 为 `IMPORT ONLY / CONNECTOR NOT IMPLEMENTED`；**TARGET PRODUCT** 是使用官方 MusicKit / Apple Music API 连接账号、读取歌单、创建新歌单与写入歌曲，手动 CSV 不是目标产品的主要入口。该目标连接器本轮未开发、未授权、未验收。

中国平台入口优先级为：合法且当前实际可读取曲目的 Public Link → Paste tracks → File fallback。目前链接识别/可访问性检查不等于曲目读取，因此 QQ/网易云仍是 `IMPORT ONLY`，酷狗为 `PARTNERSHIP REQUIRED / IMPORT ONLY`。

## 9. Universal Web 与部署

前端使用同域相对 `/api`；`PUBLIC_BASE_URL` 可派生 OAuth callback，`VITE_API_PROXY_TARGET` 仅用于开发。部署要求包括 HTTPS、Secure/HttpOnly/SameSite Cookie、OAuth state、trusted origin/CORS 与后端 Secret 注入；这些生产条件仍待真实部署验证。本地 OAuth token 使用 Windows DPAPI；`OAuthTokenStore` 已建立生产替换边界，但托管 KMS/加密存储实现尚未提供。

当前只有本地开发地址，没有公网域名、证书或部署主机，因此 Universal Web 状态为 `PARTIAL`。部署者仍需配置平台开发者应用和服务端环境；普通终端用户不应运行 PowerShell或接触 Key。详见 `deployment.md`。

## 10. R1–R6

| 要求 | 状态 | 结论 |
|---|---|---|
| R1 Rust Core | `PASS` | 核心音乐计算、Agent 状态、匹配、权限与写入均在 Rust。 |
| R2 UI | `PASS` | Analyze、Discover、Compare、Versions、Copy、Agent、Connections 均有界面。 |
| R3 Configurable LLM | `PARTIAL` | endpoint、model、temperature、tokens、timeout、retry、price、cost limit 可配置；真实 LLM 决策环仍配置阻塞。 |
| R4 Realtime progress + interrupt | `PASS` | Agent 与 Copy 本地单进程均有 SSE/cancel/resume；Copy 不跨进程恢复。 |
| R5 History | `PASS` | Agent 关键字段与 checkpoint 存入 SQLite，不保存 OAuth Secret。 |
| R6 Token + Cost | `PARTIAL` | usage 记录与费用上限自动测试通过，真实非零 Token/Cost 未验收；fallback 运行诚实为 0。 |

## 11. 测试与验证

- Rust：`cargo fmt --all --check`、`cargo check --workspace`、`cargo test --workspace`；最终套件 105/105。
- Frontend：`npm run lint` 与 `npm run build` 通过。
- 浏览器：真实文件分析/推荐/换批、真实数据 Agent、真实临时 Compare、Versions OAuth 阻塞、Transfer OAuth 阻塞和平台能力状态均已检查。
- Mock 只验证 Connector、匹配、歧义、失败隔离、取消/恢复和确认闸门；未计为真实外部平台验收。

## 12. Token、费用与安全

模型调用受 max agent steps、timeout、retry limit、max tokens 与 max cost USD 约束。UI 只显示结构化决策理由与 ToolResult 摘要，不展示隐藏思维过程。Secret 只允许从后端运行环境/安全存储读取；不得进入前端、Git、日志、SQLite、报告或 ZIP。OAuth token 不返回浏览器 JavaScript，外部写入必须显式确认，不存在删除源歌单路径。

## 13. 阻塞与人工事项

- `OPENAI_API_KEY` 与可用 endpoint：配置后才能执行真实 LLM 多步决策验收。
- Spotify 开发者应用、redirect URI 和真人授权：完成后验证分页读取。
- Google OAuth、YouTube Data API、redirect URI、配额和真人授权：完成后验证搜索、创建私有播放列表和逐首写入。
- 只有取得真实新 YouTube 播放列表链接，Copy 才能改为 `REAL AUTH VERIFIED`。
- 公网域名、HTTPS、反向代理、托管 Token Store 与运行基础设施尚需部署者提供。
- Apple Music 连接器不在当前实现；中国平台保持导入/合作边界。

## 14. 五分钟 Demo

演示顺序：问题与定位（0:00）→ 真实文件 Analyze（0:30）→ 三区 Discover（1:10）→ 换一批（1:50）→ Agent Decision/Tool/Result/Replan（2:10）→ Compare（2:55）→ Version Radar（3:35）→ Spotify→YouTube Copy 安全闸门（4:10）→ Rust、Token/Cost 与安全（4:45）。逐字稿见 `demo_script.md`。
