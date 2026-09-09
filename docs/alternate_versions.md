# Alternate Versions Explorer

更新时间：2026-09-09

## 目标与边界

普通推荐默认排除用户原曲的 Live、Remix、Acoustic、Sped Up 等派生版本。Alternate Versions Explorer 是独立的主动探索入口：只有用户点击 `Explore other versions` 并选择版本类型后，系统才搜索不同录音版本。

支持的用户选择包括 Live、Concert、Remix、Acoustic、Unplugged 和 Remaster。底层统一版本解析还识别 Bonus Track、Sped Up、Slowed、Radio Edit、Instrumental、Karaoke、Cover、Reaction 与 Nightcore，避免各流程维护互相矛盾的字符串判断。

## 候选与评分

`backend/src/alternate.rs` 使用确定性规则检查：

- 基础歌名是否一致；
- 艺人身份是否一致；
- 候选版本是否符合用户所选类型；
- 官方艺人、Topic 或 VEVO 等来源信号；
- 时长存在时的差异。

同名不同艺人会被拒绝；标题中的假关键词不会自动被识别为版本。每个候选保留平台、版本类型、官方状态、来源 URL、系统分数、置信度和理由。

## Provider 与安全状态

- 真实搜索只调用当前用户通过官方 Google OAuth 授权的 YouTube Data API。
- 缺少配置或授权时返回 `BLOCKED_EXTERNAL_AUTH`，不自动切换 Mock。
- Mock 只有用户明确点击后才运行，并显示 `Mock alternate-version connector · MOCK_VERIFIED` 与 `mock://` 来源。
- 选择候选加入歌单时复用 Playlist Writer 的匹配预览与明确确认；不会一键写入真实账号。

## Release Radar

同一 `/versions` 页面还按已导入曲目的艺人查询 MusicBrainz 公开发行组，分别展示 `New releases`、`Upcoming albums` 与 `Artist updates`。查询失败显示错误，无符合条件的数据统一显示 `No update available`，不会用固定发行记录填充。该数据源不等于流媒体平台发行预告，日期缺失的记录会被忽略。

## 验证结果

- 自动测试覆盖 Original → Live、Original → Remix、假 Remix 关键词和同名不同艺人拒绝。
- 独立 `/versions` 已在浏览器绑定 50 首真实分析，页面显示每批 4 首、最多 40 首、进度与取消边界。
- 本地浏览器从明确 Demo 推荐卡展开版本探索；真实 YouTube 搜索正确显示 `BLOCKED_EXTERNAL_AUTH` 且候选为空。
- 显式 Mock 返回 Live、Remix、Acoustic 三类候选，每项均保留 Mock 来源和理由。
- 推荐卡与 Alternate 候选的 `Add to playlist` 均进入同一确认流程；最终创建按钮在用户勾选确认前禁用。

真实 YouTube 搜索与真实写入尚未通过 OAuth 验收，当前状态为 `MANUAL_AUTH_REQUIRED / BLOCKED_BY_CONFIGURATION`，不能标记为 `REAL API VERIFIED`。
