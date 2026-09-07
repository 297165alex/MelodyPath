# 平台能力与真实实现边界

> 2026-09-07：以下为历史记录。`feature/more-platforms` 当前能力与官方核验以 [platform_capability_audit.md](platform_capability_audit.md) 为准；Apple 新增公开目录读取代码但未真人验收，不能将本页旧 IMPORT ONLY 表格当作最新矩阵。Spotify/YouTube 读取真人验收以 README 的 2026-09-07 记录为准，最终真实写入仍未通过。

> MelodyPath是《程序设计训练（Rust语言）》AI Agent课程项目，同时将阶段性成果用于智理杯智能体大赛。

以下是代码与本机验收状态，不用 Demo 代替平台验收。前端只消费后端 `PlatformCapability` 的布尔字段；没有 reader/writer 的平台不会出现可点击的 Connect 或目标按钮。

状态含义：

- **REAL OAUTH VERIFIED**：实际登录、读取/写入并得到真实平台链接。
- **IMPLEMENTED BUT UNCONFIGURED**：官方 OAuth/接口代码存在，但开发者配置或人工授权未完成。
- **IMPORT ONLY**：只接受用户上传的导出文件或粘贴清单。
- **UNSUPPORTED**：当前没有合法且已验证的实现。

| 平台 | 官方账号授权 | 个人歌单读取 | 创建/修改歌单 | 公开链接 | 当前状态 |
|---|---|---|---|---|---|
| Spotify | Authorization Code Flow 已实现 | 分页读取已实现 | 新建私有列表与批量加入已实现 | 仅识别，不冒充曲目导入 | **IMPLEMENTED BUT UNCONFIGURED** |
| YouTube / YouTube Music | Google OAuth Web Server Flow 已实现 | `mine=true` 与 playlistItems 分页已实现 | 新建私有列表与插入视频已实现 | 仅识别，不冒充曲目导入 | **IMPLEMENTED BUT UNCONFIGURED** |
| Apple Music | 当前连接器未实现，不显示 Connect | 连接器未实现 | 连接器未实现 | 无 | **IMPORT ONLY / CONNECTOR NOT IMPLEMENTED** |
| 网易云音乐 | 不显示 Connect | 未声称支持 | 未声称支持 | 只识别/检查；无稳定曲目读取 | **IMPORT ONLY** |
| QQ音乐 | 不显示 Connect | 未声称支持 | 未声称支持 | 只识别/检查；无稳定曲目读取 | **IMPORT ONLY** |
| 酷狗音乐 | 不显示 Connect | 未接通 | 未接通 | 未实现 | **PARTNERSHIP REQUIRED / IMPORT ONLY** |
| 酷我音乐 | 不显示 Connect | 未接通 | 未接通 | 未实现 | **IMPORT ONLY** |

`PlatformCapability` 明确返回通用读写字段，也按用途拆分 `playlist_read_for_copy`、`playlist_read_for_compare`、`playlist_read_for_recommendation`，并返回 `copy_source_supported`、`copy_destination_supported`、`alternate_version_search_supported`、`status` 与 `reason`。Spotify/YouTube 的账号 reader 仅用于用户主动发起的 Copy；Compare/Recommendation 均关闭。中国平台当前仅引导合法文本或文件导入。

## 数据使用能力

Apple Music 的当前实现与目标产品必须分开：当前仅导入；目标通过官方 MusicKit / Apple Music API 账号连接、读取 Library Playlists、创建新歌单和添加歌曲。手动 CSV 是备用入口，不是 Apple 用户的目标主要体验。目标连接器尚未实现，本轮没有新增平台。

QQ/网易云/酷狗优先考虑合法且已实现曲目读取的 Public Link，其次 Paste tracks，最后 File fallback。当前仅能识别链接或检查页面可访问性时，状态仍为 Import Only，不能宣称 Public Link 曲目导入已支持。

后端 `data_use` 按能力逐项返回，而不是一个总开关。Spotify 与 YouTube API 数据可用于账号身份、歌单列表、用户主动选择后的归属展示、歌单元数据传输、创建与加入；不可进入 MelodyPath 的口味画像、独立衍生指标、跨平台相似度、LLM 或模型训练。手工粘贴、文件和明确标注的 Demo 数据仍可运行完整 Rust 分析。

## 官方依据

- [Spotify Developer Policy](https://developer.spotify.com/policy)、[2026 Development Mode 迁移说明](https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide)
- [Google OAuth for Web Server Apps](https://developers.google.com/youtube/v3/guides/auth/server-side-web-apps)、[YouTube API Developer Policies](https://developers.google.com/youtube/terms/developer-policies)
- [MusicKit](https://developer.apple.com/musickit/)、[Apple Music 用户认证](https://developer.apple.com/documentation/applemusicapi/user-authentication-for-musickit)
- [网易云音乐开发者中心](https://developer.music.163.com/)
- [腾讯连连 QQ音乐 H5 SDK](https://cloud.tencent.com/document/product/1081/67456)、[酷狗开放平台](https://open.kugou.com/docs)
