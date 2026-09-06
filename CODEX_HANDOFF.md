# MelodyPath 继续工作交接

更新：2026-09-07。唯一项目目录：`C:\Users\user\Downloads\MelodyPath`。
当前分支：`feature/oauth-public-links`。

## 当前完成阶段

无人值守代码收尾完成，127 Rust tests 与 11 Playwright tests 通过，等待真人 YouTube 重新授权、两个平台真实 URL 导入和最终 Copy 验收。
未修改 README、推荐、Compare、Music Profile；未 push、merge、rebase、force；未删除工作区、回滚成果或执行真实歌单写入。没有用 Mock 冒充真实成功。

## 本地 commits

- `3b9b6c81b96b0620755d9ab81c296d0e7d362662`：前轮 OAuth 浏览器 state 绑定、安全错误与 capability 状态。
- `64489ffdbf1c216c233e95fc9ddddad1b9f8063b`：本轮最小 OAuth scopes、官方授权链接导入、前端回调与 Spotify picker、测试及审计文档。
- 本文件随后单独保存为交接 commit；交接提交本身的最新 hash 用 `git log -1 --format=%H` 获取。

## 本轮修改文件

- `.gitignore`：忽略 Playwright 测试产物。
- `backend/src/main.rs`：读/写分离授权入口、匿名/授权 public link 路径、401 AUTH_REQUIRED、安全回调阶段日志和 /me 会话布尔值。
- `backend/src/models.rs`：连接状态增加 write_authorized。
- `backend/src/writers/spotify.rs`、`backend/src/writers/youtube.rs`：最小 scopes、保存/刷新 granted scopes、写入权限闸门、官方 URL 读取共用分页、明确 synthetic HTTP 测试。
- `frontend/src/App.tsx`：回调后核验 session、显式 Connected/Error、平台状态失败不会被其他平台阻塞、Spotify 固定 footer、公开链接 Import Preview 进入 Copy 确认。
- `frontend/src/ExportModal.tsx`：Copy 写入单独授权说明和入口。
- `frontend/src/styles.css`、`frontend/src/types.ts`：弹窗布局、状态字段。
- `frontend/package.json`、`frontend/package-lock.json`：Playwright 与真实检查应用的 lint 命令（原顶层 tsc --noEmit 不检查引用的应用代码）。
- `frontend/playwright.config.ts`、`frontend/tests/oauth-import.spec.ts`：隔离合成数据 UI 回归，使用本机 Edge headless / 测试端口 5180。
- `docs/oauth_scope_audit.md`：scope 官方依据、回调证据边界、人工步骤。
- `CODEX_HANDOFF.md`：本交接。

## OAuth scopes 与真实状态

Spotify 默认：`playlist-read-private playlist-read-collaborative`。
Spotify Copy 显式授权才添加：`playlist-modify-private`。
删除了非必需 user-read-private 和公开歌单写入 scope；基本 identity 不申请邮件等额外信息。

Google 默认：`https://www.googleapis.com/auth/youtube.readonly`。
Google Copy 显式授权才添加：`https://www.googleapis.com/auth/youtube.force-ssl`。
Google `include_granted_scopes=false`。宽泛 consent 文案来自 force-ssl，官方没有仅限歌单插入的更窄写 scope；没有谎称写入授权本身已变窄。请求缩小不自动撤销旧 grant，必要时用户手动移除旧 Google 授权后重新连接。旧存储没有 granted_scopes 时按无写权限处理，不阻断已有只读 session。

Spotify：用户已明确确认真人官方 OAuth、callback 和真实歌单读取成功。本轮保留该验收结果；新 URL 导入与写入尚未真人验收，不需要无故重做 Spotify 授权。若已过期则等待重新连接。
YouTube：用户已点击 Google consent Continue，但此前 callback/code/state/exchange/session/cookie/identity/playlists 无充足可访问历史证据，不宣称成功。严禁读取用户 Cookie、token store、数据库、私人歌单来追溯。
状态：`READY_FOR_YOUTUBE_REAUTH` / `WAITING_FOR_USER_GOOGLE_AUTH`。
真实歌单选择与最终写入：`WAITING_FOR_USER_PLAYLIST_CONFIRMATION`。

新增诊断仅记录阶段与布尔值：callback_received/code_present、state_validated、token_exchange_succeeded、token_and_identity_validated、session_saved、callback_redirect 的安全 reason、session_status 的 cookie 是否存在及 connected。绝不记录 code/state/token/cookie 或歌单内容。
回调保存会话前已官方读取频道 identity；/me connected 证明会话存在、有效期及保存的身份，不证明当前 playlist read 或 Copy 成功。

## Public links

Spotify / YouTube：代码已支持合法 URL → ID → 官方 API metadata/playlistItems 分页 → Import Preview。需要合法 session；未授权/过期返回 AUTH_REQUIRED，失败不生成歌曲、不自动 Demo。
账号 picker 与 URL 导入复用读取器；已授权 URL 不要求必须出现在 mine 列表，具体可访问性仍由官方 API 决定。
平台曲目只用于传输，不进入分析、画像、推荐或 LLM。Preview 后需用户确认才进入 Copy；实际写入还有独立确认。
**两平台真实 URL 导入尚待真人验收**，首页保持 Experimental。
其他平台：Apple/酷狗/汽水 URL_RECOGNITION_ONLY；网易云/QQ ACCESSIBILITY_CHECK_ONLY（可访问性依平台实际响应）；未知平台 UNSUPPORTED。没有私有 API、Cookie、模拟登录或非官方曲目抓取。

## 已运行测试及结果

最终源代码：
- `cargo fmt --all --check` PASS。
- `cargo check --workspace` PASS；仍有既有/辅助测试方法的 dead_code warnings，没有编译错误。
- `cargo test --workspace --quiet` **127 passed / 0 failed**。
- frontend `npm run lint` PASS（现在分别检查 app/node tsconfig）。
- frontend `npm run build` PASS。
- frontend `npm run test:e2e` **11 passed / 0 failed**，明确 synthetic fixtures，不能称真人 OAuth 成功。
- `git diff --check` PASS。

覆盖：state 伪造/过期/复用、callback 安全错误、空 token/无 identity 拒绝、session/disconnect/refresh、精确 scopes、readonly create/add 阻断、合法授权 URL 分页、无 session/过期/invalid ID/读取失败、前端 query 不伪造 connected、可见 callback 错误、1280×720 和 390×600 picker footer/选择数/disabled、public AUTH_REQUIRED/预览确认、无 analyze/execute 隐式调用、Failed to fetch 友好提示。
首轮 UI 测试暴露新增 JSX 缺少闭合括号（已修复）；后续新增预览测试 fixture 缺少 genres 字段（已补齐）；最终全绿。
浏览器 CUA 初始化工具此前失败，但独立 Playwright 已可用，没有因此停止。

最新运行进程 HTTP 核对：
- 3000 /health 和 5174 首页均 200。
- Spotify/Google authorize 默认与 write=true 都 307 到官方域名，实际 scopes 与上述精确一致。
- 匿名 Spotify / YouTube public link 均 AUTH_REQUIRED、0 tracks、can_analyze=false。匿名结果不是对用户浏览器 session 的判定。
- 只输出状态、scope、官方域名，未输出 Client ID/Secret、code/state 或 Cookie。

## 当前服务

本轮核对旧 PID 18064 的路径属于本项目后，以无 Force 的 Stop-Process 停止并通过以下脚本重新启动最新版本。已有加密 session 文件未读取/修改/删除，由应用正常加载。
后端：`http://127.0.0.1:3000`；前端：`http://127.0.0.1:5174/`。
后端 exec session 15972（仅本会话参考，恢复后可能不可用）。前端原服务保留。所有新授权应从 5174 首页按钮开始，callback 使用既有 3000 地址。
运行状态可能随桌面会话结束变化，恢复时先做 health 检查。

## 明早人工步骤 / 尚未完成

1. 打开 http://127.0.0.1:5174/，开始真人 **YouTube 只读重新授权**。若仍见旧广泛授权，先在 Google 账号第三方连接移除旧 MelodyPath grant，再从首页连接。不是 Copy 入口，不请求写权限。
2. 回来后确认明显 YouTube Connected；点击“选择我的播放列表”确认真实官方读取。若 Error，保留安全错误文字，不发送 token/code/cookie。
3. 在当前合法浏览器 session 下分别粘贴自己有权限的 Spotify 与 YouTube playlist URL，核对真实 Import Preview；不要把合成测试当验收。
4. 若要完成端到端 Transfer，再主动点击 Copy 的写权限授权，选择一个 Spotify 自有/协作测试歌单，核对候选并逐首确认/跳过。最终明确确认后才新建私有 YouTube 播放列表。需真实链接与写入结果才能完成迁移验收。
5. 不建议现在 merge main；等待以上真人验收。继续禁止 push/merge/rebase/force，除非用户后续明确改变要求。

## 下一步具体命令

```powershell
Set-Location -LiteralPath 'C:\Users\user\Downloads\MelodyPath'
git status --short
git branch --show-current
git log -2 --oneline
Invoke-WebRequest -Uri 'http://127.0.0.1:3000/health' | Select-Object StatusCode
Invoke-WebRequest -Uri 'http://127.0.0.1:5174/' | Select-Object StatusCode
```

若服务未运行，仅在相应端口空闲时启动：

```powershell
.\start-backend.ps1 -FrontendUrl http://127.0.0.1:5174
# 另一 PowerShell：
Set-Location -LiteralPath 'C:\Users\user\Downloads\MelodyPath\frontend'
npm run dev -- --port 5174 --strictPort
```

无新代码变化无需重复全套；修改后按阶段运行：

```powershell
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
npm --prefix frontend run lint
npm --prefix frontend run build
npm --prefix frontend run test:e2e
```

## Git status 快照与恢复入口

代码提交 64489ff 后快照：仅 `?? CODEX_HANDOFF.md`，其他工作区清洁。本交接单独提交后应为空；以恢复时 `git status --short` 为准，保留任何后续用户改动。
下次用户只需发送“读取 CODEX_HANDOFF.md，从上次停止的位置继续。”；先读本文件和 docs/oauth_scope_audit.md，从“明早人工步骤”核验 YouTube session/真实 playlist read 开始。代码与自动化收尾已完成，仍缺真人授权与真实目标链接，不能报告整体真实迁移已通过。
