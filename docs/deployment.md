# MelodyPath 单域名部署准备

> 历史记录（2026-09-05）。当前 Linux 加密存储、生产配置、访客隔离、测试和 Render 人工步骤以 [public_deployment.md](public_deployment.md) 为准；下文“生产 store 尚未实现”已被后续工程准备取代，但真实公网验收仍未完成。

更新时间：2026-09-05

## 目标拓扑

普通用户只访问一个稳定 HTTPS 地址：`https://<melodypath-domain>/`。前端静态资源由同一域名提供，反向代理把 `/api/*` 与 `/health` 转发到 Rust 后端。前端代码使用相对 `/api`，没有生产环境 localhost 依赖。

本轮只完成部署准备，没有创建公网资源、域名、证书或第三方账号。

2026-09-06 本机终检使用 `http://127.0.0.1:6174/`。Windows 当前保留了 5100–5199，导致默认 5174 监听被拒绝；验收仅选择了可用开发端口，没有更改系统网络配置。现有启动脚本的首选端口范围尚未自动排除 Windows 保留范围；开发时可运行 `npm run dev -- --port 6174`。这属于开发环境限制，不改变生产 HTTPS 目标或 OAuth 架构。

## 环境参数

- `PUBLIC_BASE_URL`：公开 HTTPS 根地址。未显式设置回调时，用于派生 Spotify/YouTube callback。
- `API_BASE_URL`：运维记录的后端内部地址；同域部署的浏览器不直接访问它。
- `VITE_API_PROXY_TARGET`：仅 Vite 开发代理目标。
- `OAUTH_COOKIE_SECURE=true`：生产环境强制 Secure；Cookie 同时为 HttpOnly、SameSite=Lax、Path=/。
- `OAUTH_TOKEN_STORE`：本地为 `windows_dpapi`；生产环境必须替换为实现 `OAuthTokenStore` 的托管密钥/KMS 存储。

服务端 Secret 仅通过部署环境或托管 Secret 服务注入；不得进入前端 bundle、Git、README、SQLite 或日志。

## OAuth 控制台

生产地址必须稳定且与控制台完全一致：

- Spotify：`https://<domain>/api/spotify/callback`
- Google/YouTube：`https://<domain>/api/youtube/callback`

前端只访问 `/api/spotify/authorize` 或 `/api/youtube/authorize`，用户不手动打开 callback。后端校验 `state`、交换 token、设置不透明 HttpOnly session，再重定向回 `PUBLIC_BASE_URL`。

参考：[Spotify Authorization Code](https://developer.spotify.com/documentation/web-api/tutorials/code-flow)、[Google Web Server OAuth](https://developers.google.com/youtube/v3/guides/auth/server-side-web-apps)。

## 安全清单

- TLS 在反向代理终止，并只信任预定 Host/Origin。
- 仅同域浏览器调用 API；若拆分 API 子域，显式限制 CORS 与 credentials origin，不使用通配符。
- OAuth state 防 CSRF；写入仍需要预览与显式确认。
- refresh token 不返回浏览器、不写明文数据库、不打印日志。
- 生产 Token Store 必须支持加密、删除、轮换、最小权限和审计；Windows DPAPI 只适合本地开发用户范围。
- Copy run 状态目前是单后端进程内会话；生产多实例前需要共享任务存储/队列，当前不得宣称跨重启恢复。

## 发布前人工阻塞

`BLOCKED_BY_CONFIGURATION`：域名、HTTPS、部署主机、托管 Secret/KMS、Spotify 应用、Google Cloud 项目、YouTube Data API 与配额尚未由账号持有人配置。

`MANUAL_AUTH_REQUIRED`：配置完成后仍需用户分别在 Spotify 与 Google 官方页面授权，并实际创建新的私有 YouTube 播放列表、取得链接，才能标记真实 Copy 验收通过。
