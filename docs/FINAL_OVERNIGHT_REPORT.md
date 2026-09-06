# MelodyPath 最终夜间开发报告

日期：2026-09-04  
项目：`C:\Users\user\Downloads\melody-path-local-analysis`

## 最终结论

本轮已完成所有不依赖人工账号授权的实现、自动测试、本地浏览器验收和交接文档。项目保持原有本地分析、Last.fm 推荐、历史、Agent Loop、好友桥梁和导出能力，并新增或补齐：

- 推荐版本/别名过滤与逐阶段统计；
- 可持久化、可取消/恢复的 Rust Agent 状态模型；
- 独立 `/compare`；
- Alternate Versions Explorer；
- 独立 Spotify → YouTube `/transfer` MVP；
- 推荐/共同推荐/版本候选进入统一 Playlist Writer；
- 外部请求超时、有限重试、YouTube 时长获取和错误隔离；
- 能力状态、架构、验收和接续文档。

最终状态不是“全部真实平台通过”。真实 Last.fm 回归与真实 Spotify → YouTube OAuth 迁移分别处于 `BLOCKED_EXTERNAL_CONFIGURATION` 和 `BLOCKED_EXTERNAL_AUTH`。

## 已实现并验证

### 推荐可靠性

- 集中版本解析：Original、Live、Concert、Remix、Acoustic、Unplugged、Bonus Track、Remaster、Sped Up、Slowed、Radio Edit、Instrumental、Karaoke、Cover、Reaction、Nightcore。
- `Dancin` 排除 `Dancin - Krono Remix`，`Bang Bang` 排除 `Bang Bang (Bonus Track)`；源曲明确是 Remix 时保留版本语义。
- 集中 ArtistIdentity 支持稳定 ID、规范化名称及 Jay Chou/周杰倫/周杰伦、JJ Lin/林俊傑/林俊杰别名。
- 响应/UI 分开 Last.fm 原始 similarity、系统 score 和 UI confidence。
- 流水线统计拆分原始、版本过滤、规范化、去重、源歌单排除、艺人上限、三区及 tag 第一/第二层。
- 真实模式不调用 Apple 关键词搜索，不回退固定 Demo 候选。

### Rust Agent

- 结构化 Goal/Intent/Plan/Step/ToolCall/Result/State/Run。
- 状态、有限重试、超时、最大步骤、取消、恢复与 SQLite 检查点。
- 浏览器实际运行个人 Genre 场景，显示 13 个真实计划步骤、工具、尝试次数、100% 进度和 `COMPLETED`。

### Compare 与 Alternate Versions

- `/compare` 两侧文件/文本分别预览，再明确确认临时比较；动态 Track/Artist/Genre/Tag/多样性指标。
- 共同候选排除双方源歌单；Provider 不可用时保持空且不补 Demo。
- Alternate Versions 只有用户主动展开并选择版本后搜索；同名异艺人和假版本关键词受确定性校验。
- 真实 YouTube 不可用时显示 `BLOCKED_EXTERNAL_AUTH`；显式 Mock 独立标记。

### Transfer MVP

- `TransferTrack` 与 Source/Destination Connector 边界。
- Spotify 官方只读来源复用；YouTube 官方搜索/写入复用。
- 每首最多 5 个候选；标题、艺人、真实时长、官方/Topic、版本词确定性评分。
- `MATCHED_HIGH / MATCHED_AMBIGUOUS / UNMATCHED`。
- Alternate fallback 默认关闭；版本回退仍需人工确认。
- 未确认禁止创建目标；只创建新私有列表；单首失败继续；源 Spotify 不修改。
- CSV/JSON 报告；核心执行器的取消与恢复有自动测试。

## 自动质量检查

全部通过：

| 命令 | 结果 |
|---|---|
| `cargo fmt --all --check` | 通过 |
| `cargo check --workspace` | 通过；5 个非阻塞 dead-code 警告 |
| `cargo test --workspace` | 84 passed，0 failed |
| `frontend/npm run lint` | 通过 |
| `frontend/npm run build` | 通过 |
| `git diff --check` | 通过；仅提示 Windows LF/CRLF 转换 |

Rust 测试覆盖导入、Genre/版本/艺人规范化、推荐候选与统计、Provider 失败不回 Demo、Compare、Agent 重试/取消/恢复、Transfer 分页/评分/确认/失败继续/取消/恢复/Unicode/500 首和 OAuth 安全边界。

## 浏览器验收

环境：一套后端 `http://127.0.0.1:3000`，一套前端 `http://127.0.0.1:5174`；`/health` 为 200，浏览器控制台无 warning/error。

### 真实 Compare

- A：Carly Rae Jepsen、Dua Lipa、The 1975，共 3/3 解析。
- B：Miles Davis、John Coltrane、Bill Evans，共 3/3 解析。
- 结果：`REAL_TEMPORARY_COMPARISON`、`is_demo=false`；0 共同曲目、0 共同艺人。
- Last.fm 未配置时共同推荐为空，页面明确没有使用 Demo。

### Transfer

- 上传 `testdata/Japanese.csv`：76/76 解析，来源为真实文件，源列表只读。
- 真实 YouTube Provider：`BLOCKED_EXTERNAL_AUTH`，0 高置信、0 歧义、76 未匹配，没有切换 Mock。
- 用户明确选择 Mock：76 高置信、0 歧义、0 未匹配；确认前执行按钮禁用。
- Mock 执行：76 成功、0 失败、0 跳过、0 未匹配，100%；结果为 `mock://`，没有冒充 YouTube。

### Alternate Versions 与加入歌单

- 真实 YouTube 搜索正确显示 `BLOCKED_EXTERNAL_AUTH`，没有候选或 Mock 回退。
- 显式 Mock 显示 Live、Remix、Acoustic 候选及 Mock 来源。
- `Add to playlist` 进入共用预览；未勾选最终确认前“创建”按钮禁用。

## 外部阻塞

### Last.fm

当前启动进程无法读取 `LASTFM_API_KEY`，Provider 状态为未配置。因此 Japanese、Korean、English、Chinese 四文件在版本/别名修复后的真实联网回归没有执行。历史真实数据保留在推荐验收报告中，但不能作为本轮修复后的通过证据。

### Spotify 与 Google/YouTube

缺少或未完成人工提供的开发者配置、Redirect URI 登记、YouTube Data API/额度及两端账号登录授权。没有读取真实 Spotify 测试歌单，没有创建真实 YouTube 私有列表，也没有真实播放列表链接。

这些步骤需要账号持有人在官方页面手动完成；不得在对话、前端、日志、SQLite 或仓库中提供 Secret/Token，也不得用 Mock 冒充。

## 已知产品/实现限制

1. Transfer HTTP 执行仍是同步请求；核心取消/恢复已测试，但前端没有持久 transfer session、状态/SSE、取消和恢复接口。
2. Transfer 可在最多 5 个已有候选中改选或跳过，尚无用户自定义查询的“重新搜索”专用 API。
3. 真实账号下 Spotify “拥有或协作”列表边界、YouTube 官方频道信号和 quota 行为尚待端到端核对。
4. `/compare` 无 Last.fm 时只有确定性比较，没有共同候选。
5. Alternate Versions 未做真实 YouTube OAuth 搜索验收。
6. `cargo check` 保留 5 个非阻塞未使用项警告：集中 ArtistIdentity 公共接口、候选 MBID 字段、SourceConnector 与恢复函数等待正式调用链接入。
7. 前端的 Compare、Transfer、Alternate 组件目前集中在 `App.tsx`；本轮遵守不做无关重构。

## Git 与秘密审计

- 未发现源码/文档中符合常见高风险 Key/私钥格式的内容。
- `.gitignore` 已覆盖 `.env`、私钥、token JSON、数据库、Rust/前端构建目录、日志和 `testdata/*.csv`。
- **仍需人工处理：** 四份 `testdata/*.csv` 在本轮开始前已经处于 Git tracked 状态。新增 ignore 规则不能追溯取消跟踪；本轮没有执行会修改 Git index 的 `git rm --cached`。提交前应先确认这些文件是否获准公开，若属于私人歌单，应在保留本地文件的前提下由维护者取消跟踪并检查历史。
- 没有创建提交、推送远程或公开部署。

## 主要变更文件

后端：`agent.rs`、`engine.rs`、`identity.rs`、`normalize.rs`、`recommendation.rs`、`alternate.rs`、`transfer.rs`、`models.rs`、`main.rs`、Spotify/YouTube writers。  
前端：`App.tsx`、`api.ts`、`types.ts`、`styles.css`。  
文档：`README.md`、`PLAN.md`、`AGENTS.md`、推荐验收、夜间进度、Agent/Compare/Alternate/Transfer/交接报告及本报告。

## 下一步（需要用户回来）

1. 先决定四份 tracked CSV 是否允许进入仓库；私人文件应取消跟踪并审计 Git 历史。
2. 若要完成推荐复验，只在安全环境变量中配置 Last.fm，重启唯一后端，再按 Japanese → Korean → English → Chinese 串行真实验收。
3. 若要完成 Transfer，按 `docs/transfer_mvp_report.md` 配置官方 OAuth，由用户手动登录两端，以 5—10 首歌单完成预览、歧义确认和真实私有 YouTube 列表创建。
4. 只有得到真实可访问的 YouTube 播放列表链接后，才把 Transfer 标记为 `REAL API VERIFIED`。
