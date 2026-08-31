# MelodyPath 开发交接：Transfer MVP

## 1. 当前状态与最高优先级

MelodyPath 已具备本地歌单导入、确定性分析、Last.fm 可解释推荐、好友桥梁、Agent Loop、历史记录和导出等能力。下一阶段最高优先级不是继续扩展推荐平台，而是完成一个独立、可验收的 Spotify → YouTube 跨平台歌单迁移 MVP。

目标用户流程：

> 连接 Spotify 与 YouTube 官方账号  
> → 从当前用户拥有或参与协作的 Spotify 歌单中选择一个  
> → 分页读取全部曲目  
> → 在 YouTube 搜索并确定性匹配候选  
> → 用户确认歧义、改选或跳过  
> → 创建新的私有 YouTube 播放列表并逐首写入  
> → 查看进度、结果和 CSV/JSON 报告

这一流程是核心 Agent 执行场景：它需要明确状态、工具调用、人工确认、可中断执行和可审计结果，不是让 LLM 生成歌曲推荐。

## 2. 已完成能力

### 本地分析与推荐

- CSV、TSV、JSON、TXT、M3U/M3U8 和批量文本导入预览。
- 真实输入、Demo、错误等数据状态隔离，真实失败不会自动回退 Demo。
- Rust 本地统计：歌曲、歌手、专辑、年代、合作、重复、集中度、多样性、Genre 和 Energy 覆盖率。
- 统一 Genre 规范化与缺失 Energy 处理。
- `LastFmRecommendationProvider` 使用真实种子、`track.getSimilar`、相似艺术家和 tag 路径生成候选，再由 Rust 本地评分。
- 舒适区、拓展区、惊喜区展示种子、来源接口、相似度、分数、理由和 Provider 状态；真实模式不使用 Apple 关键词搜索或固定 Demo 兜底。

### 平台与执行基础

- Spotify 官方 Authorization Code Flow、HttpOnly 会话、后端内存 token、刷新和解除连接。
- Spotify 当前用户信息、可访问歌单分页、歌单曲目分页和统一 `Track` 转换。
- YouTube/Google 官方 OAuth、频道信息、拥有的播放列表分页、playlistItems 分页和标题清理。
- YouTube 搜索、候选匹配、新建播放列表和逐首加入视频的基础 writer 代码。
- 统一 `PlaylistWriter`、匹配状态、候选预览、明确确认、单曲错误结果和导出结果模型。
- Rust Agent Loop、SSE 实时进度、任务中断、失败终态、最大步数和 SQLite 历史。
- CSV、JSON、M3U8 文件导出，以及前端预览—确认—执行—结果链路。

### 当前真实验收状态

- Last.fm Provider 已完成真实请求验收，真实文件保持 `is_demo=false`。
- Spotify 与 YouTube OAuth/读写代码存在，但仍不能仅凭 Mock 或代码检查宣称 Spotify → YouTube 真实迁移已成功。
- YouTube 真实写入需要账号持有人完成 Google Cloud 配置、登录授权和最终播放列表链接核验。

## 3. 推荐模块已知限制

这些限制已记录在 `docs/recommendation_acceptance_report.md`，Transfer 开发不得通过删除或重构推荐模块来绕过：

1. 同曲不同版本过滤仍有缺口，例如 Remix 与 Bonus Track 候选可能未正确排除或合并。
2. 跨语言艺术家别名可能绕过源歌单排除，例如 `Jay Chou / 周杰倫 / 周杰伦`、`JJ Lin / 林俊傑 / 林俊杰`。
3. 推荐统计目前不能完整拆分版本过滤、规范化去重、源歌单排除、艺术家上限及 tag 第一/第二层淘汰数量。
4. 已有真实验收中惊喜区可能诚实为空；不得使用 Demo 或固定歌曲补齐。

这些问题应保留为独立推荐修复事项，不应阻塞或污染 `/transfer` 的确定性匹配设计。

## 4. Transfer MVP 完整产品边界

### 平台范围

- 源平台仅 Spotify。
- 目标平台仅 YouTube 播放列表。
- 第一版不要加入 Apple Music、网易云、QQ音乐、酷狗或酷我。
- 不修改或删除源 Spotify 歌单；每次执行只创建一个新的 YouTube 播放列表，默认私有。

### 连接与源歌单

- 使用 Spotify 和 Google/YouTube 官方 OAuth，不接收密码、Cookie，不模拟登录或调用私有接口。
- 页面分别显示 Spotify、YouTube 的未连接、已连接、连接失败状态；两端都授权后才能迁移。
- Spotify 只列出当前用户拥有或参与协作且有权读取的歌单。
- 每次只选一个源歌单，显示名称、封面和歌曲数，并自动分页读取全部曲目。

### 统一迁移模型

建立独立 `TransferTrack`，至少包含：

- `title`
- `artists[]`
- `album?`
- `duration_ms?`
- `isrc?`
- `source_platform`
- `source_track_id`
- `source_url?`
- `normalized_title`
- `normalized_artists[]`

Spotify 数据不得发送给 LLM。匹配、版本判断和分数计算必须由确定性程序完成。

### YouTube 搜索与评分

- 优先搜索词：`歌手名 歌名 official audio`。
- 每首源歌曲最多保留 5 个 YouTube 候选。
- 评分至少考虑标题相似度、艺术家相似度、时长差、官方艺人/Topic 频道、official audio 标识和版本词。
- 当源歌曲本身没有对应标记时，Live、Cover、Karaoke、Remix、Sped Up、Slowed、Instrumental、Reaction 等候选必须降权。
- 匹配状态：`MATCHED_HIGH`、`MATCHED_AMBIGUOUS`、`UNMATCHED`。
- 每个结果必须保留分数和可解释依据，不使用 LLM 猜测。

### 预览、确认与执行

- 写入前展示完整预览：源歌曲、选中候选、最多 5 个备选、分数、依据以及高置信度/歧义/未匹配统计。
- 歧义歌曲必须允许用户改选候选、重新搜索或跳过。
- 未确认的歧义歌曲不得自动写入；整次迁移未明确确认前不得创建目标播放列表。
- 确认后创建默认名为 `原歌单名 · Transferred by MelodyPath` 的私有 YouTube 播放列表。
- 逐首写入；单首失败继续后续歌曲；实时显示进度并支持中断。
- 结果包含原始数、匹配数、写入成功、写入失败、跳过、未匹配和新播放列表链接。
- 支持下载 CSV 与 JSON 报告，记录源歌曲、目标候选、分数、最终状态和错误原因。

## 5. 可复用代码

### Spotify

- `backend/src/writers/spotify.rs`：OAuth state、授权回调、后端会话、token 刷新、连接状态、歌单与曲目分页、统一 Track、搜索/匹配及 Spotify writer。
- `backend/src/main.rs`：`/api/spotify/authorize`、callback、连接状态、歌单列表、导入和解除连接路由。
- `frontend/src/api.ts` 与 `frontend/src/types.ts`：Spotify 状态、列表和导入 API 类型及调用。

Transfer 应复用 OAuth、会话和分页逻辑，但不要调用 Spotify 写回去修改源歌单。

### YouTube

- `backend/src/writers/youtube.rs`：Google OAuth、连接状态、token 刷新、播放列表/曲目分页、标题清理、候选搜索、匹配、新建播放列表和加入视频。
- `backend/src/main.rs`：YouTube authorize、callback、连接状态、列表、导入和解除连接路由。
- `frontend/src/api.ts` 与 `frontend/src/types.ts`：YouTube 状态、列表和导入 API 类型及调用。

Transfer 应把现有候选搜索与 writer 能力组合进独立服务，补齐最多 5 个候选、歧义确认和迁移任务状态；不要把该流程塞进推荐模块。

### 进度、中断与历史

- `backend/src/agent.rs`：可取消任务、状态转换、进度、失败终态、超时/步数边界和 SQLite 历史。
- `backend/src/main.rs`：任务创建、查询、SSE 事件和取消路由。
- 前端现有 Agent 页面可作为状态展示参考，但 `/transfer` 应保持独立用户流程。

复用状态机思想和安全边界即可；不要让迁移任务调用 LLM，也不要把 OAuth token 写入 SQLite。

### 匹配与导出

- `backend/src/models.rs`：统一 `Track`、`MatchCandidate`、`PlatformTrackMatch`、`MatchStatus`、`TrackFailure` 和导出结果模型。
- `backend/src/writers/mod.rs`：统一 writer trait、候选匹配和写入结果抽象。
- `backend/src/main.rs`：现有 `/api/exports/preview`、`/api/exports/execute` 和报告下载路径中的明确确认边界。
- `frontend/src/ExportModal.tsx`：预览、候选选择、确认、执行和结果界面模式。

Transfer 可以复用模型或交互模式，但应建立独立 transfer session，避免与普通文件导出或推荐导出混淆。

## 6. 建议实施顺序

1. **只读审计**：阅读 README、PLAN、本交接文档和平台配置文档；确认现有 Spotify/YouTube OAuth、token 生命周期和 API scope。
2. **独立领域模型**：新增 `TransferTrack`、候选、预览、执行项、任务状态和最终报告模型，不改推荐模型。
3. **Spotify 源读取**：复用官方 OAuth 和分页；只返回用户拥有或协作的歌单；选择一个并读取全部曲目。
4. **YouTube 候选服务**：每首最多 5 个候选；实现标题、艺术家、时长、官方频道和版本词的确定性评分及三种匹配状态。
5. **后端预览 API**：创建 transfer session，返回完整匹配预览；支持歧义改选、重新搜索和跳过。
6. **独立 `/transfer` 页面**：显示双端连接、单歌单选择、读取数量、逐首匹配解释和明确确认控件。
7. **执行任务**：确认后才创建私有 YouTube 播放列表；逐首加入、记录错误、发送进度并支持中断。
8. **结果与报告**：显示汇总、新播放列表链接，并提供 CSV/JSON 下载。
9. **Mock 与自动测试**：覆盖 Spotify 分页、YouTube 搜索、评分、版本降权、歧义、无匹配、单首失败继续、中断、未确认禁止写入以及源歌单不变。
10. **完整静态检查**：运行 Rust fmt/check/test 与前端 lint/build。
11. **真实 OAuth 检查点**：缺少任一配置或人工授权时立即停止；配置齐全后只用 5—10 首测试歌单完成真实浏览器验收。

## 7. 真实 OAuth 人工配置

### Spotify

- 在 Spotify Developer Dashboard 创建开发者应用。
- 配置 Client ID 与 Client Secret 为后端环境变量；不要写入项目文件或前端。
- 登记与后端路由一致的 Redirect URI，例如本地 callback 地址。
- 确认所需 scope 至少覆盖私人/协作歌单读取；Transfer 不需要修改源 Spotify 歌单。
- 由用户在 Spotify 官方授权页面手动登录并同意授权。

### Google / YouTube

- 在 Google Cloud 项目中启用 YouTube Data API v3。
- 配置 OAuth consent screen 和 OAuth Client。
- 将 Client ID、Client Secret 和 Redirect URI 作为后端环境变量，不写入仓库或前端。
- 确认 scope 可读取账号频道并创建、管理当前用户自己的 YouTube 播放列表。
- 由用户在 Google 官方页面手动登录和授权；还需关注 YouTube API quota。

### 真实验收通过条件

只有以下事实全部满足，才能标记真实 OAuth 迁移通过：

1. 两端连接状态均来自真实 OAuth 会话；
2. Spotify 真实读取一个 5—10 首的用户歌单及全部曲目；
3. 用户在浏览器中看到并确认完整匹配预览；
4. 歧义项经过人工确认或跳过；
5. YouTube 成功创建新的私有播放列表并逐首写入；
6. 页面返回可访问的新 YouTube 播放列表链接；
7. 源 Spotify 歌单未被修改；
8. 报告准确记录成功、失败、跳过和未匹配项。

缺少 Client 配置、Redirect URI、API 启用、额度或人工授权时必须停止并报告。不得用 Mock、截图占位、固定 URL 或 Demo 结果冒充真实验收。

## 8. GitHub 与秘密信息

GitHub 中不得提交或附带：

- API Key；
- Spotify 或 Google Client Secret；
- OAuth access token、refresh token 或会话 cookie；
- `.env` 或任何包含真实凭据的变体；
- `melody_path.db`、其他数据库、token 缓存或运行日志中的敏感内容；
- 用户私人歌单、账号身份信息或未脱敏的迁移报告。

提交前检查忽略规则和变更列表。文档与测试只能使用占位符、Mock 数据或经过明确授权且已脱敏的样例；任何真实 Secret 都只能存在于后端进程可读取的安全环境变量中。
