# 网易云公开歌单第一阶段

日期：2026-09-08。范围：匿名公开页面元数据与完整性判断，不是官方 API connector 或完整歌曲导入验收。

## 实现

- 复用 `platforms.rs` 的精确 HTTPS 官方主机/路径校验、数字 playlist ID、canonical URL、同平台重定向限制。移动输入 `https://y.music.163.com/m/playlist?id=...` 规范化到 `https://music.163.com/playlist?id=...`，移除无关参数。
- `backend/src/platforms/netease.rs` 只解析声明为 `application/ld+json` 的公开 `MusicPlaylist`，支持顶层对象、数组、`@graph` 和字符串/数组 `@type`。不执行 JavaScript，不读取应用内部状态，不解码编码曲目。
- 只接受 HTTP 200 的 HTML，完整响应上限 512 KiB，沿用匿名客户端总超时和重定向限制。不发送 Range；部分响应、读取错误、超限、无效 UTF-8 不作为完整页面解析。
- `playlist_name` 为页面真实名称；`track_count` 为页面 **声明数量**，不是已导入数量。UI 在原公开链接组件显示这一差别。缺失字段保持 null。
- 检查实际 `track` / `itemListElement` 数量与 `numTracks`，不把 `numberOfItems` 当实际返回数量；标题/艺人缺失会报告元数据不足。即使完整结构出现，也不凭 HTML 成功解析宣称已核实平台自动曲目导入许可。
- 当前响应仍为 `ACCESSIBILITY_CHECK_ONLY`、`preview_tracks=[]`、`import_rows=[]`、`can_analyze=false`。所有不完整/未知状态均不生成 Track，不创建 Import Preview，不调用 MetadataResolver 查询或猜测缺失歌曲。

## Track 与既有 pipeline 边界

当前没有获得完整且许可已核实的歌曲来源，故 **未启用网易云 URL → Track → 推荐**。这一步按任务要求停在 ACCESSIBILITY_CHECK_ONLY；不是用测试数据完成真实导入。Spotify、YouTube、Apple connector 及 MetadataResolver 核心不变，既有文件/文本 Import Preview、Resolver、Recommendation 仍可用。

未来合法完整 reader 应输出既有 Track，保留标题、艺人、专辑、duration_ms、来源平台及 source_url；未返回的专辑/时长为 null，不填 0，不用页面描述推断艺人。不新增平行 Track 或另一个推荐引擎。本轮没有用假设的完整页面 fixture 宣称该成功路径已经实现。

## 真实公开样本

匿名请求 [网易云移动公开歌单](https://y.music.163.com/m/playlist?id=7299150850)，canonical 为 [主站公开页](https://music.163.com/playlist?id=7299150850)。2026-09-08 本机公开 HTTP 返回 200，名称为“历届奥运会主题曲（1984～2022）”，声明 15 首，JSON-LD 实际 10 项且缺艺人/时长。因此只验证公开元数据与诚实降级，未验证完整歌曲导入或基于 URL 的真实推荐。

没有使用账号密码、Cookie、浏览器自动登录、私有 API、逆向接口或其他客户端编码数据。未保存整页、响应头或私人歌单。网页抓取工具直接打开失败后，使用本机匿名 HTTP 验证；不把工具打开失败解释为平台整体不可访问。

## 验证

联网验收显式运行：`cargo test --workspace netease_real_public_page_acceptance -- --ignored --nocapture`。默认忽略该测试，避免普通 CI 依赖外部站点；它使用生产 PlatformService，只输出公开聚合数量和状态。失败不能被算作通过。

离线测试覆盖不完整条目、缺艺人、完整结构仍不证明许可、无效/截断 JSON、内部状态与无关 schema、响应超限和中断；沿用 URL/重定向安全测试。新增前端测试验证声明 15 首/实际 10 项不会显示导入预览或确认按钮，也不会发起分析导入请求。

本轮结果：

- `cargo fmt --all --check`、`git diff --check`：PASS。
- `cargo test --workspace`：152 passed / 0 failed / 2 ignored；保留既有 3 个 dead-code warning。
- 显式网易云联网测试：1 passed，返回 `declared=Some(15); status=public_track_list_incomplete; imported=0`。
- 前端 lint、build：PASS。
- 前端全量 E2E 首跑：30 passed / 1 failed；新增用例在 `page.goto` 等待外部 Google Fonts 加载时超时，未进入业务断言。该用例隔离外部字体后专项重跑 1/1 PASS；未放宽 timeout 或业务断言。31 个用例分别通过，不写成一次全量 31/31。
