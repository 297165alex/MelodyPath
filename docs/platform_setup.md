# 音乐平台本地配置清单

## A. Spotify Developer App

1. 打开 Spotify Developer Dashboard，创建 Web API 应用。
2. Redirect URI 精确填写：`http://127.0.0.1:3000/api/spotify/callback`
3. 在项目根目录 `.env` 设置 `SPOTIFY_CLIENT_ID`、`SPOTIFY_CLIENT_SECRET`、`SPOTIFY_REDIRECT_URI`。
4. 重启后端，在首页配置向导点击“检查配置”，再前往 Spotify 官方授权。

开发模式的应用拥有者通常需符合 Spotify 当前 Premium/用户白名单要求。请求范围仅为读取私有/协作歌单、修改公私歌单和读取基础账号身份。

## B. Google Cloud / YouTube Data API

1. 新建 Google Cloud 项目，启用 YouTube Data API v3。
2. 配置 OAuth consent screen，创建 Web application OAuth client。
3. Authorized redirect URI 精确填写：`http://127.0.0.1:3000/api/youtube/callback`
4. 在 `.env` 设置 `GOOGLE_CLIENT_ID`、`GOOGLE_CLIENT_SECRET`、`GOOGLE_REDIRECT_URI`、`YOUTUBE_API_KEY`。
5. 测试状态下把验收账号加入 Test users；重启后端并使用配置向导检查。

## C. Apple Music MusicKit

2026-09-07 更新：本分支已实现 Apple Music **公开目录** URL 元数据与曲目分页，复用现有 Import Preview。尚未真人验收，私人资料库、MusicKit Web 账号授权与 Apple 写入均未接入。完整能力、官方依据与限制见 [平台审计](platform_capability_audit.md)。

无需现在付款。已有资格的部署者可在仓库外使用 Apple Team ID、Key ID、MusicKit 私钥生成 ES256 Developer Token，只通过后端安全环境配置 `APPLE_MUSIC_DEVELOPER_TOKEN` 后重启。本项目不读取私钥文件、不自动签发 Token；预生成 Token 路径不再要求多余的 Team/Key 环境变量。缺失、格式无效或过期时为 `CONFIG_REQUIRED` / `WAITING_FOR_APPLE_DEVELOPER_CREDENTIALS`。配置就绪仅代表本地格式检查，Apple 可能继续拒绝签名、origin 或权限。

普通用户无需开发者凭据，在首页粘贴带地区的 `music.apple.com/.../playlist/.../pl.ID` 公开链接即可尝试目录读取。核对完整歌曲、跳过条目和地区差异后才可记录真人验收。所有 Developer Token 留在后端；旧 bootstrap 接口不再下发 Token。私人资料库需要 Music User Token，不能通过公开目录 URL 绕过。

## D. 必须由用户亲自完成的验收

Spotify 与 Google/YouTube 的官方登录、同意授权、真实歌单读取及真实写回必须由账号持有人亲自操作。Apple 当前不进入登录验收。没有 OAuth 配置时，真实文件/文本分析、Last.fm 推荐、好友比较、Agent fallback 与历史仍可运行。

## E. 生产环境

生产部署设置稳定 HTTPS `PUBLIC_BASE_URL`。若未显式提供平台回调，后端会派生 `<PUBLIC_BASE_URL>/api/spotify/callback` 与 `<PUBLIC_BASE_URL>/api/youtube/callback`；控制台登记值必须完全一致。开启 `OAUTH_COOKIE_SECURE=true`，由同域反向代理把 `/api` 转发到 Rust，并为 `OAuthTokenStore` 提供托管密钥/KMS 实现。开发机的 Windows DPAPI 文件不能复制到服务器充当生产凭据库。
