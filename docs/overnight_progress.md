# MelodyPath 夜间开发进度

更新时间：2026-09-04（Asia/Shanghai）

## 执行规则

- 严格保留已有 Last.fm 推荐、本地分析、好友桥梁、Agent Loop、历史、中断和导出。
- 不读取、输出、记录或提交任何 API Key、Client Secret、OAuth Token、密码或 Cookie。
- 真实 API、Mock 验证、OAuth 阻塞和计划功能必须分别标记。
- 不执行 `git push`，不发布公网，不使用私有或逆向平台接口。

## 阶段 0：恢复和保护现有项目

状态：完成

已完成：

- 完整阅读项目根目录 `README.md`、`AGENTS.md`、`PLAN.md` 和 `docs/` 中现有 Markdown 文档。
- 检查项目目录和 Git 状态；工作区在本轮开始时无未提交修改。
- 确认以下实现仍存在：真实文件导入、`LastFmRecommendationProvider`、三区推荐、历史、SSE 进度、中断、导出、Spotify/YouTube 官方 OAuth 代码和现有测试。
- 确认 `docs/recommendation_acceptance_report.md` 记录的未完成问题仍是本轮推荐修复基线：版本候选、跨语言艺术家别名和流水线分段统计。

待完成：

- 基线检查全部通过：`cargo fmt --all --check`、`cargo check --workspace`、`cargo test --workspace`（54/54）、`npm run lint`、`npm run build`。
- 确认此前中断的推荐修复未落盘；当前代码仍复现报告所述身份、版本和统计缺口。

已知限制：

- Spotify 与 Google/YouTube 的真实 OAuth 仍需要账号持有人和开发者配置，夜间任务只完成不依赖人工授权的编码、Mock、测试、文档和本地浏览器验证。
- 上一次真实推荐验收的惊喜区为空，且中间统计不足；不得用 Demo 补齐。

下一阶段：阶段 1——推荐身份、版本语义与流水线统计可靠性修复。

## 阶段 1：推荐可靠性修复

状态：代码与自动测试完成；真实 Last.fm 回归暂时阻塞

已完成：

- 新增集中式 `identity.rs`，统一 `ArtistIdentity`、`TrackIdentity`、稳定 ID、规范化名称、组合艺人拆分和受控别名表；`Jay Chou / 周杰倫 / 周杰伦` 与 `JJ Lin / 林俊傑 / 林俊杰` 不再由业务代码中的散落条件处理。
- 统一解析 Original、Live、Concert、Remix、Acoustic、Unplugged、Bonus Track、Remaster、Sped Up、Slowed、Radio Edit、Instrumental、Karaoke、Cover、Reaction 和 Nightcore；普通推荐过滤派生版本，但仍在模型中保留版本语义供 Alternate Versions 使用。
- 真实推荐依次记录 Last.fm 原始、版本过滤后、规范化后、去重后、源歌单排除后、同艺人上限后、三区结果，以及 tag 第一层/第二层获取与淘汰统计。
- 推荐结果新增 `already_in_source_playlist`；页面分别展示 Last.fm 原始相似度、Rust 综合分和 UI 置信度。
- 增加 Dancin/Krono Remix、Bang Bang/Bonus Track、明确 Remix 源语义、晴天与背對背擁抱跨语言艺人别名、逐阶段统计和 tag 两层统计回归测试。

测试结果：

- `cargo fmt --all`：通过。
- `cargo check --workspace`：通过（只有既有/非阻塞 dead-code 警告）。
- `cargo test --workspace`：62 passed，0 failed。
- `frontend/npm run lint`：通过。
- `frontend/npm run build`：通过。

真实回归阻塞：

- 当前没有运行中的前后端服务。
- 仅检查配置存在性的布尔状态后，Process/User/Machine 三个作用域均未检测到 `LASTFM_API_KEY`；检查过程中没有读取或输出密钥内容。
- 因此不能用 Mock 冒充 Japanese/Korean/English/Chinese 的真实联网复验。本项记录为 `BLOCKED_EXTERNAL_CONFIGURATION`，不会阻塞后续不依赖真实 Last.fm 配置的本地开发。

下一阶段：阶段 2——轻量、可恢复、可测试的 Agent Orchestrator。

## 阶段 2：Agent Orchestrator

状态：完成（确定性本地执行与 Mock 场景验证）

已完成：

- 在现有 `AgentService` 上增加 `AgentGoal`、结构化 `AgentIntent`、`AgentPlan`、`AgentStep`、`AgentToolCall`、`AgentToolResult`、`AgentState` 和完整 `AgentRun` 快照。
- 统一运行状态为 `PLANNING / RUNNING / WAITING_USER_CONFIRMATION / BLOCKED_EXTERNAL_AUTH / COMPLETED / FAILED / CANCELLED`。
- 每一步执行前后持久化计划、已完成/待执行步骤、工具调用、工具结果、重试次数、警告和恢复检查点；SQLite 旧数据库通过非破坏性加列迁移继续可用。
- 工具结果必须通过非空校验后才能进入下一步；临时错误按配置执行有界退避重试；最大步骤和总超时在执行前/执行中强制检查。
- 新增 `POST /api/tasks/{id}/resume`，允许 FAILED/CANCELLED 任务从未完成步骤继续；服务重启会把运行中任务标记为可恢复失败，而不伪装完成。
- Agent UI 展示真实计划、步骤状态、工具名、尝试次数、重试和工具调用数量；支持中断及从检查点恢复。
- LLM 仍只作为可选解释增强；解析、分析、比较、候选、过滤和评分由确定性工具完成。外部平台歌单数据没有新增任何 LLM 发送路径。

测试结果：

- Agent 定向测试 5/5 通过：计划执行和工具结果持久化、一次临时失败后重试、取消、从检查点恢复、最大步数保护。
- 前端类型检查在 Agent 类型同步前的基线通过；本阶段最终会随阶段 9 再运行完整 lint/build。

已知限制：

- 当前两个 Agent 场景使用项目明确标注的 Demo 输入完成本地调度验证；这不代表任何 Spotify/YouTube 真实连接。
- `WAITING_USER_CONFIRMATION` 与 `BLOCKED_EXTERNAL_AUTH` 已建立状态模型，将在 Playlist Action/Transfer 工作流接入。

下一阶段：阶段 3——独立 `/compare` 两份真实本地歌单临时比较流程。

## 阶段 3：好友音乐比较

状态：本地真实输入流程完成；真实共同推荐受 Last.fm 配置状态约束

已完成：

- 新增可直接打开的 `/compare` 客户端路径；两侧分别支持 CSV/TSV/JSON/TXT/M3U8 文件和批量文本，均先调用现有 Rust 导入预览，再由用户确认比较。
- 新增 `POST /api/compare`，接收两份已经完成统一导入/分析的 `PersonalDemo`；默认临时处理，`saved_locally=false`，不会建立公开社交账号。
- `compare_playlists` 不再为真实输入返回固定桥梁曲目；动态计算 Track、Artist（含别名）、Genre、Tag Jaccard 和多样性互补度，并分别说明各指标。
- 共同推荐划分 `Safe for Both / Bridge / Adventure Together`，同时排除 A、B 两份源歌单，保留对 A、对 B 的理由、共同依据、Provider 和分数。
- 当 Last.fm 未配置或没有双方都可解释的候选时保持空列表，并在页面说明没有使用 Demo 补齐。
- Demo 比较仍可通过单独按钮明确打开，且 `is_demo=true / data_source=DEMO`。

测试结果：

- Engine 定向测试 9/9 通过，覆盖相同歌单、完全不同、部分重合、统一跨来源模型及双方源歌单排除。
- 前端 TypeScript lint 通过。

已知限制：

- 本轮没有新增好友数据持久化；用户请求保存时服务会明确拒绝，避免把未实现的保存状态标成成功。
- Spotify/YouTube 链接仍遵守现有官方权限检查；不可读取时应连接账号或上传导出文件。

## 阶段 4：Alternate Versions Explorer

状态：代码与 Mock 验证完成；真实搜索为 OAuth 阻塞

已完成：

- 新增独立 Alternate Version 搜索模型和评分逻辑；版本解析与普通推荐共用同一规范化实现，不会破坏默认推荐的版本过滤。
- 推荐卡提供 `Explore other versions`，可选择 Live、Concert、Remix、Acoustic、Unplugged、Remaster。
- 真实路径只调用当前用户通过官方 OAuth 授权后的 YouTube Data API；候选按基础歌名、艺人、版本一致性和官方/Topic/VEVO 频道信号确定性评分。
- 候选明确显示平台、版本类型、官方状态、置信度、来源 URL 和理由；无原版/无合格候选时诚实显示 unavailable。
- 缺少 Google/YouTube 配置或用户授权时返回 `BLOCKED_EXTERNAL_AUTH`，不会自动切换 Mock；Mock 必须由用户明确点击，并显示 `MOCK_VERIFIED` 和 Mock connector 标签。

测试结果：

- Alternate Versions 定向测试 3/3 通过：Original → Live、Original → Remix、`Remix to Ignition` 假关键词和同名不同艺人排除。
- 前端 TypeScript lint 通过。

已知限制：

- 当前环境未配置/授权 Google OAuth，真实 YouTube 版本搜索未执行；状态为 `BLOCKED_EXTERNAL_AUTH`。
- Search API 首次返回不含可靠时长，当前候选的 `duration_ms` 可为空并如实展示；Transfer 阶段会对视频详情批量补充时长后再执行跨平台匹配。

下一阶段：阶段 5——Spotify → YouTube Transfer 的确定性预览、Mock connector、确认和可恢复写入骨架。

## 阶段 5：Transfer MVP

状态：代码与 Mock 验证完成；真实 OAuth 验收阻塞

已完成：

- 新增 `/transfer`，第一版只实现 Spotify/真实文件/当前分析作为只读来源，YouTube 作为唯一目标；没有加入其他平台。
- 新增统一 `TransferTrack` 和 `SourceConnector / DestinationConnector`，Spotify 数据不经过 LLM。
- YouTube 查询采用“艺人 + 歌名 + official audio”，每首最多保留 5 个候选；确定性评分拆分标题、艺人、真实时长差、官方/Topic 信号和版本一致性。
- 匹配状态为 `MATCHED_HIGH / MATCHED_AMBIGUOUS / UNMATCHED`；歧义和版本回退必须逐首改选或跳过，未确认时后端拒绝创建播放列表。
- Alternate fallback 默认关闭；开启后仍标记为歧义，不会假装原版。
- 目标只创建新的私有 YouTube 播放列表；逐首失败继续，源 Spotify 歌单永不写入；结果包含成功、失败、跳过、未匹配、进度和新列表链接。
- 生成可下载 CSV/JSON 迁移报告。Mock 链路使用 `mock://` 且页面明显显示 Mock connector，不会显示为真实 YouTube。
- 核心执行器支持取消，并可基于既有目标列表和已完成曲目恢复，不重复写入已完成歌曲。

测试结果：

- Transfer 测试覆盖源分页、最多 5 候选、标题/艺人/时长/官方/版本评分、歧义、fallback 开关、未确认拒绝、单曲失败继续、取消、恢复不重复、CJK 和 500 首输入。
- 本轮全量 Rust 测试：84 passed，0 failed；前端 TypeScript lint 通过。

阻塞：

- 当前环境没有 Google/YouTube OAuth 配置与用户授权，无法执行真实创建播放列表；真实状态必须是 `BLOCKED_EXTERNAL_AUTH`，Mock 结果不能作为真实验收。

## 阶段 6：推荐后加入歌单

状态：完成（复用既有安全写入流程）

- 普通推荐、好友共同推荐、Alternate Version 候选均新增 `Add to playlist` 操作。
- 操作统一进入已有 `PlaylistWriter` 的目标选择、候选预览、歧义选择和明确确认；没有任何“一次点击直接写入”。
- Spotify/YouTube 仅允许当前授权用户可写目标；未授权时显示真实配置/授权状态。Mock 和文件导出继续明确区分。

## 阶段 7：Agent 工作流 UI

状态：完成

- 首页新增 “What would you like MelodyPath to do?”，提供 Analyze、Discover、Compare、Alternate Versions、Move playlist 和 Agent workflow 六个入口。
- Agent 页面显示 Goal、真实 Plan、每步工具、状态、尝试次数、整体进度、重试、取消和恢复；不展示或伪造隐藏思维过程。
- `/compare` 与 `/transfer` 支持直接 URL 打开及浏览器前进/后退。

## 阶段 8：错误恢复与可靠性

状态：自动测试完成

- Spotify/YouTube HTTP Client 增加 12 秒超时；YouTube 搜索对 429/5xx 和临时网络错误最多重试两次并退避，4xx 授权/请求错误不盲目重试。
- YouTube 搜索批量读取 `contentDetails.duration`，时长缺失保持 `None` 并在评分解释中注明“不按 0 处理”。
- 外部搜索单曲失败转为该曲未匹配，不使整个 Transfer 预览崩溃；写入单曲失败继续后续歌曲。
- 覆盖 malformed import、空源拒绝、别名、Unicode/CJK、500 首、取消、恢复、OAuth missing、Provider unavailable 和敏感配置 Debug 脱敏。

下一阶段：阶段 9 全量质量检查，然后启动单套前后端执行阶段 10 浏览器验收。

## 阶段 9：全量质量检查

状态：通过

- `cargo fmt --all --check`：通过。
- `cargo check --workspace`：通过；仅保留不影响构建的 dead-code 警告，用于后续正式 Connector 接入和恢复接口。
- `cargo test --workspace`：84 passed，0 failed。
- `frontend/npm run lint`：通过。
- `frontend/npm run build`：通过，Vite 生产构建成功。

本阶段未使用 Mock 代替任何真实联网验收，也未读取或输出任何凭据。

下一阶段：启动唯一一组本地前后端并进行阶段 10 浏览器验收。

## 阶段 10：真实浏览器验收

状态：本地流程通过；真实外部服务保持阻塞

运行环境：

- 仅启动一套后端（`127.0.0.1:3000`）和一套前端（`127.0.0.1:5174`）。
- `/health` 返回 200；浏览器控制台无 warning/error。

验收结果：

- 首页：任务启动器六个入口均可见；本地导入与 Demo 按钮明确分离。
- `/compare`：两份各 3 首的真实文本均先预览为 3/3，再明确确认比较；结果为 `REAL_TEMPORARY_COMPARISON`、`is_demo=false`。由于 Last.fm 未配置，共同推荐为空且页面明确说明没有 Demo 补齐。
- `/transfer`：上传 `testdata/Japanese.csv` 后显示 76/76 首真实解析、源歌单只读。真实 YouTube Provider 返回 `BLOCKED_EXTERNAL_AUTH`（76 首均保持未匹配），没有自动切换 Mock；用户明确选择 Mock Connector 后得到 76 个高置信 Mock 匹配，确认前执行按钮禁用，确认后完成 76/76，链接为 `mock://...` 且明确不代表真实 YouTube。
- Agent：个人 Genre 探索工作流实际生成并显示 13 个步骤、工具名、尝试次数、进度和 `COMPLETED` 状态，执行记录写入本地 SQLite。
- Alternate Versions：推荐卡可展开 Live/Concert/Remix/Acoustic/Unplugged/Remaster；真实 YouTube 搜索返回 `BLOCKED_EXTERNAL_AUTH`，明确没有搜索或 Mock 回退；显式 Mock 返回独立标记的 Live/Remix/Acoustic 候选。
- Add to playlist：推荐卡进入共用 Playlist Writer；真实平台显示未配置，显式 Demo 目标可生成预览，最终写入按钮在勾选明确确认之前保持禁用。

未完成的真实外部验收：

- Last.fm：`LASTFM_API_KEY` 在当前启动环境不可用，因此未执行四份真实文件的联网推荐回归。
- Spotify/Google OAuth：开发者配置和用户授权缺失，因此没有读取真实 Spotify 歌单，也没有创建真实 YouTube 私有播放列表。

下一阶段：更新架构、功能状态和人工接续文档，然后执行最终敏感信息与未完成项审计。

## 阶段 11：文档与交接

状态：完成

- `README.md` 新增统一能力状态表，并把 Transfer 从“尚未开始”更新为 IMPLEMENTED / MOCK VERIFIED / BLOCKED_EXTERNAL_AUTH。
- `AGENTS.md` 补充现有 Transfer 代码位置、真实 OAuth 验收优先级和同步 HTTP 执行限制。
- `PLAN.md` 更新推荐、Compare、Alternate Versions 和 Transfer 的实际阶段状态。
- `docs/teammate_handoff.md` 重写为当前代码交接，列出真实阻塞、实施顺序、人工 OAuth 步骤和 GitHub 安全清单。
- 新增 `docs/agent_architecture.md`、`docs/social_compare.md`、`docs/alternate_versions.md` 和 `docs/transfer_mvp_report.md`。
- 文档没有写入或回显 API Key、Client Secret、OAuth Token、Cookie 或私人歌单内容。

下一阶段：最终审计 Git 状态、忽略规则、疑似秘密文件、遗留占位和代码/文档状态一致性，并写入 `docs/FINAL_OVERNIGHT_REPORT.md`。

## 阶段 12：最终审计

状态：完成；存在一个需要维护者确认的 Git 隐私事项

- `git diff --check` 通过，仅有 Windows 行尾转换提示。
- 源码中没有 `todo!()`、`unimplemented!()` 或实际 TODO/FIXME；搜索到的 placeholder 均为 UI 占位控件名称或 Apple 配置拒绝占位凭据的测试。
- 未发现源码/文档中的常见高风险 Key 或私钥格式；没有输出任何凭据内容。
- `.gitignore` 增加私人测试 CSV 与日志规则，并已覆盖环境文件、token、数据库和构建目录。
- 四份 `testdata/*.csv` 在本轮开始前已经被 Git 跟踪。ignore 规则不能追溯取消跟踪；为避免未经确认修改 Git index，本轮没有执行 `git rm --cached`。提交前必须由维护者确认公开权限或取消跟踪并审计历史。
- 未创建 commit、未 push、未部署。
- 完整实现、测试、浏览器结果、外部阻塞、限制和人工步骤写入 `docs/FINAL_OVERNIGHT_REPORT.md`。

最终状态：所有无需账号授权的任务已完成；真实 Last.fm 与 Spotify/YouTube OAuth 验收分别等待安全环境配置和用户手动授权。

## 2026-09-05 最终产品收口检查点

状态：代码收口完成，等待最终全量命令复核与安全 ZIP。

2026-09-06 终检更新：Rust fmt/check/test 通过（105/105），前端路由小修后 lint/build 通过。六页面直达浏览器复查通过，控制台错误为 0。Happy_Mix 重新上传后 50/50 解析、REAL_FILE、is_demo=false；Agent 13 个真实 Rust 工具完成，bound analysis 与结果一致，12 首推荐与对应页面集合相等。LLM 仍为 DETERMINISTIC_FALLBACK，Token/Cost=0。已核对安全归档 61 文件白名单，最终交付路径为 Downloads/melody-path-final-audit.zip；不读取 Secret，不包含私人数据，不删除源文件。当前前端 6174（原 5174 被 Windows 保留），未公开部署、未 git push。外部 OAuth 与 LLM 配置保留明确阻塞，本轮收尾后停止。

- 真实 Last.fm 环境已恢复；Japanese、Korean、English、Chinese 与 Happy_Mix 五份真实文件串行浏览器回归均为 `is_demo=false`，三区均有真实候选，五份结果指纹互不相同。
- Agent 已绑定真实 Analysis/Comparison；模型控制器只可选择当前依赖前沿的白名单工具，真实 ToolResult 会改变 surprise recovery 与 friend strategy 的下一工具。当前未配置 `OPENAI_API_KEY`，浏览器如实显示 `DETERMINISTIC_FALLBACK`，不是 LLM 验收。
- 推荐保留完整三区候选池与游标；换批从当前 session 消费候选且不立即重复。Last.fm 遥测包含 tag 两层、Genre bridge、second-hop artist、请求预算和重试。
- Copy 执行已改为 run id + 状态查询 + SSE + cancel/resume；执行快照仍为后端进程内状态，不宣称跨进程恢复。
- `/compare` 已用真实临时文本完成浏览器验证；`/versions` 已绑定真实当前 Analysis，但真实版本搜索仍被 YouTube OAuth 阻塞。
- Spotify/YouTube 官方连接器为 `IMPLEMENTED BUT UNCONFIGURED / MANUAL_AUTH_REQUIRED`；Apple Music 没有本项目连接器，固定显示 `IMPORT_ONLY`；中国平台只显示 `IMPORT_ONLY` 或 `PARTNERSHIP_REQUIRED`。
- 单域名 HTTPS 部署参数、回调派生、安全 Cookie 与 `OAuthTokenStore` 抽象已准备；没有创建公网资源。Windows 本地使用 DPAPI，生产仍需托管 Token Store 实现。
- 本检查点没有新增平台、没有公开部署、没有执行 git push，也没有读取或输出 Secret。
