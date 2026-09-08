# MelodyPath 代码答辩导读

本页只解释已有实现，不要求为了答辩重构后端。建议按“模型 → 能力 → 导入 → Agent 页面”打开代码。

## Backend

| 文件 / 入口 | 作用 | 答辩时指出的设计 |
|---|---|---|
| `backend/src/models.rs` | serde 请求、响应与领域类型 | `DataState` 区分真实文件/文本与 Demo；`ImportedTrack` 保留解析信息，`Track` 统一歌曲表示；`PlatformCapability`、`PlaylistLinkInspection` 区分平台权限和单次结果；`AgentDecision`、`AgentTask` 描述决策与运行状态 |
| `backend/src/platforms.rs` | `PlatformService::capabilities` 返回能力；`inspect_link` 识别和检查公开链接 | URL 白名单与重定向边界；网易云/QQ 只检查可访问性，酷狗/汽水只识别；`data_use` 独立控制用途 |
| `backend/src/import.rs` | `parse_import` 分派 CSV/TSV、JSON、TXT、M3U/M3U8 解析；`StoredImport::preview` 生成摘要 | 空曲目报错，解析警告和列顺序确认可见；预览与分析分开，真实失败不回退 Demo |
| `backend/src/main.rs` | Axum 路由与会话编排 | `preview_import` 登记预览；`analyze_import` 按 ID 取用户确认的输入；`inspect_playlist_link` 按能力调用官方 reader 或返回限制 |
| `backend/src/metadata.rs`、`engine.rs` | 元数据补全与确定性分析 | 缺失信息保留，统一曲目进入画像和统计，结果登记为 Analysis ID |
| `backend/src/writers/spotify.rs`、`youtube.rs`、`apple.rs` | 各自官方 connector | Spotify/Google 官方 OAuth 与分页；Apple 仅已实现公开目录 reader，需配置，未真人验收 |
| `backend/src/agent.rs` | 决策、白名单工具、执行结果反馈、检查点 | 模型只选择允许工具；Rust 校验并执行；模型不可用时明确 fallback |

导入调用链：`POST /api/imports/preview` → `parse_import` → 暂存 `StoredImport` → 返回 `ImportPreview` → 用户确认 → `POST /api/imports/{id}/analyze` → `metadata.analyze_imported` → Analysis。服务重启后旧预览 ID 失效，需重新解析。

URL 调用链：`POST /api/playlists/inspect-link` → `PlatformService::inspect_link` → 官方平台按需检查配置/授权并分页读取 → 有实际 tracks 才给预览。URL recognition 与 accessibility check 本身不产生歌曲。账号歌单通过对应官方 reader 生成统一 Track；平台 API 数据受用途限制，走确认后的传输，不接入本地分析或 LLM。

## Frontend：Import 页面

首页由 `frontend/src/App.tsx` 的 `Home` 承载。`prepareImport` 调用解析预览，`loadFile` 通过 `frontend/src/importFile.ts` 解码文件，`ImportPreviewPanel` 显示行数、警告与列顺序，`confirmImport` 才触发分析。

`inspectLink` 调用 `frontend/src/api.ts` 中的链接 API；`LinkImportStatus` 把响应拆成 URL Recognition、Accessibility Check、Track Import 三段。只有 `capability=TRACK_IMPORT_AVAILABLE` 且 `preview_tracks` 非空，才展示链接 Import Preview。中国平台结果旁提供文件/文本下一步。

`SpotifyPlaylistPicker` 与 `YouTubePlaylistPicker` 负责官方账号歌单选择。`confirmLink` 复用 `ExportModal.tsx` 的匹配预览和确认写入交互；看见源歌曲预览不代表已创建目标歌单。

## Frontend：Agent 页面

`App.tsx` 的 `AgentPage` 绑定当前 Analysis 或 Compare 的两个 Analysis ID。`start` 创建任务，`EventSource` 订阅 `/api/tasks/{id}/events`，`cancel` / `resume` 调用对应 API。界面从任务 JSON 读取 plan、decisions、tool calls、tool results，并展示进度、状态、Token 和费用。

五步导览在任务开始前也可见；运行轨迹按请求、决策、调用、结果、最终说明分组，最近 8 条记录用于展示。完成后末段通过 call_id 关联 prepare_explanation 的成功结果，展示 Rust 分析摘要和推荐说明；不生成虚构解释。完整任务记录可在 History 核验。未完成时末段标为当前状态，不能把失败或中断说成完成。`Data state` 与 `Decision mode` 分别回答“用的是什么数据”和“由谁选择下一步”。

答辩可说：“这里的解释有工具结果依据；确定性 fallback 是实际运行模式，不能把它说成真实 LLM 调用。平台真实读取的既有验收，也不能替代真实写入验收。”

## 建议现场讲解顺序

用仓库示例文件完成预览和分析，进入 Agent，先指出 User Request 与数据绑定，再选一条 Agent Decision、对应工具名和 Tool Result 摘要，最后展示结束状态、模式与用量。需要说明架构或平台差异时，配合 [架构答辩说明](architecture_explanation.md) 与 [五分钟演示脚本](demo_script.md)。不要打开凭据、数据库或私人歌单文件作为代码演示。
