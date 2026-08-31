# 平台能力与真实实现边界

> MelodyPath是《程序设计训练（Rust语言）》AI Agent课程项目，同时将阶段性成果用于智理杯智能体大赛。

以下是代码与本机验收状态，不用 Demo 代替平台验收。

| 平台 | 官方账号授权 | 个人歌单读取 | 创建/修改歌单 | 公开链接 | 当前状态 |
|---|---|---|---|---|---|
| Spotify | Authorization Code Flow 已实现 | 分页读取已实现 | 新建与批量加入已实现 | 仅识别 | 配置后可用；本机尚待用户真实授权验收 |
| YouTube / YouTube Music | Google OAuth Web Server Flow 已实现 | `mine=true` 与 playlistItems 分页已实现 | 新建私有列表与插入视频已实现 | 仅识别 | 配置后可用；本机尚待用户真实授权验收 |
| Apple Music | MusicKit Web 配置检测和 bootstrap 已实现 | 连接器预留 | 连接器预留 | 无 | 需要 Apple Developer 配置；尚未完成真实账号验收 |
| 网易云音乐 | 官方开放平台存在，但需申请应用资格；不是无条件通用网页 OAuth | 未声称支持 | 获正式资格后再实现 | 官方域名识别、合法重定向、ID 与公开页面结构检查 | 不使用密码、Cookie 或逆向私人接口；未稳定读取曲目 |
| QQ音乐 | 未找到面向普通网页开发者、可无条件申请的个人歌单 OAuth；官方能力多见于腾讯连连/合作场景 | 未声称支持 | 需正式资格 | 官方域名识别、合法重定向、ID 与公开页面结构检查 | 未稳定读取曲目 |
| 酷狗音乐 | 有官方开放平台，具体歌单能力取决于申请资格 | 未接通 | 未接通 | 预留 | 等待资格 |
| 酷我音乐 | 未验证普通网页个人歌单接口 | 未接通 | 未接通 | 预留 | 更多平台 |

## 数据使用能力

后端 `data_use` 按能力逐项返回，而不是一个总开关。Spotify 与 YouTube API 数据可用于账号身份、歌单列表、用户主动选择后的归属展示、歌单元数据传输、创建与加入；不可进入 MelodyPath 的口味画像、独立衍生指标、跨平台相似度、LLM 或模型训练。手工粘贴、文件和明确标注的 Demo 数据仍可运行完整 Rust 分析。

## 官方依据

- [Spotify Developer Policy](https://developer.spotify.com/policy)、[2026 Development Mode 迁移说明](https://developer.spotify.com/documentation/web-api/tutorials/february-2026-migration-guide)
- [Google OAuth for Web Server Apps](https://developers.google.com/youtube/v3/guides/auth/server-side-web-apps)、[YouTube API Developer Policies](https://developers.google.com/youtube/terms/developer-policies)
- [MusicKit on the Web](https://developer.apple.com/musickit/web/)、[Apple Music Developer Token](https://developer.apple.com/documentation/applemusicapi/generating-developer-tokens)
- [网易云音乐官方开放平台工具说明](https://github.com/NetEase/skills/blob/master/README.md)
- [腾讯连连 QQ音乐 H5 SDK](https://cloud.tencent.com/document/product/1081/67456)、[酷狗开放平台](https://open.kugou.com/docs)
