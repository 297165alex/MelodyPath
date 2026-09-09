# Spotify Confirm Import 调用链修复

日期：2026-09-09

## 根因

SpotifyPlaylistPicker 的“确认选择”仅调用 POST /api/spotify/import 并保存 SpotifyImportResult。它展示的是账号读取结果，不是已登记到 imports registry 的 ImportPreview。下一步 Continue 实际执行 onExport('spotify', ...) 并 onClose()，走写入预览而非分析；没有 import_id，也没有 POST analyze。底层 MetadataResolver 和推荐服务根本没有被调用。

另有两个状态问题：App 的 activePersonal 没有接受 REAL_ACCOUNT；品味页使用 /，而 tabForPath('/') 返回 home，导致浏览器返回时结果页被识别为首页。

## 修复后的调用链

1. 选择 Spotify 歌单，沿用既有官方 connector 读取。
2. HTTP 编排层将服务端读取结果转换为 StoredImport，保存全部曲目到进程内 registry，在原响应上增加 import_preview.id。预览本身不触发分析。
3. 用户点击 Confirm Import，前端保留该 ID，POST /api/imports/{id}/analyze；不再调用 onExport，也不提前关闭弹窗。
4. MetadataService 使用既有 MetadataResolver 和推荐 Provider。Accept: application/x-ndjson 时发送 metadata、recommendation 和最终 result；原 JSON API 仍兼容。
5. 完整结果返回后保存 analysis_id 与 REAL_ACCOUNT 状态，进入 /analysis；推荐页继续使用同一结果，浏览器返回保留当前会话的分析视图。

Importing... 对应账号读取；Analyzing metadata... 和 Generating recommendation... 由实际后端阶段更新。没有伪造百分比，也没有用定时器假装后端完成。重复确认及运行中关闭被禁用。

缺少 ID、预览过期、HTTP 错误、网络失败或响应断流时，预览保留，错误显示在弹窗中，可重试或返回重选。缺少 Last.fm 配置仍可展示画像与明确的推荐服务未配置状态。

## 范围与数据来源

本轮按用户明确要求，将“主动选择 Spotify 歌单并确认”接入分析与相关推荐，取代这一入口过去的仅传输行为。其他入口（包括 Spotify 公开 URL 的 Copy 路径）不在本轮改动范围内。

Spotify、YouTube、Apple connector 和 NetEase adapter 均未修改。Spotify 官方来源和 REAL_ACCOUNT 保留，不将账号输入伪装成文件或 Demo。既有 Agent 基于 Spotify source 的禁止 LLM 判定保持有效。connector 的既有 data_use / policy_notice 字段未重写；它们是旧的传输能力合同，本轮新增的是 HTTP 编排层的分析预览。此实现与流程测试不构成平台政策或新的外部平台验收声明。

预览和分析仍为进程内数据；服务重启需重新生成预览，页面刷新不会恢复 React 中的分析结果。不保存私人歌单到文件，不读取或记录用户凭据。

## 验证

- cargo test --workspace：169 passed，0 failed，4 ignored（既有显式联网测试）。
- frontend lint：PASS。
- frontend build：PASS。
- e2e：38/38 PASS（明确的合成 HTTP / Provider 测试，不代表真实 OAuth）。

Rust 新增测试覆盖：Spotify 结果登记与确认闸门、真实编排层 POST analyze、注入 MetadataResolver → 推荐 Provider、NDJSON 阶段顺序、JSON 兼容、analysis_id 保存、REAL_ACCOUNT / Spotify 来源、完整曲目保存、空歌单拒绝、过期 ID 错误。

浏览器新增测试覆盖：select → preview → confirm → analysis → discover → back；三阶段 UI；重复点击禁用；缺少 ID、404、500、网络失败、截断响应及重试。

上述测试只使用明确的合成 fixture / 注入 Provider，不读取私人歌单。本轮未重新执行真实 OAuth + 外部 Metadata / Last.fm 推荐验收，不把测试替身写成真实平台成功。
