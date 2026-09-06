# MelodyPath 开发交接

更新时间：2026-09-04

## 当前结论

项目没有停在设计稿：真实导入、本地分析、Last.fm 推荐实现、临时双歌单比较、Alternate Versions、Rust Agent Loop，以及 Spotify → YouTube Transfer 的本地/Mock 闭环都已存在。当前最重要的工作是准备并完成真实 Spotify 与 Google/YouTube OAuth 端到端验收，不是重写项目，也不是扩展更多音乐平台。

状态必须使用以下口径：

- **IMPLEMENTED**：代码与自动测试存在。
- **MOCK VERIFIED**：显式 Mock 流程通过，只证明流程。
- **BROWSER VERIFIED**：本地浏览器交互已验证。
- **BLOCKED_EXTERNAL_AUTH / CONFIGURATION**：缺少外部配置、额度或用户授权。
- **REAL API VERIFIED**：只有真实 Provider/账号结果可使用此标签。

## 已完成能力

### 本地分析与推荐

- CSV、TSV、JSON、TXT、M3U/M3U8 和批量文本先解析预览，再由用户确认分析。
- `NONE / REAL_FILE / REAL_TEXT / DEMO / ERROR` 隔离；真实失败永不自动回退 Demo。
- Rust 基础统计、Genre/Energy 覆盖率、元数据完整/部分/缺失统计。
- 统一 Genre、艺人身份、标题和录音版本规范化。
- `LastFmRecommendationProvider`、真实种子选择、track/artist/tag 候选来源、Rust 确定性评分、三区和逐阶段统计。
- 原歌单排除、派生版本过滤、规范化去重、同艺人上限；Apple 只补充目录元数据，不生成真实推荐候选。

### Agent 与社交场景

- Rust Agent 的 Goal、Intent、Plan、Step、ToolCall/Result、状态、重试、最大步骤、取消、恢复和 SQLite 检查点。
- 独立 `/compare`：两份真实输入分别预览，临时计算 Track/Artist/Genre/Tag/多样性指标，双方源歌单排除，无候选时不补 Demo。
- Alternate Versions：用户主动选择 Live、Concert、Remix、Acoustic、Unplugged、Remaster；真实 YouTube OAuth 与明确 Mock 严格分开。
- 推荐、共同推荐和 Alternate 候选均可进入共用 Playlist Writer 的预览—确认流程。

### Transfer MVP

- 独立 `/transfer` 页面与 `backend/src/transfer.rs`。
- 只读来源支持 Spotify、用户文件或当前真实分析；目标只支持 YouTube。
- `TransferTrack` 包含原始与规范化标题/艺人、专辑、时长、ISRC、来源平台/ID/URL。
- YouTube 查询使用“艺人 + 歌名 + official audio”，每首最多保留 5 个候选。
- 确定性评分拆分标题、艺人、真实时长、官方/Topic 信号和版本一致性；缺失时长保持 `None`。
- `MATCHED_HIGH / MATCHED_AMBIGUOUS / UNMATCHED`；版本回退默认关闭，开启后仍需逐首确认。
- 未明确确认时后端禁止创建目标；目标只能新建私有列表；单首失败继续；源 Spotify 永不写入。
- CSV/JSON 结果报告；核心执行器具有取消和从既有列表恢复且不重复写入的自动测试。
- 本地浏览器已用 76 首真实文件验证显式 Mock 闭环：76/76 写入、0 失败、`mock://` 链接；真实 YouTube 路径正确返回 `BLOCKED_EXTERNAL_AUTH`。

## 当前限制与不得夸大的能力

1. 当前环境没有可供本次进程使用的 `LASTFM_API_KEY`，修复后的 Japanese/Korean/English/Chinese 四文件真实推荐回归未重跑。历史真实结果只能作为修复前基线。
2. Spotify 和 Google/YouTube 开发者配置、额度与人工授权未提供，真实 Transfer 没有产生 YouTube 播放列表链接。
3. Transfer HTTP 执行目前是同步请求；取消/恢复已在核心执行器中测试，但前端还没有持久 transfer session、状态轮询/SSE、取消和恢复端点。不要宣称端到端实时中断已完成。
4. Transfer 页面尚未实现歧义项的“重新搜索”专用后端端点；可改选已有候选或跳过。真实 OAuth 验收前应决定是否补齐。
5. `/compare` 在 Last.fm 不可用时能完成真实确定性比较，但共同推荐会诚实为空。
6. Alternate Versions 的真实搜索和真实写入均依赖 Google OAuth；显式 Mock 不是平台验证。

推荐修复详情见 `docs/recommendation_acceptance_report.md`。任何缺少 Provider、OAuth、额度或人工确认的步骤都必须停在明确阻塞状态。

## 关键代码

- `backend/src/import.rs`：统一文件/文本导入。
- `backend/src/normalize.rs`、`identity.rs`、`genre.rs`：标题、版本、艺人和 Genre 规范化。
- `backend/src/recommendation.rs`：Last.fm Provider、种子、候选、过滤、评分和统计。
- `backend/src/engine.rs`：本地分析、Compare 和桥梁约束。
- `backend/src/agent.rs`：Agent 状态机、工具循环和持久化。
- `backend/src/alternate.rs`：版本探索与确定性筛选。
- `backend/src/transfer.rs`：Transfer 模型、预览、评分、确认和执行器。
- `backend/src/writers/spotify.rs`：Spotify OAuth、会话、分页和 writer。
- `backend/src/writers/youtube.rs`：Google OAuth、YouTube 搜索/时长/列表写入。
- `backend/src/main.rs`：REST、SSE、配置状态和确认边界。
- `frontend/src/App.tsx`：当前包含 `TransferPage`、`ComparePage` 和 `AlternateVersionsPanel` 组件及路由切换。
- `frontend/src/ExportModal.tsx`：共用写入预览/确认。

## 下一位开发者的实施顺序

1. 阅读 `README.md`、`PLAN.md`、`AGENTS.md` 和 `docs/` 中相关报告；先运行全量检查，保持 84 个 Rust 测试与前端构建通过。
2. 用只返回“存在/不存在”的方式检查 OAuth 环境配置，不输出 Client Secret、Token 或 Key。
3. 在没有账号授权的情况下，只补齐可独立验证的 Transfer session、状态查询、取消/恢复 API 和歧义重新搜索；不得触碰推荐架构。
4. 运行 Mock 与浏览器回归，确认真实按钮仍会在配置缺失时返回 `BLOCKED_EXTERNAL_AUTH`，不会自动切 Mock。
5. 配置齐全后，由账号持有人手动完成 Spotify 和 Google 官方登录。不要代填密码、验证码或创建/显示 Secret。
6. 只选一个 5—10 首、用户拥有或协作的 Spotify 测试歌单；核对分页读取总数和每首最多 5 个 YouTube 候选。
7. 人工处理所有歧义/版本回退，确认后创建新的私有 YouTube 播放列表；核对逐首结果和源歌单未改变。
8. 只有拿到可访问的真实 YouTube 播放列表链接，才把 Transfer 标成 **REAL API VERIFIED**。

## 真实 OAuth 人工配置

### Spotify

- Spotify Developer Dashboard 中的应用、Client ID/Client Secret。
- Redirect URI 与后端 callback 完全一致：本地默认 `http://127.0.0.1:3000/api/spotify/callback`。
- 读取私人/协作歌单所需 scope。
- 账号持有人在 Spotify 官方页面登录并授权。

### Google / YouTube

- Google Cloud 项目启用 YouTube Data API v3，并有足够 quota。
- OAuth consent screen、Web OAuth Client、Client ID/Client Secret。
- Redirect URI 与本地 callback 完全一致：`http://127.0.0.1:3000/api/youtube/callback`。
- 账号持有人在 Google 官方页面登录并授权当前 YouTube 频道。

凭据只放在后端进程可读的安全环境变量中。Token 当前仅保存在后端内存，会话 cookie 为 HttpOnly；服务重启后需要重新授权。

## GitHub 安全清单

不得提交或附加：API Key、Client Secret、access/refresh token、Cookie、`.env`、`melody_path.db`、token/cache 文件、运行日志中的敏感内容、用户私人歌单或未脱敏迁移报告。提交前只列文件名检查疑似秘密，不在终端或报告中输出值；确认 `.gitignore` 覆盖本地数据库、环境文件、构建目录和私人 CSV。
