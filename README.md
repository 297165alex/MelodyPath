# MelodyPath

MelodyPath 是一个以 Rust 为可信计算核心、用可解释 Agent 帮助用户分析歌单、探索新音乐、比较好友品味并安全复制播放列表的本地优先 Web 应用。

# Overview

音乐平台擅长推荐“你已经喜欢的东西”，却很少解释为什么一首新歌适合你、它离当前偏好有多远，或两个人的歌单可以从哪里建立共同入口。MelodyPath 因此把歌曲解析、身份归一化、音乐画像、候选生成、确定性评分和执行权限放进一条可核验的工作流。

项目目前包含六个主要场景：

- **可解释音乐探索 Agent**：围绕用户目标选择白名单 Rust 工具，并根据真实工具结果继续、重规划或结束。
- **音乐偏好分析**：从文件或批量文本中计算 Genre、艺人、专辑、年代、重复曲目、合作曲目、集中度与多样性。
- **推荐探索**：用真实 Last.fm 候选和本地评分生成舒适区、拓展区与惊喜区，并保留来源、种子和推荐理由。
- **好友歌单比较**：比较两份用户提供的歌单，展示共同歌曲、共同艺人、相似度与双方都能理解的桥梁推荐。
- **跨平台复制歌单**：首个 MVP 面向 Spotify → YouTube，先匹配和预览，再由用户处理歧义并确认创建新的私有播放列表。
- **Version Radar / 版本雷达**：扫描已有歌曲，主动发现 Live、Concert、Remix、Acoustic、Unplugged、Remaster 等不同录音版本。

MelodyPath 严格区分真实文件、真实文本、真实账号数据、显式 Demo 与错误状态。真实流程失败时不会自动切换到 Demo，也不会用固定歌曲伪装成实时推荐。

# Quick Start

## Prerequisites

- 支持 Rust 2024 edition 的稳定 Rust 工具链（建议 Rust 1.85 或更高版本）
- Node.js 20.19+ 或 22.12+
- npm

真实 Last.fm 推荐需要 MelodyPath 部署者在后端环境中配置 `LASTFM_API_KEY`。普通用户不需要申请 Last.fm Key；没有部署者配置时，本地解析和音乐画像仍可运行，推荐区域会明确报告 Provider 未配置，不会回退 Demo。外部 LLM、Spotify 和 YouTube 均为可选配置，详见后文状态表。

## 编译

在项目根目录编译 Rust workspace：

```powershell
cargo build --workspace
```

在另一个终端中，从项目根目录安装前端依赖并编译：

```powershell
cd frontend
npm install
npm run build
```

## 配置 Endpoint / Key

根目录 `.env.example` 是环境变量名称和示例地址的参考模板，不包含真实凭据；不要将示例文件的存在视为配置已经生效。以下 Developer credentials 由 MelodyPath 部署者统一配置，普通用户不需要申请 Spotify、Google 或 Last.fm Developer Key。普通用户连接 Spotify / YouTube 时，只需在平台官方 OAuth 页面审阅权限并授权。Windows 本地部署可通过系统环境变量界面设置当前用户变量，重新打开终端后启动；`start-backend.ps1` 也会在运行时加载其列出的 User scope 变量。

- **基础功能无需 Key**：本地文件/文本解析与基础音乐画像可直接使用。
- **真实推荐**：部署者配置 `LASTFM_API_KEY`，用于 Discover 及 Agent 的真实推荐候选；普通用户无需 Last.fm Key，也无需登录 Last.fm。
- **LLM Agent**：在启动后端的进程环境中提供 `OPENAI_API_KEY`，并在应用 Settings 中配置 OpenAI-compatible Endpoint 和 Model，使其与 Key 所属服务一致。`.env.example` 没有定义 Endpoint / Model 环境变量，不要自行猜测变量名；启动脚本的 User scope 加载列表也不包含 `OPENAI_API_KEY`，需确保启动终端已继承该变量。未配置 Key 时明确使用 `DETERMINISTIC_FALLBACK`，当前真实 LLM 验收仍未完成。
- **Spotify 真实账号连接**：部署者配置 `SPOTIFY_CLIENT_ID`、`SPOTIFY_CLIENT_SECRET`、`SPOTIFY_REDIRECT_URI`，可按需设置 `SPOTIFY_MARKET`，并在 Spotify Developer Dashboard 登记完全一致的回调地址；普通用户只在 Spotify 官方页面授权。
- **YouTube / Google 真实账号连接**：部署者配置 `GOOGLE_CLIENT_ID`、`GOOGLE_CLIENT_SECRET`、`GOOGLE_REDIRECT_URI`，在 Google Cloud 启用 YouTube Data API v3、登记完全一致的回调地址并准备额度；普通用户只在 Google 官方页面授权。默认连接只申请 YouTube 只读权限；只有用户进入 Copy Playlist 写入流程时才单独申请写权限。模板另列的 `YOUTUBE_API_KEY` 不能替代账号 OAuth 授权。

本地后端与 API 地址参考 `MELODYPATH_BIND`、`API_BASE_URL` 和 `VITE_API_PROXY_TARGET`；模板使用后端 `http://127.0.0.1:3000`。`FRONTEND_URL` 应与本次实际前端地址一致，一键启动脚本会将选定地址传给后端；手动启动时请以前端启动日志或 `start-melodypath.ps1` 最终输出的 Frontend 地址为准。部署时可参考模板中的 `PUBLIC_BASE_URL` 及 OAuth 回调说明。

真实 Key 和 Secret 只保存在本机环境或安全凭据存储中，不应写入 README，也不得提交 Git。没有 Spotify / YouTube OAuth 配置或未完成授权时，账号连接不可视为可用，Mock 不代表真实平台验收。

## 1. 启动后端

在项目根目录执行：

```powershell
cargo run -p melody-path-api
```

后端默认监听 `http://127.0.0.1:3000`。可打开 `http://127.0.0.1:3000/health` 检查服务状态。

Windows 用户也可以使用安全启动脚本。它只在运行时读取 Windows User scope 中的环境变量，不打印变量值：

```powershell
.\start-backend.ps1
```

## 2. 启动前端

打开另一个终端：

```powershell
cd frontend
npm install
npm run dev
```

请以前端启动日志实际输出的 Local 地址，或 `start-melodypath.ps1` 最终输出的 Frontend 地址为准。

Windows 推荐在安装前端依赖后，在项目根目录一次启动前后端：

```powershell
.\start-melodypath.ps1
```

脚本会复用已检测到的前端或启动新前端，并重启已有的 MelodyPath 后端；子进程隐藏运行，验证后端健康状态后输出 `Frontend = ...`。不需要保持 PowerShell 窗口打开。

> 页面浏览提示
> 
> 由于 MelodyPath 包含较长的分析结果、Agent 执行轨迹和推荐内容，部分功能完成后，结果可能显示在当前视图之外。
>
> 如果点击按钮后没有立即看到变化，请尝试：
> - 向上滚动查看分析结果；
> - 向下滚动查看后续推荐区域；
> - 等待 Agent 或分析任务完成后查看完整页面。
> 
> 部分页面（如 Recommendation、Agent、Version Radar、Compare）会动态生成较长内容，请不要仅根据当前屏幕位置判断任务是否成功。



## 3. 上传歌单

> Spotify 与 YouTube / YouTube Music 已支持官方 OAuth 连接。授权后可选择账号内歌单，也可粘贴 Spotify 或 YouTube Playlist URL，通过官方 API 读取真实曲目并生成 Import Preview。Apple Music 与中国平台仍受各自官方能力和合作资格限制，可继续使用标准歌单文件或文本导入。
>
> 如果未来各平台提供更开放统一的官方接口，我也希望继续完善“一键连接 → 自动分析”的体验。

在首页展开本地导入区域，可以：

- 上传 `.txt`、`.csv`、`.tsv`、`.json`、`.m3u` 或 `.m3u8`；
- 粘贴每行一首的批量文本，例如 `歌手 - 歌名`；
- 明确点击 Demo，只体验与真实数据分离的演示流程。

文件或文本会先进入导入预览。页面展示总行数、成功解析数、警告、无法解析行、检测字段和前 20 首歌曲；用户确认后才开始分析。无法匹配元数据的歌曲仍保留原始歌名和歌手，并继续参与不依赖 Genre 的基础统计。

### 如何获得歌单文件？

如果你的音乐歌单来自其他平台，可以通过平台合法导出功能、第三方合法导出工具，或整理为 `歌手 - 歌名` 后导入。以下方式不代表 MelodyPath 已支持直接登录这些平台；不要使用 Cookie、模拟登录、逆向接口或私有 API 绕过限制。

### Spotify

推荐方式：
1. 使用支持 Spotify Playlist Export 的工具导出歌单；
2. 保存为 CSV 或 TXT；
3. 上传到 MelodyPath。

导出的文件至少包含：
Artist - Track Name
即可进行分析。

### Apple Music

可以：
- 导出播放列表信息；
- 或使用第三方导出工具生成 CSV；
- 再上传到 MelodyPath。

### YouTube Music

可通过：
- Google Takeout
- 第三方 Playlist Export 工具

获得歌曲列表后上传。

支持格式：
Artist - Track

### 网易云音乐 / QQ音乐 / 酷狗音乐 / 汽水音乐 / 其他平台

由于不同平台开放接口不同，目前 MelodyPath 不要求用户登录这些平台。

推荐方式：
- 导出歌单文件；
- 或复制歌曲列表；
- 或整理为：

歌手 - 歌名


格式后上传。


### 快速体验

如果暂时没有自己的歌单，可以直接使用项目提供的示例 CSV：


`examples/Happy_Mix.csv`


体验：

- Music Profile
- Recommendation
- Surprise Zone
- Agent Analysis


---

上传后，歌曲会先进入导入预览阶段。

页面会展示：

- 总行数；
- 成功解析数量；
- 无法解析歌曲；
- 检测到的字段；
- 前 20 首歌曲预览。


用户确认后才开始分析。

无法匹配 Genre 等额外信息的歌曲仍会保留原始歌名和歌手，并继续参与基础统计。

## 4. 体验主要页面

- **音乐画像**：确认导入后查看实际参与分析数量、Genre/Energy 覆盖率、艺人和专辑分布等结果。
- **推荐**：进入 `/discover` 查看 Comfort Zone、Expansion Zone、Surprise Zone、推荐种子、Provider 遥测和探索路线。真实候选需要配置 Last.fm。
- **Agent**：进入 `/agent`，绑定刚完成的真实 Analysis 或 Compare 结果，查看结构化决策、Rust 工具调用、SSE 进度、取消/恢复、Token 与费用。
- **Compare**：进入 `/compare`，分别导入 Friend A 与 Friend B 的歌单，确认后进行临时比较。
- **Version Radar**：进入 `/versions`，选择当前 Analysis 或本地文件并扫描其他录音版本。真实 YouTube 搜索需要完成 Google OAuth 与 YouTube Data API 配置。
- **Copy Playlist**：进入 `/transfer` 查看 Spotify → YouTube 的匹配、预览、确认和执行流程。真实复制需要两端开发者配置及用户本人授权。

### 课程演示用例

使用仓库示例 `examples/Happy_Mix.csv`，按以下顺序操作：

1. 按上述说明启动 MelodyPath，打开实际输出的前端地址。
2. 在首页进入本地歌单导入，上传 `examples/Happy_Mix.csv`。
3. 查看 Import Preview 并确认导入，然后查看 Music Profile。
4. 进入 Discover，查看 Comfort / Expansion / Surprise，再点击“换一批”。真实候选需要 `LASTFM_API_KEY`；未配置时检查明确的 Provider 未配置提示。
5. 进入 Agent，绑定刚才的分析结果，运行 Personal Music Exploration；没有 `OPENAI_API_KEY` 时检查 `DETERMINISTIC_FALLBACK` 标记。
6. 进入 Compare，在 Friend A、Friend B 分别导入并确认歌单，测试 Friend Bridge。可在两侧使用同一示例验证比较流程，观察品味差异时再使用另一份自备文件。
7. 进入 Version Radar，选择当前 Analysis，查看扫描流程及 OAuth 状态；真实 YouTube 搜索需要 Google OAuth 和 YouTube Data API 配置。
8. 进入 Copy Playlist，查看 Spotify → YouTube 流程和当前 OAuth 状态。只有完成双端配置及授权、预览匹配并明确确认后，才能创建新的 YouTube 私有播放列表。

演示无需用 Mock 冒充真实平台功能。Spotify / YouTube 官方 OAuth、身份、账号歌单读取和 Playlist URL 曲目导入已完成真人验收；当前 Spotify → YouTube 的最终真实写入仍未验收，配置、授权或写权限缺失时应展示实际阻塞状态。

# Features

> 小提示：MelodyPath 有时候会“认真工作但不主动把你拉到结果那里”。如果点击分析、推荐或 Agent 后暂时没有看到结果，请记得向上或向下滚动查看完整内容。
> 
## 1. Music Profile Analysis

所有真实输入都会先转换为统一的 `ImportedTrack` / `Track` 模型。当前导入支持：

- TXT
- CSV 与 TSV
- JSON
- M3U / M3U8
- 手动批量输入

CSV/TSV 使用 Rust `csv` 库解析，支持 UTF-8 BOM、引号和字段内逗号、中英文常见表头、多艺人、Genre、时长与真实 Energy 字段。文本中歌手与歌名顺序不明确时，预览页会要求用户确认，而不是直接猜测。

分析结果包括：

- **Music Profile**：核心偏好、相邻偏好、待探索方向、集中度、多样性与元数据置信度；
- **Genre**：统一规范化后的分布、核心 Genre 与覆盖率；
- **Artist**：艺人频次、合作歌曲与集中式跨语言艺人身份；
- **Feature**：专辑和年代分布、重复曲目、合作曲目、真实 Energy 均值与覆盖率。

Energy 缺失保持为 `None`，不会被当作 0，也不会生成虚构值。Genre 或外部元数据缺失不会删除用户原曲。

## 2. Explainable Recommendation

真实推荐从用户歌单中选择最多 10 首代表性种子，通过 Last.fm 的歌曲相似、艺人相似和标签关系生成候选，再由 Rust 完成身份归一化、版本过滤、源歌单排除、去重、同艺人上限和确定性排序。

推荐分为：

- **Comfort Zone / 舒适区**：与核心偏好接近，优先考虑高歌曲相似度、标签重合和熟悉风格。
- **Expansion Zone / 拓展区**：通过相似艺人、第一层相邻标签或相邻 Genre 增加探索距离。
- **Surprise Zone / 惊喜区**：差异更大，但必须保留一条可解释的偏好连接。

Surprise 不是随机推荐，也不会用固定歌曲补齐。Controlled Serendipity 会综合使用：

- Similar Artist；
- Similar Tag 与第二层标签路径；
- Genre Bridge 的两跳探索；
- second-hop similar artist 等可解释桥梁。

每首推荐可显示关联种子、来源接口、Last.fm 原始 similarity、系统综合 score、UI confidence、共享标签和理由。三者是独立字段，不会无条件显示成同一个百分比。候选池支持“换一批”和“查看更多”；同一 Analysis Session 内优先消费已保存候选，避免立即重复请求或重复展示。

## 3. Personal Music Exploration Agent

Personal Music Exploration Agent 不是按固定顺序播放动画的普通脚本。真实任务必须绑定当前 `analysis_id`，Agent 只能在 Rust 提供的 Tool Registry 白名单和已满足依赖的工具前沿中选择下一步：

```text
User Goal
    ↓
Agent Decision
    ↓
Rust Tool Call
    ↓
Tool Result
    ↓
Replan / Continue / Finish
```

工具结果会影响后续选择。例如惊喜候选不足时，下一步可以在 Genre Bridge 与 second-hop similar artist 路径中选择；不会永远执行一份不看结果的固定计划。

页面只展示可核验的结构化状态：User Goal、Normalized Intent、Decision Mode、当前 Plan、Tool Called、Tool Result Summary、Continue/Replan/Finish、Token Usage 与 Estimated Cost。它不展示隐藏 chain of thought，也不把自然语言解释当作音乐事实。

配置兼容的外部模型后，LLM 可参与高层决策。未配置 `OPENAI_API_KEY` 或模型调用失败时，运行会明确标记为 `DETERMINISTIC_FALLBACK`；fallback 仍使用真实绑定数据和真实 Rust 工具，不会切换 Demo。

## 4. Friend Bridge Agent

Friend Bridge 接收两份分别预览并确认的歌单，使用确定性指标比较：

- common songs；
- common artists；
- Track、Artist、Genre 与 Tag similarity；
- diversity complementarity；
- bridge recommendation。

桥梁候选分为 `Safe for Both`、`Bridge` 与 `Adventure Together`，同时排除两份源歌单，并分别保留对 Friend A、Friend B 的关联依据。比较默认是临时流程，不创建公开社交账号，也不默认保存好友歌单。真实候选不足时保持为空，不用 Demo 补齐。

## 5. Version Radar

独立 `/versions` 页面和扫描流程已经实现。用户可以从当前已分析歌单、本地文件或已授权的 YouTube 账号歌单中选择来源，并主动查找：

- Live
- Concert
- Remix
- Acoustic
- Unplugged
- Remaster

Version Radar 用于发现同一首歌在不同平台或不同录音版本中的可用形式。Rust 会比较基础标题、艺人身份、版本语义、官方艺人/Topic/VEVO 信号，以及双方都有真实时长时的时长差。扫描按每批 4 首、最多 40 首执行，页面提供进度和取消。

当前页面、批处理、版本分类、确定性匹配以及 `Add to playlist → preview → confirm` 安全流程均已实现。真实平台搜索依赖 Google OAuth 与 YouTube Data API 配置；未配置或未授权时显示 `BLOCKED_EXTERNAL_AUTH`。显式 Mock 只验证流程，不能视为真实平台搜索成功。

## 6. Copy Playlist

产品统一使用 **Copy Playlist / 跨平台复制歌单**；这里的“复制”明确表示保留源内容。当前课程 MVP 的目标流程是：

```text
Spotify Source Playlist
        ↓
YouTube Matching
        ↓
Preview
        ↓
User Confirmation
        ↓
Create a new private YouTube Playlist
```

后端会分页读取用户拥有或参与协作的 Spotify 歌单，转换为统一 `TransferTrack`，再通过 YouTube 官方搜索为每首保留最多 5 个候选。匹配分数考虑标题、艺人、真实时长差、官方/Topic 频道信号和 Live、Cover、Remix、Sped Up 等版本词，输出 `MATCHED_HIGH`、`MATCHED_AMBIGUOUS` 或 `UNMATCHED`。

歧义和不同版本候选必须由用户改选或跳过；明确确认前禁止创建播放列表。执行时只创建新的私有 YouTube 播放列表，单首失败不会终止整个任务，并提供 run id、SSE 进度、取消/恢复、逐首结果及 CSV/JSON 报告。

**原 Spotify 歌单不会被修改或删除。** 当前 Copy 会话保存在后端进程内，服务重启后需要重新生成预览，不能宣称跨重启恢复。代码与 Mock 已验证，但真实 Spotify → YouTube OAuth 端到端复制尚未完成，因此当前没有真实目标播放列表链接。

# Platform Support

状态定义：

- **REAL VERIFIED**：已用真实外部账号或 Provider 完成端到端验收。
- **IMPLEMENTED BUT CONFIG REQUIRED**：官方接口代码已实现，但仍需要开发者配置、额度或用户人工授权。
- **IMPORT ONLY**：当前只能使用用户主动提供的文件或粘贴文本。
- **UNSUPPORTED**：当前没有合法、稳定且已验证的实现。

| 平台 | 读取能力 | 写入能力 | 当前状态 |
|---|---|---|---|
| Spotify | 官方 OAuth、真实账号身份、用户歌单、曲目分页和已授权 Playlist URL 导入均已真人验收 | 新建私有歌单与批量加入代码已实现；最终真实写入尚待验收 | **REAL VERIFIED**（账号连接与读取） |
| YouTube / YouTube Music | Google OAuth、真实账号身份、用户播放列表、playlistItems 和已授权 Playlist URL 导入均已真人验收；Version Radar 搜索也使用该授权 | 新建私有播放列表与插入视频代码已实现；最终真实写入尚待验收 | **REAL VERIFIED**（账号连接与读取） |
| Apple Music | 当前没有 MusicKit 账号读取 Connector；只接受用户导出的文件或文本 | **UNSUPPORTED** | **IMPORT ONLY / CONNECTOR NOT IMPLEMENTED** |
| NetEase / 网易云音乐 | 文件或粘贴文本；公开链接目前只做识别和可访问性检查，不能读取曲目 | **UNSUPPORTED** | **IMPORT ONLY** |
| QQ Music / QQ音乐 | 文件或粘贴文本；没有已验证的通用个人歌单 OAuth | **UNSUPPORTED** | **IMPORT ONLY** |
| Kugou / 酷狗音乐 | 文件或粘贴文本；官方个人歌单能力需要正式合作资格 | **UNSUPPORTED** | **IMPORT ONLY / PARTNERSHIP REQUIRED** |
| Qishui / 汽水音乐 | 文件或粘贴文本；公开链接仅识别格式，没有已验证的官方曲目读取接口 | **UNSUPPORTED** | **IMPORT ONLY / URL RECOGNITION ONLY** |

Last.fm 推荐 Provider 已使用五份真实歌单完成串行浏览器验收，状态为 **REAL VERIFIED**；Developer Key 由 MelodyPath 部署者配置，普通用户无需 Key，也无需登录 Last.fm。Spotify 与 YouTube / YouTube Music 的官方 OAuth、真实身份、用户歌单读取和 Playlist URL 导入均已由账号持有人完成真人验收，状态为 **REAL VERIFIED**。普通用户无需自己的 Developer credentials，只需在官方 OAuth 页面授权。

完成对应平台官方 OAuth 后，Spotify 与 YouTube Playlist URL 可通过官方 API 读取真实曲目并生成 Import Preview。Apple Music、网易云、QQ、酷狗和汽水仍只保留当前真实的 URL 识别、页面可访问性检查或不支持状态；“可识别”或“页面可访问”不等于能够合法读取完整曲目。

# Architecture

```text
React + TypeScript UI
  ├─ Import preview / Analysis / Discover
  ├─ Compare / Version Radar / Copy Playlist
  └─ Agent trace / History / Settings
                 │ REST + SSE
                 ▼
Rust + Axum Backend
  ├─ Decision Layer
  ├─ Tool Registry
  ├─ Deterministic Rust Tools
  ├─ Last.fm / metadata providers
  └─ Spotify / YouTube official connectors
                 │
                 ▼
SQLite + process-local session state
```

- **Frontend — React + TypeScript + Vite**：负责导入预览、结果可视化、结构化 Agent 轨迹、歧义确认和执行进度。
- **Backend — Rust + Axum**：负责解析、规范化、画像、推荐、比较、版本判断、Copy 匹配、权限检查、REST 与 SSE。
- **Storage — SQLite**：保存 Agent 目标、意图、计划、工具结果、历史、检查点、设置、Token 和费用记录；OAuth Secret 不写入 SQLite。导入预览、Analysis Registry 与 Copy run 当前包含进程内状态。
- **Agent**：由 Decision Layer、白名单 Tool Registry 和确定性 Rust Tools 组成。外部模型不是数据处理器，也没有 shell、任意文件、任意 HTTP 或任意 SQL 权限。

关键实现位置：

- `backend/src/import.rs`：统一文件与文本导入；
- `backend/src/engine.rs`：音乐画像、比较指标和桥梁逻辑；
- `backend/src/recommendation.rs`：Last.fm 候选、三区评分、过滤和遥测；
- `backend/src/agent.rs`：结构化决策循环、工具执行、SSE、取消/恢复和历史；
- `backend/src/alternate.rs`：Version Radar 的版本判断与评分；
- `backend/src/transfer.rs`：Copy Playlist 的统一模型、匹配、确认和执行；
- `backend/src/writers/spotify.rs`、`youtube.rs`：官方 OAuth 与平台读写；
- `frontend/src/App.tsx`：主要页面与路由；
- `frontend/src/ExportModal.tsx`：统一的选择、匹配预览和确认写入界面。

# Agent Design

在模型配置可用时，LLM 只负责高层决策：

- intent understanding；
- planning；
- 从当前允许的工具中进行 tool selection；
- 根据紧凑 `ToolResult` 决定 continue、replan 或 finish；
- 生成不改变确定性结果的 explanation。

Rust 始终负责核心确定性工作：

- parsing；
- title、artist、Genre 与 version normalization；
- recommendation ranking、dedup 与源歌单排除；
- Compare metrics 与 bridge constraints；
- Transfer matching；
- permission、确认闸门与 external writes。

模型输出必须符合 serde 校验的结构化 `AgentDecision`，未知工具会在执行前被拒绝。Agent 还受最大步骤、总超时、重试、输出 Token 和费用上限保护。核心音乐计算不会交给 LLM 猜测；Spotify API 内容也不会发送给 LLM。

当前真实浏览器 Agent 使用真实导入数据与真实 Rust 工具完成，但因为环境中没有可验证的外部 LLM 配置，Decision Mode 为 **DETERMINISTIC_FALLBACK**，Token 与费用为 0。该状态不能写成 REAL LLM VERIFIED。

# Security and Privacy

MelodyPath 不会：

- 获取或保存 Spotify、Google 或其他音乐平台密码；
- 读取用户 Cookie；
- 模拟登录、绕过验证码或调用私有/逆向接口；
- 在用户确认前创建或修改外部播放列表；
- 删除或修改 Copy 的源歌单；
- 把 Spotify API 内容发送给 LLM、用于画像、衍生指标或模型训练。

所有账号连接只允许走官方 OAuth。OAuth state 使用一次性校验；浏览器只持有不透明的 HttpOnly、SameSite 会话 Cookie。Access token、refresh token、API Key 和 Client Secret 不返回前端、不写入日志、不提交 Git。Windows 本地开发使用当前用户范围的 DPAPI 加密 OAuth 会话并存放在项目目录之外；生产部署仍需要托管 Secret/KMS、HTTPS、安全 Cookie、密钥轮换与审计。

`.env`、数据库、token 文件、私人歌单、构建目录和运行日志均不应提交到公开仓库。仓库中的 `.env.example` 只提供变量名，不包含真实凭据。

# Testing

最近一次最终审计记录（2026-09-07）：

| 检查 | 结果 |
|---|---|
| `cargo fmt --all --check` | PASS |
| `cargo check --workspace` | PASS |
| `cargo test --workspace` | **128 PASS / 0 FAIL / 1 ignored（显式联网探测）** |
| `frontend/npm run lint` | PASS |
| `frontend/npm run build` | PASS |
| `frontend/npm run test:e2e` | **12/12 PASS** |

Rust 测试覆盖导入格式与大歌单、Genre/艺人/版本规范化、Energy 缺失、Last.fm Provider 降级、三区推荐、候选池换批、源歌单排除、Compare、结构化 AgentDecision、非法工具、动态下一步、步数/费用限制、真实 Analysis 绑定、Copy 分页与匹配、歧义确认、失败隔离、取消/恢复，以及 OAuth 安全边界。

真实数据验证包括：

- 五份不同规模的真实歌单已串行完成浏览器导入与 Last.fm 推荐，全部 `is_demo=false`，三区均产生真实候选，且推荐集合明显不同；
- 真实文件 Personal Agent 与真实文本 Friend Bridge 已验证不会读取 Demo payload；
- Happy_Mix 回归记录为 50/50 解析、`REAL_FILE`、13 个真实 Rust 工具调用完成，Agent 结果与绑定 Analysis 一致；
- `/`、`/discover`、`/compare`、`/versions`、`/transfer`、`/agent` 已完成本地浏览器直达检查，终检没有新增关键控制台错误。
- Spotify 官方 OAuth、真实账号身份、真实用户歌单以及真实 Playlist URL 的 16 首曲目 Import Preview 已通过真人验收；返回 `TRACK_IMPORT_AVAILABLE`。
- YouTube 官方 Google OAuth、真实账号身份、真实用户歌单以及真实 YouTube Playlist URL 曲目导入已通过真人验收。

以下仍不属于真实外部验收：外部 LLM 多步决策、真实 YouTube 版本搜索、真实跨平台播放列表创建与写入。相关 Mock 只证明程序流程和安全闸门，不证明这些尚未验收的外部操作已完成。

# Limitations and Future Work

项目已经尝试并完成 Spotify 一键授权流程、YouTube OAuth 流程和统一多平台能力矩阵的代码实现，但当前仍有明确限制：

- **OAuth 配置**：Spotify 与 YouTube 的开发者应用、Client 配置和完全一致的 Redirect URI 由 MelodyPath 部署者提供；普通用户只需在官方页面授权。默认 YouTube 连接只读，Copy Playlist 写入时才单独请求写权限。
- **平台官方 API 权限**：开发模式资格、审核、YouTube 配额及中国平台合作权限不由仓库代码自动获得。
- **真实 LLM 验收**：Controller、schema、预算和 fallback 已实现，但真实外部 LLM 决策环仍受配置阻塞。
- **Version Radar**：页面和本地流程已实现，真实 YouTube 搜索与写入仍需 OAuth/API 配置。
- **Copy 持久性**：run id、SSE、取消和恢复在单个后端进程内工作，尚不支持服务重启后的任务恢复。
- **公开分享链接**：Spotify / YouTube 在官方授权后可通过官方 API 导入真实曲目；Apple Music 与中国平台仍只支持现有的域名/ID 识别、页面可访问性检查或文件导入。
- **公网部署**：同域部署参数和安全边界已准备，但尚无公开域名、HTTPS 证书、生产 Token Store 或公开验收地址。

未来工作包括：

- 使用官方 MusicKit / Apple Music API 实现 Apple Music 账号连接、Library Playlist 读取和歌单创建/写入；手工 CSV 只作为备用入口，而不是目标主要体验；
- 在获得明确官方资格后增加更多 Connector，不使用 Cookie、私有接口或逆向方案；
- 为生产环境接入托管 Secret/KMS、共享任务存储与队列；
- 完成 Spotify → YouTube 真实写入、真实 YouTube 私有播放列表链接和公网部署验收。

# Course Requirement

MelodyPath 对课程 R1–R6 的当前审计状态如下。`PARTIAL` 表示代码或自动测试已存在，但真实外部配置验收尚未完成，不会被包装成 PASS。

| 要求 | 状态 | 项目证据 |
|---|---|---|
| **R1 — Rust Core** | **PASS** | 导入、规范化、身份、Genre、元数据、种子、Last.fm Provider、推荐评分、去重、版本分类、Compare、Copy 匹配、权限和 Agent 状态均由 Rust 完成。 |
| **R2 — User Interface** | **PASS** | React 页面覆盖 Analyze、Discover、换批、Compare、Version Radar、Copy Playlist、Agent、History、Settings 和平台状态。 |
| **R3 — Configurable Model** | **PARTIAL** | Endpoint、模型、Temperature、Token、超时、重试、价格与费用上限可配置；真实外部 LLM 决策环仍未验收。 |
| **R4 — Realtime Progress and Interrupt** | **PASS** | Agent 与 Copy 均有 SSE、取消和恢复；Copy 当前仅保证单进程会话恢复，不跨服务重启。 |
| **R5 — History Management** | **PASS** | SQLite 保存 Agent goal、intent、data state、decision mode、plan、tool calls/results、状态、时间、warning 和 checkpoint，不保存 OAuth Secret。 |
| **R6 — Token and Cost Statistics** | **PARTIAL** | Token/费用记录、估算和费用上限测试通过；当前真实 Agent 为 fallback，非零外部模型用量尚未验收。 |

项目至少实现了以下场景定制，而不是把通用聊天 Agent 换一个名称：

1. **领域知识库**：集中 Genre ontology、Ballad 系列桥梁、跨语言艺人身份和 Original/Live/Remix/Acoustic 等录音版本语义。
2. **专用工具链**：真实导入 → 画像 → 种子选择 → Last.fm 候选 → Rust 评分/过滤 → 路线，以及双歌单 Compare 和 Copy 匹配工具。
3. **定制 Prompt 与结构化输出校验**：模型只能返回 serde 校验的 `AgentDecision`，并只能选择 Tool Registry 白名单中的当前可用工具。
4. **真实结果驱动的重规划**：候选为空、惊喜区不足或双方重合度变化，会改变下一工具，而不是始终执行相同脚本。

MelodyPath 相比通用 Agent 的专用性体现在：它理解 Genre 距离、相似歌曲/艺人/标签关系、艺人别名、录音版本、源歌单排除、同艺人上限和平台数据政策；它能把每个推荐、比较和外部写入追溯到真实输入、Rust 工具结果与明确权限。LLM 负责高层决策和说明，音乐事实、评分、匹配与安全边界始终由可测试、可复现的 Rust 实现控制。
