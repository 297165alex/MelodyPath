# Spotify 授权、读取与写回手动验收

> 以下步骤会在用户明确确认后创建一个新的真实 Spotify 私有歌单；不会修改原歌单。

## 准备

1. 创建 Spotify Developer 应用。
2. 登记 Redirect URI：`http://127.0.0.1:3000/api/spotify/callback`。
3. 设置 `SPOTIFY_CLIENT_ID`、`SPOTIFY_CLIENT_SECRET`、`SPOTIFY_REDIRECT_URI`、`FRONTEND_URL` 和所需 `SPOTIFY_MARKET`。
4. 分别启动后端与前端，并使用 `http://127.0.0.1:5173`，不要混用 `localhost`，以确保 HttpOnly cookie 可用。
5. 确认当前 Spotify Developer 访问条件、账号资格和 API 配额允许测试；这些条件可能随政策调整。

## 登录与歌单读取验收

1. 首页 Spotify 卡片必须显示后端能力 API 返回的状态；未配置凭据时不能出现“已连接”。
2. 点击“前往 Spotify 官方授权”，确认地址栏是 `accounts.spotify.com`，并且 MelodyPath 页面没有账号/密码输入框。
3. 确认范围为 `playlist-read-private`、`playlist-read-collaborative`、`playlist-modify-public` 与 `playlist-modify-private`。
4. 授权返回后确认首页显示真实 Spotify 昵称，点击“选择我的歌单”能读取当前账号 API 可访问的歌单。
5. 勾选一个或多个歌单并确认；检查曲目数量、前 10 首预览、Spotify attribution 和原始链接。
6. 检查响应 `data_use` 中读取/展示/传输/写回能力为 true，而分析、衍生指标、跨平台比较、LLM 与训练能力为 false。浏览器网络面板中不应出现把 Spotify Track 发往 `/api/tasks`、`/api/analyze/manual` 或外部 LLM Endpoint 的请求。
7. 点击“解除连接”，随后 `/api/spotify/me` 应返回 `connected: false`，再次读取歌单应为 401；后端内存 token 和 HttpOnly cookie 均已删除。

## 单人验收

1. 打开 Demo，进入从 Korean R&B 到 Neo Soul 的推荐页。
2. 点击“保存到 Spotify”，选择 10 首。
3. 如尚未连接，点击 OAuth 授权，在 Spotify 官方页面完成登录/同意。
4. 返回页面后再次打开写回，输入一个不存在的新歌单名。
5. 生成预览，检查高置信度、待确认、无法匹配三类；为待确认歌曲选择正确版本。
6. 在浏览器开发者工具确认预览请求没有创建歌单。
7. 勾选明确确认，执行写入。
8. 检查成功、失败、待确认计数之和等于请求数；打开返回的 `open.spotify.com/playlist/...` 链接核对歌曲。

## 双人验收

1. 进入好友桥梁页，点击“保存到 Spotify”。
2. 选择桥梁曲目、预览、确认版本并执行。
3. 打开真实链接，核对只加入匹配成功歌曲，并且原始双方歌单未被修改。

## 刷新与失败验收

- 等待 access token 过期或使用短期测试 token 后再次预览，确认后端自动刷新。
- 取消 OAuth、篡改 state、移除授权或选择地区不可播候选，应得到明确错误且不创建歌单。
- 不设置 Spotify 环境变量时，状态应显示“需要配置”，文件与 Demo 写入仍可完成。

## 验收记录模板

- 日期/部署：
- Spotify 开发者应用模式与账号资格：
- 登录成功并显示昵称：是 / 否
- 个人歌单真实读取：是 / 否
- 新歌单真实创建：是 / 否
- 曲目批量写入：成功数 / 失败数 / 待确认数
- 返回的真实歌单链接：
- 解除连接后 token 删除：是 / 否
- 备注/截图：

未填写真实账号验收记录时，只能说“代码路径已实现”，不能用 Demo 流程替代真实验收。
