# MelodyPath platform handoff

最终收尾：2026-09-08。唯一工作目录 `C:\Users\user\Downloads\MelodyPath`，分支 `feature/more-platforms`。

## 最终交接状态

- 本轮基线 `ae4b1a7`（Complete multi-platform capability support），开始时工作区干净，分支与 diff 检查通过。
- 最终本地提交标题：`Finalize multi-platform support and UX`。提交 hash 可通过 `git log -1 --format=%H -- PLATFORM_HANDOFF.md` 取得，避免文件内自引用。
- 能力矩阵已冻结；本课程接受 Apple 的已实现、需配置、未真人验收状态，不购买 Apple Developer Program，不等待凭据，不将 Apple 真人验收作为合并条件。
- 本轮未研究新 API，未新增平台、OAuth 或功能；未操作 `feature/public-deployment`。

## 冻结能力矩阵

| 平台 | 当前能力 | 最终状态 |
|---|---|---|
| Spotify | 官方 OAuth、账号歌单读取、公开歌单 URL 导入、文件/文本导入；API 读取需要配置和授权 | Official API · REAL VERIFIED（既有账号与曲目读取验收） |
| YouTube / YouTube Music | 官方 OAuth、账号歌单读取、公开歌单 URL 导入、文件/文本导入；API 读取需要配置和授权 | Official API · REAL VERIFIED（既有账号与曲目读取验收） |
| Apple Music | 官方公开目录歌单读取代码已实现，缺配置时仍可文件/文本导入；私人资料库未支持，写入未支持 | OFFICIAL API IMPLEMENTED · CONFIG REQUIRED · NOT REAL VERIFIED |
| 网易云音乐 | 文件/文本导入、公开链接识别、公开可访问性检查；没有完整歌单 API 曲目读取 | FILE/TEXT IMPORT · URL recognition · accessibility check |
| QQ音乐 | 文件/文本导入、公开链接识别、公开可访问性检查；没有完整歌单 API 曲目读取 | FILE/TEXT IMPORT · URL recognition · accessibility check |
| 酷狗音乐 | 文件/文本导入、URL 识别 | FILE/TEXT IMPORT · URL recognition |
| 汽水音乐 | 文件/文本导入、URL 识别 | FILE/TEXT IMPORT · URL recognition |
| Last.fm | 推荐 Provider，保留五份真实文件的既有验收记录 | REAL VERIFIED recommendation provider |

Spotify / YouTube 的真人验收限于既有读取记录，本轮没有重做登录或验收真实写入。自动测试和 Mock 不构成 Apple 或任何新平台的真人验收。

## 最终 UX 与文档

- 七张卡片沿用原结构、badge 样式、主题与响应式布局，主要状态和能力统一为自然中文。
- 已连接状态来自后端会话；Spotify 保留“连接 Spotify”入口。账号未连接时不会假装已连接。
- Apple 显示“官方 API · 需部署者配置”，说明公开目录代码已实现、尚未真人验收、私人资料库未支持。普通用户使用“导入文件或文本”；公开 URL 缺配置时显示友好解释，不展示等待凭据枚举或要求用户申请 Token。
- 统一提示“MelodyPath 不要求用户提供账号密码或 Cookie。”公网 Demo 普通用户无需开发者凭据；自行部署的真实 Spotify / YouTube / Apple Music / Last.fm 能力由部署者配置。
- README、平台审计与本交接文件使用同一冻结矩阵。

本轮修改仅五个文件：`frontend/src/App.tsx`、`frontend/tests/oauth-import.spec.ts`、`README.md`、`docs/platform_capability_audit.md`、`PLATFORM_HANDOFF.md`。后端、CSS、锁文件与 Playwright 配置未修改。

## 保留的实现边界

- Apple Token 仅后端使用，固定官方 API origin、禁重定向、限制分页；所有页成功才返回完整曲目。逐源条目报告明确记录跳过原因。
- 文件入口共用现有解析器与确认预览，保留 UTF-16、TAB TXT 与时长解析；不声称各平台均提供官方统一导出格式。
- 平台 API 数据不进入推荐、画像或 LLM；源 Spotify 歌单不修改。写入前仍需显式确认，Copy 恢复仅限单进程，不跨重启。
- 未读取、输出或提交真实凭据、Cookie、数据库或私人歌单。

## 最终验证（2026-09-08）

| 命令 | 结果 |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo check --workspace` | PASS；保留既有 dead_code 警告 |
| `cargo test --workspace` | 136 passed / 0 failed / 1 ignored（既有显式联网探测） |
| `cargo build --release --locked` | PASS |
| `npm --prefix frontend run lint` | PASS |
| `npm --prefix frontend run build` | PASS |
| `npm --prefix frontend run test:e2e` | 16/16 PASS，正常退出，35.2 秒 |

文案断言与新中文展示同步，并增加不显示内部枚举、Apple 文件入口和私人资料库限制的断言；没有删除或放宽功能、安全及确认闸门断言，没有增加 timeout。

## 用户后续操作

在本课程冻结范围内建议用户手动 merge main。Apple 的需配置、未真人验收是已接受的最终限制，不是待办或阻塞。自动任务只创建最终本地提交并确认工作区干净；push 和 merge 由用户自行完成。无需继续 API 研究或等待 Apple Credentials。
