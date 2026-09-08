# Platform capability audit

> 2026-09-08 最新网易云实现：公开页面列出的歌曲前缀经 HTML / JSON-LD 一致性校验后，通过有限官方歌曲详情读取生成真实预览，最多 20 首，显示实际导入数 / 页面声明总数并接入既有分析。没有可验证歌曲或全部详情不可用时仍为 ACCESSIBILITY_CHECK_ONLY。见 [当前实现与验收](netease_public_playlist.md)，下方保留历史口径。

官方能力审计日期：2026-09-07；最终产品收尾：2026-09-08（未开展新 API 研究）。范围：`feature/more-platforms` 上的平台读取扩展；不扩大 Spotify/Google scope，不改变已有写入确认闸门。本报告取代旧文档中 Apple “只有 bootstrap” 的当前实现描述；旧的真人验收记录保留其原始日期与范围。

## 官方能力与实现矩阵

“未核实”表示本次官方公开文档不足以确认第三方普通 Web 应用可用，**不表示断言平台不存在任何内部或合作 API**。页面可访问、URL 格式正确和自动测试均不等于真人 API 验收。

| Platform | Official API | Official OAuth | Public playlist metadata | Public playlist tracks | User playlist read | Export/import path | Required developer credentials | Qualification/cost | Implemented capability | Reason for limitation | Real verification status |
|---|---|---|---|---|---|---|---|---|---|---|---|
| Spotify | Web API | Authorization Code | 已实现，需授权与访问权限 | 已实现分页 | 已实现 | OAuth / URL / 自备文件文本 | 部署者 Spotify 应用配置 | 既有平台开发模式及应用资格限制 | 账号读取、URL 曲目导入；既有真人读取验收通过 | 不保证任意公开歌单可读；最终写入仍未真人验收 | REAL VERIFIED（既有读取验收）；写入未验收 |
| YouTube / YouTube Music | YouTube Data API v3 | Google OAuth | 已实现 | 已实现分页 | 已实现 | OAuth / URL / 自备文件文本 | 部署者 Google OAuth 配置、启用 API | 配额与 OAuth 发布/审核要求 | 账号读取、URL 曲目导入；既有真人读取验收通过 | YouTube Music 通过 YouTube API；最终写入仍未真人验收 | REAL VERIFIED（既有读取验收）；写入未验收 |
| Apple Music | Apple Music API / MusicKit [A1–A5] | MusicKit 用户授权，非本项目 OAuth 连接器 | Catalog API 可读取 | Catalog tracks 关系分页 | 官方可用，需 Music User Token；本轮未实现 | 本轮目录 URL；Mac 官方文本导出/复制列；自备 CSV/TSV/JSON/TXT | 部署者后端 `APPLE_MUSIC_DEVELOPER_TOKEN`；生成时需 Team ID、Key ID、MusicKit 私钥 | Developer Program 通常 USD 99/年；符合条件机构可能申请减免 [A6] | 公开 URL → 官方分页代码 → 既有 Import Preview / Track / TransferTrack；CONFIG_REQUIRED，未真人验收 | 无本轮真实凭据验收；不能将目录读取当作私人资料库读取 | OFFICIAL API IMPLEMENTED / CONFIG REQUIRED / NOT REAL VERIFIED（课程接受的最终状态） |
| 网易云音乐 | 官方开发者入口存在，页面本次读取失败 [C1] | 未核实通用个人歌单授权 | 未核实 | 未核实 | 未核实 | 自备文件、手工文本；未确认统一官方导出格式 | 未获可验证的适用凭据规范 | 申请资格/成本未核实；不能臆测个人可免费接入 | FILE_IMPORT_AVAILABLE + ACCESSIBILITY_CHECK_ONLY | 不依赖逆向库或站内接口；访问检查不提取曲目 | 未做真人 API 验收；不声称已接入 |
| QQ音乐 | 官方 IoT/H5 音乐服务接口 [C2] | 所见文档属于腾讯连连场景；非通用个人歌单 OAuth 证明 | 受限 SDK 中有歌单能力 | 受限 SDK 中有音乐能力；本项目未获适用授权 | 不可据此认定个人 Web 可用 | 自备文件、手工文本；未确认统一官方导出格式 | 合作场景凭据，不能用 QQ 登录替代 QQ音乐歌单权限 | IoT/合作用途限制，具体合同/成本未核实 | FILE_IMPORT_AVAILABLE + ACCESSIBILITY_CHECK_ONLY | 未验证可合法用于本课程 Web 应用的个人歌单 API | 未做真人 API 验收；不声称已接入 |
| 酷狗音乐 | 官方曲库开放播放 SDK [C3] | 未核实通用个人歌单 OAuth | 未核实适用接口 | 未核实适用接口 | 未核实 | 自备文件、手工文本；未确认统一官方导出格式 | SDK/商务接入资格待官方确认 | 合作资格/合同成本未核实 | FILE_IMPORT_AVAILABLE + URL_RECOGNITION_ONLY | 曲库播放 SDK 不等于个人歌单读取；分享 token/ID 格式未核实，不猜测解码 | 未做真人 API 验收；不声称已接入 |
| 汽水音乐 | 官方产品页可访问；未查到可验证的公开歌单开发文档 [C4] | 未核实；抖音 OAuth 不代表汽水歌单权限 | 未核实 | 未核实 | 未核实 | 自备文件、手工文本；未确认统一官方导出格式 | 未核实 | 未核实；不声称一定有付费 API | FILE_IMPORT_AVAILABLE + URL_RECOGNITION_ONLY | 仅识别已知 qishui.douyin.com 域名；不猜测分享 token/playlist ID | 未做真人 API 验收；不声称已接入 |

## 官方依据和检索局限

- A1 [MusicKit](https://developer.apple.com/musickit/)：官方服务、目录 API 与用户授权能力。
- A2 [Get a Catalog Playlist](https://developer.apple.com/documentation/applemusicapi/get-a-catalog-playlist)：`GET /v1/catalog/{storefront}/playlists/{id}`。地区来自用户 URL，不猜测默认地区。
- A3 [Playlist relationship](https://developer.apple.com/documentation/applemusicapi/fetch-a-relationship-on-this-resource-by-name-707nb)、[Fetching Resources by Page](https://developer.apple.com/documentation/applemusicapi/fetching-resources-by-page)：读取 tracks 关系并跟随 `next`。API 不返回的下架条目无法被本应用恢复。
- A4 [Songs attributes](https://developer.apple.com/documentation/applemusicapi/songs/attributes-data.dictionary)：标题、artistName、durationInMillis、分享 URL、ISRC；playParams 存在表示可供订阅者播放，缺失不能直接判定全球下架。
- A5 [User Authentication for MusicKit](https://developer.apple.com/documentation/applemusicapi/user-authentication-for-musickit)：用户资料库需 Music User Token。[Generating Developer Tokens](https://developer.apple.com/documentation/applemusicapi/generating-developer-tokens)：Developer Token 用于 API 身份验证。
- A6 [Apple Developer Program enrollment](https://developer.apple.com/programs/enroll/)：通常每年 USD 99；最终按地区与资格核验。课程项目可使用既有合资格部署者配置，无需让每个普通用户购买开发者会员；本轮不付款、不注册、不申请减免。
- A7 [Save a copy of a playlist in Music on Mac](https://support.apple.com/guide/music/save-a-copy-of-your-playlists-mus27cd5060f/mac)：官方支持文本导出、复制歌曲信息，也支持 XML。**本项目未实现 XML**，请选择文本或复制列。
- C1 [网易云音乐开发者中心](https://developer.music.163.com/)：本次打开失败，搜索未得到足够可验证的官方接口规范；历史“需要申请”描述不是本次已验证申请成功。
- C2 [腾讯云音乐服务](https://cloud.tencent.com/document/product/1081/67456)：官方搜索索引可读取，直接打开失败；文档明确为腾讯连连自定义 H5 SDK，不能推广为通用 Web 歌单 OAuth。
- C3 [酷狗曲库开放计划](https://open.kugou.com/docs/open-player/)、[SDK 合规说明](https://open.kugou.com/docs/mini-player/player_sdk_compliance.html)：曲库播放控件与 SDK 场景。根文档本次返回空文本。
- C4 [汽水音乐官方产品页](https://www.douyin.com/qishui/)：产品介绍不是歌单 API 文档。检索抖音官方开发者站未获得汽水歌单授权规范。

本次生产代码未使用博客、第三方逆向 GitHub 项目、签名绕过或抓包资料。

## Capability 与预览契约

沿用 `PlatformCapability` 的布尔能力字段和既有 `PublicLinkCapability`，只扩展一个 `CONFIG_REQUIRED` 状态，没有另造平行枚举。

- 平台卡片的 `status` 表示当前接入状态；`public_playlist_links` 表示 URL 能力；`auth_supported` / `playlist_read_supported` 表示账号连接/账号 reader，而非目录 URL reader；`public_link_import_supported` 表示 URL reader 已实现，不代表当前凭据有效或真人通过。
- 中国平台 `status=FILE_IMPORT_AVAILABLE`，URL 能力独立展示；不能因文件可用升级为官方 API 可用。
- Apple 缺配置为 `CONFIG_REQUIRED` / `WAITING_FOR_APPLE_DEVELOPER_CREDENTIALS`。配置只做格式和过期检查，Apple 仍可能拒绝其签名、origin 或权限；真正成功的 URL 请求才返回 `TRACK_IMPORT_AVAILABLE`。
- 继续复用 `/api/playlists/inspect-link`、首页 Import Preview、统一 Track、现有 `From<&Track> for TransferTrack` 和 ExportModal 确认流程。不新增 Apple 写入或私人资料库流程。
- 各平台统一返回 `import_rows`：source_platform、playlist_id、playlist_name、track_title、artist、duration_ms、source_url、availability、import_status。缺失信息保留 null，不生成 0 时长、虚假艺人或替代歌曲。缺元数据/缺艺人/非 songs 行明确跳过；识别或可访问性检查没有完整曲目时，整个 `import_rows` 保持为空。
- Apple 同一个 song ID 的重复条目保留在逐项报告，导入曲目只保留第一次有效记录；不同 ID 不按标题强行合并。playParams 缺失且元数据完整时保留曲目，availability=UNKNOWN，不假称可播放。
- 文件入口继续共用 Import Preview；重复行保持原样，交给已有重复统计。支持 UTF-8 与带 BOM 的 UTF-16 文本；带 Name/Artist 表头的 TAB 文本通过同一个表格解析器。Time 支持明确 `m:ss` / `h:mm:ss`，没有单位的数值不猜测。
- CSV/TSV/JSON/TXT 是用户整理格式，不声称中国平台原生导出；HTML/XML 未实现。原有 TXT 两种列顺序确认仍保留。

## 安全和人工验收

Apple Developer Token 仅运行时在后端读取、只发给固定 `api.music.apple.com`；关闭旧 bootstrap 下发。HTTP 不跟随重定向，分页 next 只接受同目录同歌单路径与数值 offset/limit。每页 4 MiB、最多 10,000 条/200 页、整体 90 秒；任何页失败或超限整次失败，不把半份歌单当完整成功。错误仅输出本地安全文案，不输出响应体、Token 或原始请求错误。

Spotify/Google OAuth 文件、scope、state 与源歌单写入边界不改动；平台 API 数据不进入画像/推荐/LLM。未检查真实密钥、Cookie、数据库、私人歌单或运行日志。

人工步骤：如已有 Apple 资格，由部署者在仓库外生成 Token 并通过安全后端环境配置，重启；普通用户粘贴本人愿意验证的**公开** Apple 歌单 URL，核对真实名称、完整条数、中文/日文/韩文、重复和缺失条目。地区差异、公开用户歌单与大型分页仍须真实验证。不要把凭据粘贴到聊天或 UI。本课程接受 OFFICIAL API IMPLEMENTED / CONFIG REQUIRED / NOT REAL VERIFIED，私人资料库未支持；不购买开发者会员、不等待凭据。本节人工步骤仅为未来自行部署的可选参考，不是课程收尾条件。没有凭据也可完成全部本地测试和文件流程。

2026-09-07 审计时执行进程通过 `Test-Path Env:APPLE_MUSIC_DEVELOPER_TOKEN` 仅检查变量存在性，结果 false；未读取凭据值。部署者配置安全环境后，新开终端在项目根目录运行 `cargo run -p melody-path-api`；另一个终端运行 `npm --prefix frontend run dev`，打开其输出的前端地址。无需在 MelodyPath UI 输入任何凭据。

测试 URL 应由用户在 Apple Music 官方页面复制，例如结构 `https://music.apple.com/cn/playlist/<名称>/pl.<真实ID>`；这只是格式说明，不是已验收的实际歌单。选本人愿意公开测试、包含少量明确曲目的歌单，随后再核对多页歌单。成功显示真实 playlist 名称、曲目、总数、逐项状态及 `TRACK_IMPORT_AVAILABLE`；没有点击确认前不创建目标列表。

## 平台扩展分支 UI 收尾记录（2026-09-08，01f143c）

能力矩阵冻结，不新增平台、API、OAuth 或 Apple 功能。README 已同步。Spotify/YouTube 官方读取与 Last.fm 推荐保留既有 REAL VERIFIED 记录；Apple 固定接受 OFFICIAL API IMPLEMENTED / CONFIG REQUIRED / NOT REAL VERIFIED，私人资料库未支持，不以付费或真人配置阻塞课程完成。

七张卡片保留原有结构、badge 样式与响应式布局，只统一中文文案：已真人验收、尚未真人验收、需部署者配置、连接账号后可用、支持文件 / 文本导入、可识别公开链接、可识别链接并检查公开可访问性。内部枚举仍用于逻辑判断，不作为主要用户文案；Apple 不显示等待凭据代码或普通用户配置按钮。

公网 Demo：普通用户无需开发者凭据。自行部署：真实 Spotify / YouTube / Apple Music / Last.fm 能力由部署者配置。统一提示“MelodyPath 不要求用户提供账号密码或 Cookie。”中国平台仅文件/文本和已实现的 URL 能力，不声称完整曲目 API。

本轮只修改前端文案与对应回归断言、README、此审计及 PLATFORM_HANDOFF.md；后端与 CSS 不变。最终测试结果见 PLATFORM_HANDOFF.md。全部检查通过即可由用户手动决定合并，不等待 Apple Credentials。

## 中国平台专项最终结论（feature/china-platform-links）

本节为该分支的最新状态，沿用已完成检查，不重复研究。没有平台升级为 PUBLIC_PLAYLIST_IMPORT_AVAILABLE，也没有新增 OAuth。完整逐平台证据、checkpoint 和测试记录见 [CHINA_PLATFORM_HANDOFF.md](../CHINA_PLATFORM_HANDOFF.md)。

| 平台 | 官方登录/API | 匿名公开页面证据 | 最终能力 |
|---|---|---|---|
| 网易云 | 未取得本项目可用的通用 Web 歌单授权规范 | [公开样本](https://music.163.com/playlist?id=7299150850) 声明 15 首，JSON-LD 实际 10 项且缺艺人/时长；不解码其他编码数据 | ACCESSIBILITY_CHECK_ONLY + FILE_IMPORT_AVAILABLE |
| QQ音乐 | 音乐人开放入口/既有 IoT SDK 不能当通用歌单 OAuth | [官方首页歌单](https://y.qq.com/n/ryqq_v2/playlist/7520364922) 为页面框架，本机匿名响应无完整曲目；HTTP 200 不等于成功导入 | ACCESSIBILITY_CHECK_ONLY + FILE_IMPORT_AVAILABLE |
| 酷狗 | 曲库 SDK 不是已获准的个人歌单 API | [官方首页歌单](https://www.kugou.com/songlist/gcid_3zq25x5kz17z038/) 返回通用框架，无足够完整歌曲数据 | URL_RECOGNITION_ONLY + FILE_IMPORT_AVAILABLE |
| 汽水 | [官方 OpenAPI 列表](https://developer.open-douyin.com/docs/resource/zh-CN/dop/develop/openapi/list) 有推荐能力，没有取得可用歌单读取规范 | [产品入口](https://qishui.douyin.com/) 无歌单结构数据或可核验分享链接；本轮未取得真实短链样本 | URL_RECOGNITION_ONLY + FILE_IMPORT_AVAILABLE |

### 网易云移动公开链接复核（2026-09-08）

用户提供的两条 `https://y.music.163.com/m/playlist?id=...` 官方移动链接均可匿名访问，并重定向到同 ID 的 `https://music.163.com/playlist?id=...`。实现新增精确 `y.music.163.com/m/playlist` 路由识别，提取纯数字 playlist ID，并规范化到主站 URL；`userid`、`creatorId` 等无关查询参数不会进入 canonical URL。

匿名页面结构仍不足以完整导入：两份页面的 `MusicPlaylist` JSON-LD 都只有 10 个实际条目，而页面分别声明 1196 与 106 首；结构化条目没有艺人和时长。没有解码其他页面数据，也没有调用站内接口。因此 `playlist_name`、`track_title`、`artist`、`duration_ms` 等缺失值不会被猜测，`import_rows` 与 `preview_tracks` 保持为空，能力继续为 **ACCESSIBILITY_CHECK_ONLY + FILE_IMPORT_AVAILABLE**。

四平台的企业/商务资格、具体费用及普通开发者歌单 credentials 均缺少适用于本项目的可验证规范，不能写成“绝不存在”或“免费可申请”。样本失败不代表所有页面永远不可读；没有完整性证据就不新增 parser，不以部分曲目冒充成功。

实现成果为网易云/QQ URL 校验修复：精确路径、精确 HTTPS 主机、同平台重定向与最多 4 次跟随；canonical 请求修正 fragment 路由并丢弃无关查询。新增 3 个离线测试覆盖路径混淆、官方路径变体和重定向边界。此处仍是可访问性检查，不是完整页面 reader；现有响应前缀和超时限制不作为完整性保证。酷狗/汽水仍仅识别，不新增网络抓取。

README 最小修正为 Spotify/YouTube 官方连接与公开 URL 优先，文件或 Takeout 为备用；真人读取验收、Apple CONFIG REQUIRED / NOT REAL VERIFIED、Last.fm 状态均保持。未使用 Cookie、私有接口、逆向或虚构曲目；未修改既有 OAuth、推荐、Agent、Compare、Copy 和前端布局。

最终全量回归：Rust 140 passed / 0 failed / 1 ignored，fmt/release locked build 全通过；前端 lint/build 全通过，E2E 16/16 PASS。未放宽断言或 timeout。建议仅以 URL 校验与真实能力文档收尾范围由用户手动合并，不能描述为新增四平台曲目导入。
