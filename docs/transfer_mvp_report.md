# Spotify ↔ YouTube Transfer MVP 报告

更新时间：2026-09-05  
最终状态：**IMPLEMENTED BUT UNCONFIGURED / AUTOMATED VERIFIED / REAL OAUTH NOT VERIFIED**

## 已完成代码

### 后端

- `backend/src/transfer.rs`：`TransferTrack`、候选、匹配、预览、执行请求/结果、来源与目标 Connector 抽象、确定性评分、版本回退、确认、逐首写入、取消和恢复核心逻辑。
- `backend/src/writers/spotify.rs`：官方 Authorization Code Flow、token 刷新、连接恢复、用户歌单和曲目分页。
- `backend/src/writers/youtube.rs`：官方 Google OAuth、频道/列表读取、查询重试、视频时长批量补充、新建私有列表与逐首插入。
- `backend/src/secure_store.rs`：Windows DPAPI 当前用户级加密 OAuth 会话存储，文件位于项目外且不会记录 token。
- `POST /api/transfers/preview`：按 `destination_platform` 使用真实 Spotify 或 YouTube Provider；Mock 只能由测试或明确开发验证选择。
- `POST /api/transfers/execute`：只有请求明确确认且所有歧义均已改选/跳过后才执行。

### 前端

- 独立 `/transfer` 页面显示 Spotify/YouTube 实际配置与连接状态。
- 来源支持 Spotify、YouTube、真实文件和当前真实分析；目标只列出具备官方 Writer 的 Spotify/YouTube，源歌单始终只读。
- 显示每首最多 5 个候选、确定性分数及标题/艺人/时长/官方/版本依据。
- Alternate fallback 默认关闭；开启后不同版本仍标为歧义并要求确认。
- 目标始终新建私有播放列表；显示逐首结果和汇总，可下载 CSV/JSON 报告。

## OAuth UX 与会话安全

- 用户只能从“连接 Spotify/YouTube”进入 auth-start endpoint，不需要也不应手动访问 callback。
- callback 缺少 code/state、用户取消或 state 失败时，后端只带安全错误码重定向回 MelodyPath，由 UI 显示可理解提示。
- state 一次性校验、access token 自动刷新、Disconnect 和失效后“重新连接”均在后端处理。
- token 不返回前端；浏览器只保存 HttpOnly、SameSite=Lax 的不透明 session。Windows 本地会话由 DPAPI 加密并存放在 `%LOCALAPPDATA%\MelodyPath`，不进入 Git、README、SQLite 或日志。

## 匹配规则

每首歌优先搜索 `艺人 歌名 official audio`。评分考虑规范化标题、艺人身份、双方都有真实值时的时长差、官方艺人/Topic/VEVO 信号以及版本语义。缺失时长保持 `None`，不按 0 分钟处理。Live、Cover、Karaoke、Remix、Sped Up、Slowed、Instrumental、Reaction 等版本在源曲未标明且未开启回退时会降权或排除。

匹配状态为 `MATCHED_HIGH`、`MATCHED_AMBIGUOUS`、`UNMATCHED`。歧义候选可以改选或跳过；未确认歧义和整个迁移未明确确认时均禁止创建目标。

## 自动测试

Transfer 自动测试覆盖：

- Spotify/来源 Connector 全分页读取；
- YouTube 候选最多 5 个；
- 标题、艺人、时长、官方来源与版本评分；
- Alternate fallback 必须主动开启且确认；
- 未确认禁止写入；
- 单首失败后继续；
- 取消；
- 从检查点恢复且不重复写入已完成曲目；
- Unicode/CJK；
- 500 首大输入。

当前最终检查：`cargo fmt --all --check`、`cargo check --workspace`、`cargo test --workspace`（105 passed）、`frontend/npm run lint`、`frontend/npm run build` 全部通过。

## 本轮浏览器 UX 验收

- 首页：中国平台只显示“导入文件或文本”，没有虚假账号 Connect；Spotify/YouTube 未配置时只显示配置向导。
- 直接访问 `/api/spotify/callback`：自动回到 MelodyPath 首页并显示“授权回调缺少授权码，请从连接按钮重新开始”，没有停留在 JSON 页面。
- `/compare`：Friend A/B 均有来源选择器；文件、粘贴和中国平台导入可用；受当前数据政策限制的 Spotify/YouTube 账号比较明确禁用并说明应上传导出文件。
- `/transfer`：显示 Spotify、YouTube、文件和已有分析来源入口；目标只列出 Spotify/YouTube 官方 Writer；本机两端均未配置，因此 Connect 和执行按钮禁用并标为 `IMPLEMENTED BUT UNCONFIGURED`，没有 Mock 冒充 Connected。
- 浏览器控制台没有 warning/error。

`cargo fmt --all --check`、`cargo check --workspace`、`cargo test --workspace`、`frontend/npm run lint` 和 `frontend/npm run build` 均通过。

## 本地浏览器验收

仅运行一套后端 `127.0.0.1:3000` 和一套前端 `127.0.0.1:5174`。上传 `testdata/Japanese.csv`：

- 文件解析：76/76。
- 真实 YouTube Provider：`BLOCKED_EXTERNAL_AUTH`；76 首保持未匹配，0 候选，没有切换 Mock。
- 显式 Mock：`MOCK_VERIFIED`；76 个 `MATCHED_HIGH`、0 歧义、0 未匹配。
- 未勾选确认时执行按钮禁用。
- 确认后的 Mock 执行：76 成功、0 失败、0 跳过、0 未匹配，进度 100%，源歌单修改为“否”。
- 结果链接为 `mock://youtube/playlist/...`，页面明确说明不代表真实 YouTube。
- 浏览器控制台无 warning/error。

## 真实 OAuth 边界

真实端到端状态仍是 **IMPLEMENTED BUT UNCONFIGURED**。尚未完成或无法在无人值守流程替代的项目包括：

- Spotify Client ID / Client Secret；
- Spotify Redirect URI 的开发者后台登记与用户登录授权；
- Google OAuth Client 配置；
- YouTube Data API v3 启用与可用 quota；
- Google Redirect URI 登记；
- 用户手动登录并授权 YouTube 频道。

因此没有读取真实 Spotify 测试歌单，没有向真实 YouTube 写入歌曲，也没有新 YouTube 播放列表链接。Mock 结果不得用来声称真实迁移成功。

## 当前实现限制

1. Copy 执行已使用独立 run id，并提供状态查询、SSE、取消、恢复及前端进度。run 登记当前仍在后端进程内；服务重启后需重新生成预览，不能宣称跨重启持久化。
2. UI 支持在返回的最多 5 个候选中改选或跳过；“重新输入查询并搜索”尚无独立后端端点。
3. Spotify 源过滤依赖当前官方 API 返回的可访问范围；真实账号下“拥有/协作”边界仍需人工核对。
4. YouTube 官方频道识别依赖 API 返回的 channel/title 信号，不是平台认证的完整权威身份图谱。
5. Windows DPAPI 依赖当前交互用户配置文件；在隔离 CI/服务账户不可用时不会降级为明文存储，而会要求重新连接。

## 用户回来后的最少人工步骤

1. 在 Spotify Developer Dashboard 和 Google Cloud Console 配置应用，启用 YouTube Data API v3，并登记上述本地 callback URL；Secret 只放入后端安全环境变量。
2. 重启后端，打开 `http://127.0.0.1:5174/transfer`，确认两端从“未配置”进入可连接状态。
3. 分别点击官方 OAuth，在平台页面手动登录和授权；不要在 MelodyPath 输入密码或 Cookie。
4. 选择一个自己拥有或协作的 5—10 首 Spotify 测试歌单，确认读取总数。
5. 用真实 YouTube Provider 生成预览，检查高置信/歧义/未匹配和错误版本；逐项改选或跳过歧义。
6. 勾选明确确认，创建新的私有 YouTube 播放列表；核对链接、逐首结果、CSV/JSON 报告和 Spotify 源歌单未改变。

预期通过标志：页面返回 `youtube.com/playlist?...` 的真实可访问链接，而不是 `mock://`，且用户在 YouTube 账号中看到新建的私有列表。

## 可直接继续的 Prompt

> 继续当前 MelodyPath 项目，只完成 Spotify → YouTube Transfer 的真实 OAuth 验收。先以布尔状态检查 Spotify 和 Google/YouTube 配置，不要读取或回显任何 Secret/Token；缺少配置时停止并准确列出缺少项。配置齐全后打开 `/transfer`，由我手动完成两端官方登录。只使用一个 5—10 首、我拥有或协作的 Spotify 测试歌单，核对完整分页读取、最多 5 个 YouTube 候选、歧义人工确认、新建私有列表、逐首结果、CSV/JSON 报告和源 Spotify 未修改。没有真实 YouTube 播放列表链接时不得声明通过，不要扩展其他平台或修改推荐算法。
