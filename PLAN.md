# MelodyPath 实施计划

> MelodyPath是《程序设计训练（Rust语言）》AI Agent课程项目，同时将阶段性成果用于智理杯智能体大赛。

## 结构

- `backend/`：Axum API、确定性分析/推荐/对比逻辑、平台读取与歌单写回适配器。
- `frontend/`：React + TypeScript 单页演示站，覆盖输入、个人报告、推荐、双人桥梁和导出流程。
- `docs/`：比赛介绍、演示脚本、架构、限制和 Spotify 验收说明。

## 阶段

1. P0（已完成）：离线 Demo、手动输入、分析、三段推荐、探索路线、双人比较和桥梁歌单。
2. P0 写回（已完成）：文件导出与明确标注的 Demo 写入；统一预览、选择、歧义确认、执行和结果报告。
3. 课程闭环（代码与本地验证完成）：Rust Agent Loop、SSE 进度、中断、SQLite 历史、模型参数、Token/费用统计和两项音乐定制场景；真实 LLM 决策与非零费用验收受配置阻塞。
4. 平台优先入口（已完成）：后端能力 API、首页平台卡片、公开链接识别/可访问性检查、直接粘贴与文件备用入口。
5. Spotify P1（代码完成，待真实账号验收）：Authorization Code Flow、昵称、个人歌单选择、Track 转换、刷新、解除连接、匹配、新建歌单和批量添加。
6. 中国平台 P1（资格受限）：网易云与 QQ 先提供公开链接识别；获得可验证官方曲目权限后才增加真实读取。
7. YouTube P1（代码完成，待真实账号验收）：Google OAuth、分页读取、统一 Track、新建播放列表与插入视频。
8. Apple 边界（收口）：当前为 `IMPORT_ONLY / CONNECTOR NOT IMPLEMENTED`；未来目标为官方 MusicKit / Apple Music API 账号连接、歌单读取、新建/写入，CSV 仅备用。本轮不扩展连接器。
9. 中国平台 P3/P4：公开链接合法检查；获得正式资格后再实现账号连接与歌单读写。
10. 推荐可靠性（完成）：Last.fm 候选、统一 Genre/版本/艺人身份、Controlled Serendipity、三区候选池/换批、逐阶段统计和 Demo 隔离；五份真实文件已串行浏览器回归。
11. 社交与版本探索（本地浏览器通过）：独立 `/compare`、Alternate Versions 及共用加入歌单确认流程。
12. Copy MVP（代码与 Mock/浏览器验证完成，真实 OAuth 阻塞）：独立 `/transfer`、最多 5 个候选、确定性评分、歧义确认、私有目标、会话状态/SSE/取消/恢复、逐首结果与 CSV/JSON 报告。
13. 持续验证：Rust 格式/检查/测试、前端 lint/build、HTTP 与浏览器回归、真实凭据手动验收。
14. 平台无关 Metadata Layer（本轮完成）：MusicBrainz 优先解析、确定性匹配状态与原输入降级；标准 Track 可经独立 Spotify export service 使用既有 OAuth 搜索并在确认后新建私有歌单。
15. 网易云公开页面条件导入（2026-09-08）：复用 URL 校验，对 HTML 容器与 JSON-LD 一致的公开歌曲前缀有限查询官方歌曲页，最多 20 首；显示实际导入数 / 页面声明总数，接入既有预览、确认、Resolver 与推荐。没有可验证歌曲时保留 ACCESSIBILITY_CHECK_ONLY。
16. 产品体验收口（2026-09-09）：文件、文本、Spotify、YouTube、中国平台与 Friend Bridge 复用统一 Task Progress；文本解析增加 Unicode 规范化及歌手/歌名顺序推断；Friend Bridge 加入 `zh/en/ja/ko` 语言兼容度；QQ/酷狗仅条件读取公开 JSON-LD；Version Radar 增加 MusicBrainz 发行动态并补齐 loading/error/empty 状态。
17. 智能化增强（2026-09-09）：文本规则层支持 `by`、书名号/日韩引号、编号、斜杠、冒号和高置信度无分隔符；仅 `REAL_TEXT` 低置信度输入可调用受限 LLM 结构化 fallback，结果必须逐行落地到原文且仍由 MetadataResolver 验证。新增 resolved-only Taste Profile 与并列排名；Friend Bridge 对外补充 `score`；QQ 公开 URL 与 JSON-LD 安全契约补齐。

## 风险与边界

- Spotify 真写入依赖用户自行创建应用并提供环境变量；无凭据时不影响 P0。
- Spotify API 内容受 Developer Policy 限制，只用于用户主动发起的传输/写回，不进入画像、衍生指标或 LLM。
- Intelligent Text Parser 不处理 `REAL_ACCOUNT` 或平台 API 内容；未配置模型、模型失败、低置信度、行数不一致或输出含原文不存在的字段时均返回 `Need confirmation`。
- Apple Music 当前明确为 Import Only；QQ音乐和酷狗只在官方公开页面存在可核验 `MusicPlaylist` JSON-LD 时条件导入，汽水仅做能力检测。任何平台都不模拟登录或使用私有接口，也不会在公开曲目缺失时补造歌曲。
- 公开页面可访问不等于曲目完整可读；网易云、QQ 和酷狗只为页面明确公开且可核验的歌曲生成预览，不补造未公开成员。QQ/酷狗的 JSON-LD 分支只有本地契约测试，尚未完成真实公网样本验收。
- 本地 OAuth token 通过 Windows DPAPI 用户范围保护；后端仅向浏览器发放不透明 HttpOnly session。生产环境必须为 `OAuthTokenStore` 注入托管密钥/KMS 实现。
- 离线推荐和元数据是明确标注的 Demo 数据，不宣称为实时平台结果。
- Transfer Mock 只验证本地流程；只有真实登录 Spotify 与 Google、确认预览并得到可访问的 YouTube 播放列表链接，才能标记真实迁移通过。
- Copy 执行已会话化并提供状态/SSE/取消/恢复 API；当前会话登记仍是后端进程内存状态，进程重启后必须重新生成预览，不宣称跨重启恢复。
