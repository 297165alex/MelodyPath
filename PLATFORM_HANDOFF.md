# MelodyPath platform handoff

更新：2026-09-07。唯一工作目录 `C:\Users\user\Downloads\MelodyPath`，分支 `feature/more-platforms`。

## 恢复时的真实状态

- 本轮基线 HEAD：`c1bb17f`（Document verified Spotify and YouTube imports）。本次实现与此交接文件一并提交，提交标题 `Complete multi-platform capability support`；当前交接提交可用 `git log -1 --format=%H -- PLATFORM_HANDOFF.md` 精确取得，避免在文件内写入自身提交 hash 造成自引用。
- 已执行 branch/status/diff-check/log，分支正确，`git diff --check` 无错误；只有 CRLF 提示。
- 以下内容根据当前工作区、实际命令输出记录，不以历史 Mock 替代真人结果。

## 当前能力

| 平台 | 当前 capability | 真人状态 |
|---|---|---|
| Spotify | 官方 OAuth、账号读取、授权 URL 曲目读取；配置与 session 决定运行可用性 | 既有 README 记录读取 REAL VERIFIED；本轮未重做真人登录；真实写入未验收 |
| YouTube | 官方 OAuth、账号读取、授权 URL 曲目读取 | 同上；本轮不改变 scope/state 或 reader |
| Apple Music | 官方公开目录读取代码、URL/ID 校验、分页、Track / TransferTrack、现有 Import Preview；缺配置 CONFIG_REQUIRED | WAITING_FOR_APPLE_DEVELOPER_CREDENTIALS；本轮未真人验收；私人资料库/写入未接入 |
| 网易云 | FILE_IMPORT_AVAILABLE + ACCESSIBILITY_CHECK_ONLY | 未核实本项目适用的通用官方歌单接口，不声称 API 真人验收 |
| QQ音乐 | FILE_IMPORT_AVAILABLE + ACCESSIBILITY_CHECK_ONLY | 同上 |
| 酷狗 | FILE_IMPORT_AVAILABLE + URL_RECOGNITION_ONLY | 同上；分享 ID 格式未核实，不猜测解码 |
| 汽水 | FILE_IMPORT_AVAILABLE + URL_RECOGNITION_ONLY | 同上；仅已知官方域名 |

## 已实现与安全边界

- Apple Token 仅后端使用，旧 bootstrap 不返回 Token。目录 API 固定官方 origin，禁重定向，分页仅同歌单 tracks 路径；响应体/错误不泄漏凭据。
- 所有页完成才返回曲目；任何页失败/超限/超时，以及 HTTP 200 中含资源 errors，均不返回半份歌单。每页 4 MiB，最多 10,000 行/200 页，整体 90 秒。
- 每个 Apple 源条目都有报告；重复 ID、缺少艺人、缺元数据、非歌曲类型明确跳过；缺时长/URL 保持 None。无 playParams 仅标 UNKNOWN，不猜测全球不可用。
- Spotify/YouTube URL 预览增补相同 import_rows 报告字段，reader 与 OAuth 文件未修改。平台 API 数据仍不进入推荐/画像/LLM。
- 文件入口继续共用解析器与预览，增加带 BOM UTF-16 解码、TAB 表头 TXT 与明确时钟格式时长。没有声称中国平台原生支持 CSV/JSON 导出；XML/HTML 未接入。

## 修改文件

- `backend/src/writers/apple.rs`：目录 API、配置、分页、规范化及测试。
- `backend/src/platforms.rs`：capability、Apple URL 识别、平台边界测试。
- `backend/src/models.rs`：CONFIG_REQUIRED、逐项报告模型。
- `backend/src/main.rs`：既有链接预览连接 Apple、统一报告、缺配置 HTTP 测试。
- `backend/src/import.rs`：桌面导出 TAB TXT/时钟时长及测试。
- `frontend/src/App.tsx`、`types.ts`、`importFile.ts`：平台卡片、导入报告、文件编码。
- `frontend/tests/oauth-import.spec.ts`：保留既有回归，增加 Apple 配置/预览确认和 UTF-16。
- `docs/platform_capability_audit.md`：官方来源、能力、费用/资格不确定性、人工步骤、README 建议。
- `docs/platform_setup.md`、`docs/platform_capabilities.md`：更新 Apple 配置和历史矩阵索引。
- `PLATFORM_HANDOFF.md`：本文件。

README、Spotify/YouTube OAuth 源文件、依赖锁未修改；未查看真实凭据、私人歌单、数据库、Cookie 或运行日志。

## 最终测试状态

- cargo fmt --all --check：PASS。
- cargo check --workspace：PASS，有既有 dead_code 警告。
- cargo test --workspace：最新 136 passed / 0 failed / 1 ignored，忽略项为原有显式联网探测。
- cargo build --release --locked：PASS，最终代码复编完成（1m 07s）。
- npm --prefix frontend run lint：PASS。
- npm --prefix frontend run build：PASS。
- npm --prefix frontend run test:e2e：16 passed（36.8s），退出码 0。

原先沙箱内运行 15 个用例均通过，但 Windows Playwright `taskkill` 子进程收尾受限，导致命令挂起。经用户批准只清理本次测试的 npm/Vite 子进程后，原命令退出码 0。随后获准在沙箱外完整重跑最终 16 个用例，正常退出。未修改 Playwright 配置、断言强度、timeout 或生产数据来回避问题。

完整 diff 已审查：无半写逻辑/新 compile error/临时 debug/生产替代歌曲；新增 Token 仅后端使用；测试中的 synthetic 响应只存在于测试范围。dead_code 警告来自既有模块，无新增 Apple 死代码。Spotify、YouTube、Last.fm 核心源文件与 README 均无 diff。

## Apple 人工配置与下一步

自动检查与完整 diff 审查完成。按用户授权创建一个本地提交；不得 push/merge/rebase，也不得操作 feature/public-deployment。建议在 Apple 公开目录真人验收后再决定 merge main，本轮不将新平台标为 REAL VERIFIED。

Apple 需要合资格部署者的 Developer Program / Media ID / MusicKit key。通常 USD 99/年，不要求用户现在付款。Token 在仓库外生成并通过后端环境变量 `APPLE_MUSIC_DEVELOPER_TOKEN` 提供；Team ID/Key ID 用于外部生成，私钥和 Token 都不能粘贴到聊天或前端。只读配置不等于 Apple 验证凭据。当前执行进程仅通过 Test-Path 检查变量存在性，结果 false，没有读取值。

用官方 Apple Music 页面复制的带地区公开歌单 URL 测试，核对真实名称、完整分页条数、Unicode、重复与跳过条目；成功应出现 TRACK_IMPORT_AVAILABLE 和 Import Preview，仍需点击确认才进入既有 Copy 预览。私人资料库和最终 Apple 写入不在本轮范围。全部自动测试不等于 Apple REAL VERIFIED。

完整官方依据与限制见 [平台审计](docs/platform_capability_audit.md)。后续只处理真实凭据/授权验收和明确发现的错误，不重建推荐模块。
