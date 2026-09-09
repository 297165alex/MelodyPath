# Spotify Copy 授权恢复与跨平台版本发现

日期：2026-09-09。此记录描述代码与合成测试，不代表真实平台写入验收。

## Spotify Copy

写入入口检查后端 `/api/spotify/me` 的 `connected` 与 `write_authorized`。后者要求实际保存的 scope 同时含 `playlist-modify-public` 和 `playlist-modify-private`。只读连接的 scope 保持不变；用户点击“重新授权 Spotify”才打开官方 OAuth 写入流程，申请 `user-read-private playlist-read-private playlist-read-collaborative playlist-modify-public playlist-modify-private`，保留协作读取兼容性。

沿用 connector 的 token exchange、实际 granted scope 保存和刷新逻辑。回调页向原窗口发同源通知，原窗口同时核验消息来源和后端连接状态；URL 或窗口消息本身不能建立写权限。授权取消、未授予 scope、弹窗被阻止、过期及状态查询失败均显示可重试提示。

原窗口保留歌曲选择、名称、匹配与明确确认状态；授权后自动继续原来的预览或已确认执行。没有确认的任务只恢复预览，不自动创建。后端在消耗导出预览前检查权限，Transfer run 也在创建前检查写权限。默认目标仍为新的私有歌单，不修改源列表。

恢复依赖原页面仍然打开和后端进程仍然存在；刷新、关闭或服务重启不具备持久恢复保证。官方授权由用户本人完成，未执行真实账号操作。

Spotify 改动仅限 OAuth 请求、写权限就绪判断及测试；读取 API、YouTube/Apple connector、NetEase adapter 均未更改。[Spotify scope 定义](https://developer.spotify.com/documentation/web-api/concepts/scopes)与[私有歌单创建](https://developer.spotify.com/documentation/web-api/reference/create-playlist)提供权限依据。

## Version Discovery

新增独立 `version_discovery.rs`、`VersionProvider` 与 `/api/version-radar/discover`。Spotify 和 YouTube adapter 复用既有 connector；MusicBrainz adapter 使用公开 recording 搜索，带 User-Agent、超时和串行节流。未来 Apple Music、NetEase、QQ Music 可实现同一 trait，当前没有声称已接入这些平台。

统一候选包含 `title / artist / platform / url / version_type / language / confidence / reason`。确定性流程先理解来源基础标题、艺人和版本，再查询、分类、过滤、按偏好排序、按 URL 去重。支持 Original、Live、Acoustic、Cover、Language Cover、Remix、Instrumental、Extended，并兼容原来的扩展分类。

跨演唱者 Cover 必须有原唱署名证据；语言只在标题明确标注 English/Chinese/Japanese/Korean 或对应文字时给出，否则保持未知。不会通过语言识别猜测音频，也不能保证发现翻译后完全不同标题的版本。MusicBrainz URL 指向录音元数据，并非音频播放地址。官方数据源不等于每个匹配都已经人工确认。

排序参考已有 Taste Profile 的 Genre / Language 与来源歌曲的版本、语言，也允许用户手动选择偏好；Spotify 曲目不用于新增自动偏好统计。偏好加分不改变 Match 分数。结果逐首展示平台、语言、匹配分数及理由，保留各 Provider 的失败状态；部分 Provider 不可用时仍可展示其他来源结果，真实失败不切 Mock。Add to playlist 仍进入原有预览和确认闸门。

没有修改 MetadataResolver 或 Recommendation core，没有调用 LLM，也不传输 Spotify 内容给 LLM。公开查询依据：[MusicBrainz recording search](https://musicbrainz.org/doc/MusicBrainz_API/Search)。现有推荐卡的单曲 Alternate Explorer 保留兼容；跨平台发现入口为 `/versions`。

## 自动验证

Rust 覆盖实际 scope 保存、只读写入拒绝、合成 OAuth 后创建私有歌单、八类版本与中日韩翻唱标注、同名错误艺人拒绝、Cover 原唱证据、Provider 失败隔离、偏好排序和去重。

浏览器合成测试覆盖已有写权限、只读重授权、预览前失效、确认执行前权限失效后恢复、原任务名称和确认闸门保留，以及跨平台结果、偏好和部分 Provider 失败展示。真实 OAuth 与真实新歌单链接仍需用户人工验收。

最终结果：`cargo test --workspace` 187 passed / 0 failed / 4 ignored（既有显式联网测试）；`npm --prefix frontend run lint` PASS；`npm --prefix frontend run build` PASS；`npm --prefix frontend run test:e2e` 53/53 PASS；`cargo fmt --all --check` 与 `git diff --check` PASS。Rust 仍有原有未使用代码警告。
