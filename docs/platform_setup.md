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

1. 加入 Apple Developer Program，在 Certificates, Identifiers & Profiles 创建 MusicKit identifier 与 Media Services key。
2. 安全保存 `.p8`，设置 `APPLE_TEAM_ID`、`APPLE_KEY_ID`、`APPLE_PRIVATE_KEY_PATH`。
3. 生成 ES256 developer token，或安全设置 `APPLE_MUSIC_DEVELOPER_TOKEN`；token 不得提交 Git。
4. MusicKit Web 在 Apple 官方界面取得 Music User Token。当前项目尚未完成真实账号读写验收，因此 UI 会如实标注。

## D. 必须由用户亲自完成的验收

Spotify、Google/YouTube 和 Apple 的官方登录、同意授权、真实歌单读取及真实写回均必须由账号持有人亲自操作。没有凭据时 Demo、手工输入、文件导出、分析、推荐、好友比较、Agent 与历史仍可运行。
