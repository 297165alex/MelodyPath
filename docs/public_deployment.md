# 公网部署工程准备

目标是受控课程演示的单实例 Linux 服务，尚未上线。现有本地 Spotify/YouTube 真人验收不等于公网 REAL VERIFIED。

## 主方案

Render Docker Web Service + 持久磁盘。Node 构建 Vite，Rust 构建 release，最终 Debian 镜像内 Axum 同域提供静态文件、SPA、`/api/*`、`/health`；Render 提供 HTTPS。容器准备专用数据目录后以非 root 用户运行。

选择理由：保留 SQLite，无需迁移数据库；稳定 HTTPS 地址便于 OAuth；浏览器相对 `/api` 请求避免跨域 cookie；平台 Environment Secrets 与持久盘适合约 14 天课程演示。免费实例不支持持久盘，因此模板使用 Starter，费用由用户在平台确认。只实现这一套主方案。

## 两周 Demo 与费用（2026-09-07 核对）

Hobby workspace 本身免费；Web Service 选 Starter（0.5 CPU / 512 MB），标价 $7/月。持久盘先选 1 GB，$0.25/GB/月。按 30 天月粗估，14 天 compute $3.27、disk $0.12，总计约 **$3.39 USD**；按秒计费，实际月份及运行时长影响账单。不是封顶报价。新 Hobby 包含每月 5 GB 出站流量与 500 build minutes；超额流量 $0.15/GB，标准构建超额 $5/1000 minutes。税费、汇率、可选外部付费 API、增加资源及到期未停止服务会增加费用。无需购买域名或付费 workspace。暂停 compute 不等于磁盘停止收费；保留磁盘会继续计费，任何删除必须由用户决定并先备份。

依据：[Starter 价格](https://render.com/articles/render-vs-railway)、[计费规则](https://render.com/docs/faq)、[磁盘与构建费](https://render.com/articles/how-much-does-cloud-application-hosting-cost-for-small-businesses)、[当前 workspace 限额](https://render.com/docs/new-workspace-plans)。付款前以 Dashboard 确认价为准；未做 512 MB 公网负载验收。

部署者将 PUBLIC_DEMO_EXPIRES_AT 设置为实际开放日期后约 14 天的 YYYY-MM-DD。后端只向 /api/public-config 公开这个日期；production 才显示提示，local 忽略该变量。日期只用于告知，不会停服务或删数据。到期由部署者手动管理 Render 服务。

公网 Demo 开放期间，普通用户无需安装 Rust/Node 或配置 Developer Credentials，Spotify/YouTube 只需官方 OAuth 授权（仍受平台测试用户资格限制）。到期公网可能关闭，源码仍保留在 GitHub / Git.Tsinghua，可 clone 自行部署。Self-host 使用真实 Spotify / YouTube / Last.fm 必须配置自己的 Developer Credentials；clone 不会获得部署者的 Secret。

**ROTATE：Spotify Client Secret、Google Client Secret。** 旧值曾在截图中出现，不得用于公网。新值只由用户填入 Render Environment，不发给助手，不写入代码、日志、README 或交接文件。

官方依据：[Docker](https://render.com/docs/docker)、[持久盘及单实例限制](https://render.com/docs/disks)、[Blueprint](https://render.com/docs/blueprint-spec)、[免费实例限制](https://render.com/docs/free)。

## 配置

| 变量 | 生产值或用途 | 机密 |
| --- | --- | --- |
| MELODYPATH_ENV | production | 否 |
| PUBLIC_BASE_URL | 实际公网 HTTPS 根地址，不含路径 | 否 |
| PUBLIC_DEMO_EXPIRES_AT | 实际开放日期后约 14 天，YYYY-MM-DD；仅提示 | 否 |
| FRONTEND_URL | 省略或与 PUBLIC_BASE_URL 相同 | 否 |
| PORT | 平台注入，模板 10000 | 否 |
| MELODYPATH_BIND | 省略则生产绑定 0.0.0.0:PORT | 否 |
| MELODYPATH_STATIC_DIR | /app/static | 否 |
| MELODYPATH_DATA_DIR | /var/data/melodypath，必须在持久盘中 | 否 |
| MELODYPATH_DB_PATH | 本地兼容；生产访客 SQLite 在 DATA_DIR/browsers | 否 |
| OAUTH_TOKEN_STORE | server_encrypted | 否 |
| OAUTH_COOKIE_SECURE | true | 否 |
| OAUTH_TOKEN_ENCRYPTION_KEY | 平台生成的 32 随机字节，标准 Base64 | **是** |
| LASTFM_API_KEY | 部署者 Last.fm Provider Key | **是** |
| SPOTIFY_CLIENT_ID / GOOGLE_CLIENT_ID | 部署者应用标识 | 非机密，可出现在官方 authorize URL |
| SPOTIFY_CLIENT_SECRET / GOOGLE_CLIENT_SECRET | 部署者 Client Secret | **是** |
| SPOTIFY_REDIRECT_URI / GOOGLE_REDIRECT_URI | 可省略，自动派生公网 callback；显式值必须精确一致 | 否 |
| OPENAI_API_KEY | 可选，无配置继续 deterministic fallback | **是** |
| YOUTUBE_API_KEY | 可选检查项，不替代 OAuth | **是** |

API_BASE_URL 是内部运维地址约定，浏览器不消费它。VITE_API_PROXY_TARGET 仅用于本地 Vite proxy。生产同域不需要这两个参数，不把 localhost 或 Secret 放入 bundle。不要复制 .env.example 的本地 callback、FRONTEND_URL 或 bind 到生产，不要将 Secret 命名为 VITE_*。

生产配置不合法则启动失败：拒绝 HTTP/loopback 公网地址、跨域 frontend、错误 callback、不安全 cookie、缺失/无效加密 key。单个平台 Developer credentials 未配置时保留真实 CONFIG_REQUIRED，不伪装已连接。普通用户不申请这些 credentials，只在官方页面授权。默认 YouTube 只读，Copy 写入单独授权。

## Linux token store

Windows 本地继续 DPAPI；Linux 生产采用服务端 AES-256-GCM，随机 96-bit nonce 与认证 tag，绑定 provider 文件名。平台保存主密钥，持久盘保存密文；Unix 文件权限 0600，同目录临时文件 sync 后 atomic rename。密钥、token 不进入镜像、前端或 SQLite。

启动前验证已有密文，错误 key 会阻止启动，避免静默覆盖旧会话。这不是托管 KMS；服务器管理员或被攻陷的进程仍能解密。适合单实例课程演示，不宣称企业级密钥托管。

主密钥必须安全备份。不能直接更换 key 并声称无损轮换；需停机重加密，或明确通知用户重新授权。本轮没有实现在线多 key 轮换。SQLite 不是字段级加密数据库，依靠目录权限、平台访问控制和备份保护。不得自动删除用户数据。

## 多访客与运行限制

原本地单用户状态不能直接共享给公网。新增生产入口以签名 HttpOnly/Secure/SameSite=Lax cookie 隔离浏览器工作区；前端先完成一次 /api/session，避免并发首次请求分裂会话。各访客的 SQLite 历史、Agent、分析、imports、预览、downloads 和 Copy run 分开，核心算法不变。

签名 key 从部署主密钥按独立用途派生。cookie 有效 8 小时，无跨设备恢复或账户绑定；清除 cookie 后 UI 无法找回旧匿名工作区，历史文件不会自动删除。部署者需制定数据保留与删除政策。

写 API 必须精确同 Origin，不开放跨域 CORS；OAuth callback 保留顶层 GET 和一次性 state。公网禁止访客修改部署者 LLM endpoint/settings，以及触发开发者凭据检查，防止服务器 Key 被发送到访客指定地址。

基础限额：64 个同时有效工作区、8 个并发 API 请求、每工作区每分钟 120 次 API 请求。这不是任意公网流量下的完整滥用防护；公开推广前仍需准入、全局持久额度、后台任务预算及数据生命周期管理。

仅单实例。OAuth token 文件和 SQLite 可持久化；pending OAuth state、预览和活动 Agent/Copy run 仍在内存中，服务重启后重新授权或生成预览，不支持跨重启恢复活动任务。Render 持久盘也不支持多副本或零停机部署。

## 验证与构建

```powershell
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo build --release --locked
npm --prefix frontend run lint
npm --prefix frontend run build
npm --prefix frontend run test:e2e
node deploy/audit-production.mjs
python deploy/smoke-production.py target/release/melody-path-api.exe frontend/dist
docker build -t melodypath:public-deployment .
```

Linux smoke 使用 Linux binary 路径替换 .exe。smoke 仅使用独立临时目录与合成 credentials，测试真实 production 进程的路由、health、callback、Secure cookie、CSRF、错误配置；Linux 还验证 SIGTERM 正常退出，绝不进行真实 token exchange。

audit 检查 bundle 的 loopback、合成后端 Secret canary、常见凭据模式和 git 敏感文件名；不读取真实环境密钥或用户数据，不能保证发现所有格式的秘密。.dockerignore 允许列表仅包含构建输入；不要通过 build args 注入真实 Secret。

## 人工部署节点

状态：WAITING_FOR_USER_DEPLOYMENT_ACTION。本轮不登录平台、不建服务、不付款、不授权 GitHub、不上传镜像、不 push、不修改 Dashboard。

1. 当前代码只在本机。用户先决定如何将审阅后的分支或镜像提供给 Render；禁止 push 的本轮无法让 Render 自动看到本地 commit。
2. 打开 [Render Dashboard](https://dashboard.render.com/)，用户登录，点 New → Web Service，授权并选择已包含最新代码的仓库和 feature/public-deployment 分支。Runtime 选 Docker，Dockerfile Path 填 ./Dockerfile；也可用 Blueprint 导入 render.yaml。
3. 选 Starter 或支持磁盘的计划，Disk Mount Path 填 /var/data，Size 先选 1 GB；用户确认付款。保持单实例，关闭自动部署，Health Check Path 填 /health。
4. 取得实际 HTTPS 服务地址后填入 PUBLIC_BASE_URL；尚无地址时先保存服务，再配置 Environment 并手动部署。缺失配置时启动失败是保护机制，不填写虚构域名。
5. 在 Environment 配置上表变量。Blueprint 自动生成 OAUTH_TOKEN_ENCRYPTION_KEY；手动方式请用本机密码管理器生成 32 随机字节 Base64，直接填入平台 Secret，不发送给助手。其他 Secret 也只在平台填写。
6. 在 Spotify Dashboard → App Settings → Redirect URIs，以及 Google Cloud → OAuth Web Client → Authorized redirect URIs，分别精确登记 `https://实际域名/api/spotify/callback` 与 `https://实际域名/api/youtube/callback`。旧 127.0.0.1 callback 可以保留本地使用。
7. 告诉助手：实际公网根 URL、build/health 状态、已配置的变量名称、callback 是否登记。只提供状态，不提供 Secret、code、token 或 Cookie。

## 公网验收

逐项验证主页、health、本地文件导入、Last.fm、Spotify Connect/identity/playlists/public URL、YouTube Connect/identity/playlists/public URL、Agent、Compare、Version Radar。官方 Allow 由用户本人点击。本地与合成测试不等于公网 REAL VERIFIED，最终 Copy 写入仍需真实目标链接。

公网域名不解除应用资格：Spotify 开发模式仍有 allowlist、用户数和额度限制，Google testing/production 发布与审核也需部署者完成。普通用户不需要 Developer Key，不等于任何账号都会被平台允许授权。官方依据：[Spotify quota modes](https://developer.spotify.com/documentation/web-api/concepts/quota-modes)、[Google production policies](https://developers.google.com/identity/protocols/oauth2/production-readiness/policy-compliance)。

README 本轮不改。只有真实公网成功后才建议补充 Online Demo URL、普通用户授权步骤、本地部署者配置说明及仍未验收的功能。

上线后 README / 网络学堂文案应保留上述凭证责任说明。以下为待填模板，**当前不可作为已上线声明发布**：

> 【在线体验】〈真实部署成功后填写 URL〉
> 本版本为课程项目临时 Demo，预计开放至〈实际日期〉。开放期间普通用户无需安装 Rust/Node 或配置 Developer Credentials，Spotify/YouTube 仅需官方 OAuth 授权。到期公网服务可能关闭，源码仍保留在 GitHub / Git.Tsinghua。希望继续使用的同学可 clone 并自行部署；self-host 如需真实 Spotify / YouTube / Last.fm 能力，须自行配置对应 Developer Credentials，不能共享本 Demo 部署者的 Secret。
