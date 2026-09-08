# 网易云公开歌单 HTML 导入

日期：2026-09-08。本次仅使用匿名官方公开 HTML / JSON-LD；无账号密码、Cookie、自动登录、私有 API、签名接口或客户端协议。属于条件性公开页面解析，不是官方 OAuth/API 接入，也不保证任意公开歌单可导入。

## 数据流与能力

移动/主站 URL 校验 → 完整公开歌单成员校验 → 公开歌曲元数据 → 既有 StoredImport / Import Preview → 用户确认 → 既有 MetadataResolver → Recommendation。

- 复用精确 HTTPS 主机、数字 ID、canonical URL 与同平台重定向限制。
- 页面 HTTP 200 / HTML / 完整 UTF-8 响应，上限 1 MiB；沿用 7 秒页面请求超时。无 Range。截断、错误、超限均不解析。
- 使用 HTML DOM 解析器，只读取公开 JSON-LD 与 `#song-list-pre-cache[data-key=track_playlist-ID] ul.f-hide` 的歌曲链接。忽略推荐区、脚本、隐藏编码载荷和 hydration data。
- JSON-LD 必须为同 ID 的 MusicPlaylist，声明数量在 1–10,000。完整 JSON-LD 可直接使用；否则要求 DOM 数量与声明一致、歌曲 ID 不重复，并与 JSON-LD 前缀顺序一致。无法验证完整成员列表时，整个 URL 保留 ACCESSIBILITY_CHECK_ONLY，不发起详情补全，不生成 Track。
- 若成员列表完整，优先使用同页 JSON-LD 已提供的标题/艺人/专辑/时长。缺艺人的已知 ID 可查询固定 `https://music.163.com/song?id=ID` 官方页面；不搜索、不猜歌曲。
- 详情独立匿名客户端、禁止重定向、每页最多 1 MiB / 3 秒；每次导入最多 20 个详情请求、详情批次最多 20 秒、串行请求间隔 150 ms。无需详情的完整条目不占请求数量。
- 详情 JSON-LD 必须声明同 ID 的 MusicRecording。标题与艺人必需，专辑/时长可缺；ISO 时长明确转换为毫秒，无效值为 null。失效、超时、请求上限不丢弃已成功曲目，不导致整个导入失败。

完整成员列表与详情成功率是两件事：完整列表已知时，详情只成功 X/Y 可以导入 X 首，并展示每个未导入项；成员列表不完整时不把可见的少数歌曲当成完整歌单。

## 统一模型与预览

沿用已有 Track：`title`、`artists`（请求中的 artist）、`album`、`duration_ms`、`platform=netease`（source_platform）、`platform_url`（source_url）。不新增另一套 Track。原始来源 URL 进入 ImportedTrack，转换后还保存在 external_ids 的 source_platform/source_url 中，供既有 Resolver 克隆时保留来源。

成功时服务端创建 StoredImport，返回 import_preview，前端自动展开原 ImportPreviewPanel。标记 REAL_PUBLIC_LINK，total_rows=声明数量，parsed_count=成功数量，invalid_count=未导入数量。用户确认后使用原 `/api/imports/:id/analyze`，不进入 Copy，不把 URL 伪装成本地文件。

`import_rows` 保留每个成员：IMPORTED、SKIPPED_DETAIL_UNAVAILABLE、SKIPPED_REQUEST_LIMIT、SKIPPED_TIME_LIMIT。原始声明数量 track_count 不是成功数量；界面明确显示 NetEase playlist detected、Tracks imported、Imported X / Y tracks 和未导入数量。全失败时 can_analyze=false、ACCESSIBILITY_CHECK_ONLY，显示 Playlist recognized but tracks unavailable。

未修改 Spotify/YouTube/Apple connector、MetadataResolver 核心或 Recommendation 算法。metadata.rs 只扩展既有 ImportedTrack → Track 的来源字段转换及对应测试。

## 真实公开样本与限制

- [15 首普通歌单](https://music.163.com/playlist?id=7299150850)：历史及本轮前置分析观察到 JSON-LD / HTML 仅 10 项，仍应返回 ACCESSIBILITY_CHECK_ONLY。
- [200 首公开歌单](https://music.163.com/playlist?id=3778678)：前置实测 JSON-LD 30 项、DOM 200 个歌曲 ID；可作为完整成员列表 + 有限详情补全的真实验收样本。它不证明所有歌单页面稳定。
- [官方歌曲页](https://music.163.com/song?id=3399839173)：前置实测 MusicRecording 提供标题、艺人、专辑和 ISO 时长。

公开页面可能改变结构、地区可见性或数量。解析器严格失败降级，不声称官方授权 API 或对所有页面的读取保证。Last.fm 未配置时现有推荐流程明确返回 not_configured，不以 Demo 冒充真实推荐。

## 测试入口

- cargo test --workspace
- cargo test --workspace netease_real_public_page_acceptance -- --ignored --nocapture（不完整页面降级）
- cargo test --workspace netease_real_public_import_preview_and_analysis -- --ignored --nocapture（真实 HTTP 路由 → 存储预览 → 用户确认端点 → Resolver / Recommendation；仅输出聚合统计）
- npm --prefix frontend run lint / build / test:e2e

离线测试覆盖 DOM 容器归属、推荐区排除、HTML 实体、同页完整 JSON-LD、重复/数量/顺序/身份冲突、恶意 URL、可选字段 null、单项失败隔离、请求预算、批次超时及来源字段保留。前端合成契约验证部分导入、REAL_PUBLIC_LINK、确认前不分析、确认后进入原分析与推荐页面；合成测试不冒充平台验收。

## 本轮验收结果

- 真实 200 首样本：生产 HTTP handler 返回 Imported 20 / 200 tracks、180 项未导入；服务端创建真实预览，确认分析端点返回 20 首非 Demo 曲目及 20 条 Resolver 结果。详情读取受 20 请求上限限制，没有声称导入全部 200 首。
- 既有 Recommendation pipeline 已执行，结果为 `not_configured`；本次进程未配置 Last.fm。没有声称外部推荐成功，没有用 Demo 或固定候选替代。真实 HTTP → Preview → Analysis 测试 PASS，耗时约 43 秒（包括既有元数据分析）。
- 真实 15 首样本：`declared=Some(15); status=public_track_list_incomplete; imported=0`，PASS。
- `cargo test --workspace`：158 passed / 0 failed / 3 ignored。两项网易云联网测试均已显式单独执行通过；另一项 ignored 是既有 YouTube 联网探测。
- `cargo fmt --all --check`、前端 lint、build：PASS。
- 前端 E2E：全量 32/32 PASS。后续增加未导入原因中文展示的断言，并专项复核。浏览器验证使用合成 API 契约；真实数据验收通过后端 HTTP 路由执行，二者不混称真实浏览器平台验收。
- 中途修复新增测试的类型名、更新与新功能冲突的旧文案断言；Windows 曾因联网测试正在运行而锁住测试可执行文件，等待退出后串行重跑通过。未放宽业务断言或测试 timeout。
