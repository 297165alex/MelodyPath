# OAuth 与公开链接收尾审计（2026-09-07）

## 最小权限

| 平台 | 默认连接 | 用户主动选择 Copy 写入时额外申请 |
| --- | --- | --- |
| Spotify | `playlist-read-private playlist-read-collaborative` | `playlist-modify-private` |
| Google / YouTube | `https://www.googleapis.com/auth/youtube.readonly` | `https://www.googleapis.com/auth/youtube.force-ssl` |

Spotify 当前使用基本身份字段 id/display_name/images，删除了不需要的 `user-read-private` 和公开歌单写权限。目标默认新建私有歌单。
YouTube 身份指频道身份，由 `channels.list(mine=true)` 获取；不申请 Google 邮箱、联系人或额外身份权限。只读 scope 用于频道、playlists、playlistItems、搜索和时长查询。

用户看到的 “See, edit and permanently delete your YouTube videos, ratings, comments and captions” 来自 `youtube.force-ssl`。默认授权现在不再请求该 scope；只有 Copy 的显式写入授权入口使用 `?write=true`。Google 当前没有仅限新建歌单和添加视频的写入 scope；`playlists.insert` 和 `playlistItems.insert` 接受 youtube、youtube.force-ssl 或 youtubepartner，不能用 readonly 完成写入，也不能将 youtube 描述成更窄的权限。用户若不接受广泛写入授权，可仅使用读取和预览。

官方依据：[OAuth scope 定义](https://developers.google.com/youtube/v3/guides/auth/server-side-web-apps)、[创建歌单权限](https://developers.google.com/youtube/v3/docs/playlists/insert)、[添加视频权限](https://developers.google.com/youtube/v3/docs/playlistItems/insert)、[Spotify 基本身份](https://developer.spotify.com/documentation/web-api/reference/get-current-users-profile)。

Google authorize 使用 `include_granted_scopes=false`。请求 scope 缩小不等于撤销过去的账号授权。旧 grant 若仍显示广泛权限，用户应在 Google 账号第三方连接中移除旧 MelodyPath 授权后重新只读连接。不会自动撤销用户授权或删除账号内容。

保存并刷新平台实际返回的 granted scopes；未返回或旧会话缺失 scope 时不推断写入授权。create_playlist/add_tracks 均检查写入 scope，readonly 明确返回 WRITE_AUTH_REQUIRED。授权与真正写入是两个独立步骤，预览和明确确认闸门仍保留。

## YouTube 回调证据边界

用户已确认到达 Google consent 并点击 Continue。但这只能证明该人工步骤，无法反推出 callback、session 或 playlist read 成功。未读取 Cookie、token store、数据库或私人歌单来追溯。

| 检查项 | 代码与测试 | 这次历史真人结果 |
| --- | --- | --- |
| callback 收到 code | 记录 callback_received / code_present 布尔值 | 未确认 |
| state 通过 | 浏览器绑定、一次性、过期校验；记录 state_validated | 未确认 |
| token exchange | 拒绝缺失/空 token；记录 token_exchange_succeeded | 未确认 |
| 频道 identity | channels 官方读取成功后才保存 session；无频道测试拒绝连接 | 未确认 |
| session 保存 | 加密本地存储成功后记录 session_saved | 未确认 |
| 浏览器 cookie | HttpOnly cookie；后续 /me 记录是否携带 session 及 connected 布尔值 | 未确认 |
| session status | 回调后前端主动请求 /api/youtube/me，query 不能建立连接 | 未确认 |
| playlists 读取 | 官方接口分页；错误显示，不以返回首页代替验收 | 未确认 |
| 错误可见性 | 安全 callback reason、明显 Error、初始 /me 失败提示 | 合成回归通过 |

日志不记录 code/state/token/cookie 内容、身份或歌单内容。/me connected 说明后端持有有效期内会话与回调已验证身份，不代表此时 playlists 读取或最终 Copy 成功。

当前标记：`READY_FOR_YOUTUBE_REAUTH` / `WAITING_FOR_USER_GOOGLE_AUTH`。

## 公开链接与 UI

- Spotify、YouTube：合法 URL/ID + 合法 session → 官方歌单元数据和曲目分页 → Import Preview。读取器与账号选择复用；拒绝无效 ID、未授权、过期、平台失败，绝不返回替代歌曲。API 401 变为 AUTH_REQUIRED。
- Preview 只进入 Copy 确认，不进入分析/画像/推荐/LLM。未点击确认不会创建或写入目标列表。
- Spotify API 对具体公开歌单的访问仍取决于账号、应用模式和平台权限。测试 fixture 能证明代码契约，不能证明任意真实公开歌单可读。
- Apple/酷狗/汽水：URL_RECOGNITION_ONLY；网易云/QQ：ACCESSIBILITY_CHECK_ONLY（页面是否可访问依实际响应）；未知平台 UNSUPPORTED。没有私有接口或抓取完整曲目。
- 首页保留 Experimental。Spotify picker 列表独立滚动、footer 可见、选择数与 disabled 状态，已验证 1280×720 和 390×600。

## 明早真人验收

1. 打开 http://127.0.0.1:5174/，通过首页 YouTube 只读连接入口重新授权；必要时先移除旧 Google grant。不要直接访问 callback URL。
2. 返回后必须显示后端核验的 YouTube Connected，再点“选择我的播放列表”确认官方读取；若 Error，记录安全错误文案即可，不发送凭据/URL 中的授权码。
3. 在当前浏览器用自己有权限的一个 Spotify URL 和一个 YouTube URL 验证 Import Preview。Spotify 既有真人 OAuth/歌单读取成功由用户确认，但新的 URL 导入尚未真人验收。
4. 需要端到端 Copy 时再显式授权 Google 写入；选择一个 Spotify 测试歌单，逐首确认/跳过歧义，明确确认新建私有 YouTube 歌单。只有真实目标链接生成并确认内容，才能标记真实迁移通过。

在上述真人环节完成前，不建议 merge main。
