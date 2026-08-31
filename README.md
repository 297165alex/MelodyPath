# MelodyPath · 可解释音乐探索与跨平台好友歌单 Agent

> MelodyPath是《程序设计训练（Rust语言）》AI Agent课程项目，同时将阶段性成果用于智理杯智能体大赛。

MelodyPath 不只统计歌单：它用可解释路线帮助用户从熟悉 Genre 逐步走向新风格，并在两名朋友的跨平台歌单之间生成变化平滑的“桥梁歌单”。比赛阶段使用完全离线、稳定且明确标注的 Demo 数据；课程实现保留完整 Rust 业务层、Agent 循环、实时进度、中断、SQLite 历史、配置与用量统计。

# 当前最高开发优先级：跨平台歌单迁移

MelodyPath 不只是歌单分析和推荐网站。现有本地分析、Last.fm 可解释推荐、历史、Agent Loop、好友桥梁和导出能力必须继续保留；推荐功能目前可用，但真实验收已经记录了同曲不同版本去重、跨语言艺术家别名识别和推荐流水线分段统计仍不完整等限制，详见 [真实推荐验收报告](docs/recommendation_acceptance_report.md)。这些限制不得被隐瞒，也不应通过删除推荐模块来规避。

下一阶段的最高优先级是一个边界清晰、可独立验收的 **Transfer MVP**：

> Spotify 用户拥有或参与协作的歌单  
> → 使用 YouTube 官方接口搜索并确定性匹配对应歌曲  
> → 由用户预览并确认歧义候选  
> → 创建一个新的 YouTube 私有播放列表

Transfer 是 MelodyPath 的核心 Agent 执行场景，不是普通的 LLM 推荐：系统需要执行分页读取、候选搜索、可解释评分、人工确认、逐首写入、进度反馈、中断和结果报告。Spotify 数据不得发送给 LLM，也不得用于训练、画像或与迁移无关的推荐。

账号连接必须使用 Spotify 和 Google/YouTube 的官方 OAuth。项目不得收集 Spotify 或 Google 密码、Cookie，不得模拟登录、绕过授权或调用私有接口；迁移不得修改或删除源 Spotify 歌单，目标端默认只能新建私有播放列表。

Mock 和自动测试通过只表示代码路径可测试，不能声称真实迁移成功。只有用户在真实浏览器中完成 Spotify 与 Google 两端登录授权、选择源歌单、确认匹配预览，并最终得到可访问的新 YouTube 播放列表链接，才能将真实 OAuth 迁移验收标记为通过。

## 已实现能力

- Rust + Axum 后端；分析、标准化、评分、桥梁约束和 Agent 调度均在 Rust 中。
- React + TypeScript + Vite 中文界面，含首页、品味地图、三段推荐、探索路线、双人桥梁、Agent、历史与设置。
- 三组离线示例数据；无需音乐平台 API 或 LLM Key。
- 首页主入口按“官方平台连接 → 公开歌单链接 → 更多导入方式”排列；普通用户无需制作 JSON/CSV。
- 后端 `/api/platforms/capabilities` 是平台状态的唯一真相源，未接通能力不会在前端伪装为可用。
- 公开链接可识别网易云、QQ音乐、Spotify 与 YouTube 官方域名、解析歌单 ID 并检查页面可访问性；没有可验证官方曲目接口时明确停止，不抓私人 Cookie。
- 手动“歌手 - 歌名”和 TXT、CSV、JSON、M3U/M3U8 文件作为备用输入；缺失元数据时明确降低置信度。
- 两项音乐定制 Agent 场景：个人 Genre 探索、跨平台好友桥梁。
- SQLite 任务历史、SSE 实时进度、任务中断、失败终态、最大步数和超时边界。
- 模型 Endpoint、名称、温度、Token、超时、重试、价格和费用上限可配置；API Key 只读环境变量。
- `PlaylistWriter` 统一接口，以及文件、Demo、Spotify 与四个平台预留适配器。
- CSV / JSON / M3U8 真实文件导出；预览和明确确认是必经步骤。
- Spotify 官方 Authorization Code Flow、昵称/连接状态、个人可访问歌单选择、统一 Track 转换、token 刷新、解除连接、本地 token 删除、ISRC/元数据匹配、新建私有歌单、批量写入与真实链接（需要用户自己的 Spotify 应用凭据）。
- Spotify API 数据只进入歌单传输/写回路径，不进入 LLM、用户画像、相似度、Genre 推荐或模型训练。
- YouTube 官方 Google OAuth、账号频道、拥有的播放列表分页、playlistItems 分页、标题噪声清理、统一 Track、token 刷新、解除连接、新建播放列表和加入视频（需要用户自己的 Google Cloud 配置，尚待真实账号验收）。
- Spotify、YouTube 与 Apple Music 均提供后端环境变量检测和中文配置向导；前端不会回显 Client Secret、私钥或 token。

## 环境要求

- Rust 1.85+（edition 2024）
- Node.js 20+
- npm 10+

## 安装与启动

打开两个终端。后端：

```powershell
cargo run -p melody-path-api
```

前端：

```powershell
cd frontend
npm install
npm run dev
```

浏览器打开 `http://127.0.0.1:5173`。Vite 会把 `/api` 代理到 `http://127.0.0.1:3000`。首次后端启动会在项目目录创建 `melody_path.db`。


## 无需登录的本地歌单分析（新增）

首页点击“立即本地分析”，直接粘贴每行一首的 `歌手 - 歌名`，或上传 TXT/CSV/JSON/M3U8。后端会：

1. 在本机解析歌单；
2. 联网时查询 Apple 公开音乐目录补全已有歌曲的 Genre、专辑、年代、时长和预览链接；
3. 查询失败时保留用户原始歌曲，不会让任务整体失败；
4. 配置 `LASTFM_API_KEY` 后，从真实种子调用 Last.fm 的相似歌曲、相似艺术家和关联标签接口生成候选，再由 Rust 在本地评分；
5. 未配置 Last.fm 或请求失败时明确显示空区，不会用 Apple 关键词搜索或 Demo 冒充推荐。

该流程不要求 Spotify、网易云、QQ 音乐等账号，不读取密码或 Cookie。Last.fm 推荐只需要后端环境变量 `LASTFM_API_KEY`，终端用户无需登录；Key 不返回前端也不写入日志。默认最多联网补全前 40 首，可通过 `LOCAL_ANALYSIS_MAX_TRACKS` 调整；公开目录 storefront 可通过 `ITUNES_STOREFRONT` 调整。

示例输入：

```text
BIBI - Kazino
DEAN - instagram
Mariya Takeuchi - Plastic Love
周杰伦 - 晴天
```

## 比赛 Demo 操作路径

1. 首页先讲解真实平台能力状态，再展开“更多导入方式”，点击 Demo 查看用户 A 的品味地图和数据声明。
2. 进入“探索推荐”，讲解舒适区、拓展区、惊喜区和 Korean R&B → Alternative R&B → Neo Soul 路线。
3. 点击“导出歌曲清单”，选择 10 首、预览、勾选明确确认并下载 CSV。
4. 点击“演示写入流程”，完成相同的预览—确认—结果链路；页面会持续标注这是虚拟写入。
5. 进入“好友桥梁”，展示 A/B 指标与桥梁歌单，再演示文件导出。
6. 进入“Agent 运行”，启动任一场景；展示 Rust SSE 进度并可中断。到“历史”加载结果和用量。

## Spotify 真实写入（可选）

1. 在 Spotify Developer Dashboard 创建应用，将 `http://127.0.0.1:3000/api/spotify/callback` 登记为 Redirect URI。
2. 复制 `.env.example` 中变量到当前终端环境，设置 `SPOTIFY_CLIENT_ID` 与 `SPOTIFY_CLIENT_SECRET`。
3. 重启后端。首页状态会从“需要配置开发者应用”变为“官方账号连接已支持”。
4. 点击“前往 Spotify 官方授权”。授权范围为 `playlist-read-private`、`playlist-read-collaborative`、`playlist-modify-public` 与 `playlist-modify-private`；项目不接收密码，不绕过登录/验证码。
5. 返回后页面显示 Spotify 昵称，可以选择一个或多个当前账号可访问的歌单，确认后查看统一 Track 预览。
6. 受 Spotify Developer Policy 限制，Spotify 来源数据不进行画像、衍生指标或 AI 分析；可用于用户主动发起的歌单传输/写回。Demo 或用户主动提供的非 Spotify 测试数据仍可展示完整推荐。
7. 选择歌曲后必须先匹配预览；中置信度候选需选择版本。明确确认后才创建新的私有歌单。页面“解除连接”会删除后端内存 token 和 HttpOnly 会话 cookie。

access/refresh token 仅保存在后端进程内存，不写入数据库、不返回前端、不记录日志；浏览器只持有不透明的 HttpOnly 会话 cookie。服务重启后需重新授权。完整验收见 [docs/spotify_acceptance.md](docs/spotify_acceptance.md)。

## 模型配置

设置页可以保存 OpenAI-compatible Endpoint、模型、温度、输出上限、价格与预算。`OPENAI_API_KEY` 只从后端环境变量读取，绝不通过设置页提交或保存到 SQLite。配置 Key 后，Agent 会在确定性结果之后调用 `/chat/completions` 增强路线解释，按服务端 usage 统计 Token 与估算费用，并遵守超时、重试和预算上限；调用失败会保留 Rust 结果并明确标记降级。未配置 Key 时 Token 和费用为真实的 `0`。

## 检查与测试

```powershell
cargo fmt --all --check
cargo check
cargo test
cd frontend
npm run lint
npm run build
```

Rust 测试覆盖文本解析、标题/版本标准化、统计指标、推荐去重与歌手集中度、桥梁约束、任务历史/中断、API 确认边界和文件转义。

## 关键代码导读

- `backend/src/engine.rs`：确定性音乐分析、推荐路线和桥梁约束。
- `backend/src/agent.rs`：可取消 Agent Loop、步骤/超时边界与 SQLite 历史。
- `backend/src/writers/mod.rs`：统一写回 trait、文件/Demo/预留适配器。
- `backend/src/writers/spotify.rs`：Spotify OAuth、刷新、匹配和写入。
- `backend/src/platforms.rs`：平台能力矩阵、官方域名白名单、公开链接识别与可访问性检查。
- `backend/src/main.rs`：REST、SSE、写回预览/确认与下载 API。
- `frontend/src/ExportModal.tsx`：选择—预览—版本确认—执行—报告界面。

## 常见问题

**没有 API Key 能演示吗？** 能。全部核心 Demo、分析、比较、Agent 进度和文件导出均离线运行。

**为什么 Apple Music 按钮仍提示尚未完成验收？** MusicKit 配置检测和 bootstrap 已实现，但尚未用真实 Apple Developer 账号完成歌单读写验收，因此不会把预留流程伪装成成功。YouTube 的官方 OAuth 与读写代码已实现，配置 Google Cloud 后可由账号持有人验收。

**为什么网易云/QQ音乐链接识别成功后仍可能不能分析？** “识别链接/页面可访问”不等于拥有合法稳定的曲目读取 API。未获得可验证官方权限时，MelodyPath 会明确停止并提供直接粘贴歌曲清单的备用入口。

**为什么 Spotify 选中的歌单不能直接做画像或推荐？** 当前 Spotify Developer Policy 禁止对 Spotify 内容进行分析、生成衍生指标或将其摄入 AI/ML。项目因此只把这些数据用于用户主动发起的歌单传输和写回。

**Demo 链接为什么是 `demo://`？** 它刻意表示虚拟歌单，不是可访问的真实平台资源。

**会修改原始歌单吗？** 不会。写回实现只创建新歌单，并且预览后需要用户明确确认。

## 文档

- [产品与比赛介绍](docs/product_intro.md)
- [3—5 分钟演示脚本](docs/demo_script.md)
- [系统架构](docs/architecture.md)
- [真实能力与限制](docs/limitations.md)
- [Spotify 手动验收](docs/spotify_acceptance.md)
- [Spotify、Google/YouTube 与 Apple 配置清单](docs/platform_setup.md)
- [平台能力与官方接入调查](docs/platform_capabilities.md)
