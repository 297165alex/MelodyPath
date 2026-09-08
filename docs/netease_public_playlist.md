# 网易云公开歌单 HTML 导入

日期：2026-09-08。本功能只使用匿名访问的网易云官方公开 HTML、JSON-LD 和歌曲详情页 metadata；不使用账号密码、用户 Cookie、自动登录、私有 API、签名接口、hydration data 或客户端协议。这是有限的公开页面解析，不是官方 OAuth/API 接入，也不保证任意公开歌单可导入。

## 数据流与能力

移动/主站 URL 校验 → 公开歌曲候选校验 → 公开歌曲 metadata → 既有 StoredImport / Import Preview → 用户确认 → 既有 MetadataResolver → Analysis → Recommendation。

- 复用精确 HTTPS 主机、数字 ID、canonical URL 与同平台重定向限制。
- 歌单页必须返回 HTTP 200、HTML 和完整 UTF-8 响应；响应上限 1 MiB，沿用 7 秒超时。截断、错误或超限响应不解析。
- DOM 解析只读取 `#song-list-pre-cache[data-key=track_playlist-ID] ul.f-hide` 中的歌曲链接，并忽略推荐区、脚本和隐藏编码载荷。
- JSON-LD 必须是同一歌单 ID 的 `MusicPlaylist`，页面声明总数须在 1–10,000。DOM 歌曲 ID 必须唯一、数量不大于声明总数，并与 JSON-LD 可见前缀的顺序一致。满足这些条件的非空公开前缀可以导入；未出现在页面中的其余歌曲不会被推断或伪造。
- 最多处理前 20 个公开候选。如果同页 JSON-LD 已含标题、艺人、专辑或时长则直接使用；缺少必需 metadata 的已知歌曲 ID 可以访问固定官方地址 `https://music.163.com/song?id=ID` 补全，不搜索、不猜测替代歌曲。
- 详情客户端匿名访问、禁止重定向，每页最多 1 MiB / 3 秒；每批最多 20 个请求、总计最多 20 秒、串行请求间隔 150 ms。详情 JSON-LD 必须是同 ID 的 `MusicRecording`。标题和艺人必需，专辑与时长允许为空；有效 ISO 时长转换为毫秒。
- 单曲详情失效或超时不会阻塞其他歌曲。至少一首成功即生成真实 Import Preview；没有公开候选或全部失败时保持 `ACCESSIBILITY_CHECK_ONLY`，且不生成 Track。

页面声明总数只用于分母和失败数量。比如页面声明 1196 首、匿名 HTML 公开 10 首且 10 首详情成功，结果是“已导入10/1196首歌曲”，不会声称读取了其余 1186 首。声明总数超过 20 时，界面同时显示“当前公开页面解析限制，仅导入前20首歌曲用于分析”。

## 统一模型与预览

沿用已有 Track：`title`、`artists`（统一格式中的 artist）、`album`、`duration_ms`、`platform=netease`（source_platform）、`platform_url`（source_url）。不新增另一套 Track。原始来源 URL 进入 ImportedTrack，转换后保存在 `external_ids` 的 `source_platform` / `source_url` 中，供既有 Resolver 克隆时保留来源。

成功时服务端创建 StoredImport 并返回 `import_preview`，前端自动展开原 ImportPreviewPanel。数据标记为 `REAL_PUBLIC_LINK`，`total_rows` 是页面声明总数，`parsed_count` 是实际成功数，`invalid_count` 是两者差值。用户确认后调用原 `/api/imports/:id/analyze`，进入既有 MetadataResolver、Analysis 与 Recommendation。

`import_rows` 只记录本次实际处理的公开候选，状态包括 `IMPORTED`、`SKIPPED_DETAIL_UNAVAILABLE`、`SKIPPED_REQUEST_LIMIT` 和 `SKIPPED_TIME_LIMIT`。未公开成员只计入总量差值，不生成虚构行。成功界面显示“网易云歌单解析成功”和“已导入X/Y首歌曲”；失败显示“检测到网易云歌单，但当前无法获取公开歌曲列表。请使用TXT/CSV备用导入。”

未修改 Spotify、YouTube、Apple connector，MetadataResolver 核心逻辑或 Recommendation 算法。`metadata.rs` 只扩展了既有 ImportedTrack → Track 的来源字段保留。

## 真实公开样本与限制

- [1196 首公开歌单](https://music.163.com/playlist?id=7736940069)：匿名公开页面声明 1196 首，当前 HTML / JSON-LD 暴露前 10 首，可验证这 10 首并有限补全详情，预期显示实际导入数 / 1196。
- [106 首公开歌单](https://music.163.com/playlist?id=7558013954)：匿名公开页面声明 106 首，当前页面同样暴露 10 首，用于验证公开前缀行为不是单一样本特例。
- [200 首公开歌单](https://music.163.com/playlist?id=3778678)：前置实测 DOM 可列出 200 个歌曲 ID，用于验证 20 首上限与完整 route → preview → analysis 流程。
- [官方歌曲页](https://music.163.com/song?id=3399839173)：前置实测 `MusicRecording` 提供标题、艺人、专辑和 ISO 时长。

公开页面结构、地区可见性和返回数量可能改变。解析器在身份或结构无法验证时降级，不声称官方授权 API 或完整歌单读取。Last.fm 未配置时，既有推荐流程会明确返回 `not_configured`，不会用 Demo 代替真实推荐。

## 测试入口

- `cargo test --workspace`
- `cargo test --workspace netease_real_partial_public_page_acceptance -- --ignored --nocapture`（真实 1196 首页面的有限公开前缀）
- `cargo test --workspace netease_real_public_import_preview_and_analysis -- --ignored --nocapture`（真实 HTTP 路由 → 存储预览 → 用户确认端点 → Resolver / Recommendation）
- `npm --prefix frontend run lint`
- `npm --prefix frontend run build`
- `npm --prefix frontend run test:e2e`

离线测试覆盖 DOM 容器归属、推荐区排除、HTML 实体、JSON-LD 前缀一致性、重复/数量/顺序/身份冲突、20 首候选上限、恶意 URL、可选字段 null、单项失败隔离、请求预算、批次超时和来源字段保留。前端合成契约验证部分导入、真实分母、限制提示、`REAL_PUBLIC_LINK`、确认前不分析以及确认后进入原分析与推荐页面；合成测试不冒充真实平台验收。

## 本轮验收结果

- 真实 1196 首样本：匿名页面声明 1196 首并公开 10 个可验证候选，结果为 `imported=10`、`status=public_html_tracks_imported`，联网验收 PASS。界面分母保持 1196，不把其余歌曲构造成 Track。
- 真实 200 首样本：生产 HTTP handler 返回 `Imported 20 / 200 tracks`；服务端创建 `REAL_PUBLIC_LINK` 预览，经确认分析端点得到 20 首非 Demo 曲目和 20 条 Resolver 结果。Recommendation pipeline 已执行，当前未配置 Last.fm，因此状态为 `not_configured`。联网 route → preview → analysis 验收 PASS。
- `cargo test --workspace`：160 passed / 0 failed / 3 ignored。两项网易云联网测试已显式单独执行通过；另一项 ignored 是既有 YouTube 联网探测。
- `cargo fmt --all --check`、前端 lint、build：PASS。
- 前端 E2E：32/32 PASS，覆盖中文成功/失败文案、真实分母、20 首限制提示、Import Preview、确认闸门及进入现有分析/推荐页面。前端使用合成 HTTP 契约；真实平台读取由上述后端联网验收覆盖，二者不混称。
