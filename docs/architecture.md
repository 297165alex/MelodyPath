# 系统架构

> MelodyPath是《程序设计训练（Rust语言）》AI Agent课程项目，同时将阶段性成果用于智理杯智能体大赛。

## 分层

```text
React / TypeScript
  ├─ 输入、报告、推荐与好友比较
  ├─ Agent SSE 进度、取消、历史与设置
  └─ 写回选择 → 匹配预览 → 版本确认 → 执行 → 报告
                    │ REST / SSE
Axum API ───────────┼───────────────────────────────────────┐
  │                 │                                       │
Rust Agent Loop     Deterministic Music Engine              PlaylistWriter
  ├─ 工具选择        ├─ 解析/标准化/版本识别                  ├─ ExportFile（真实）
  ├─ 步数/超时       ├─ 画像与相似度                          ├─ Demo（明确虚拟）
  ├─ 约束复查        ├─ 推荐评分与排除                        ├─ Spotify（可配置真实）
  ├─ 取消            └─ Bridge Score 与顺序约束               ├─ 平台能力/公开链接检查
  └─ 历史                                                       └─ Apple/YouTube/中国平台预留
  └─ 用量/费用                │                                       │
           └──────────────── SQLite ─────────────────────────────────┘
                            任务历史 / 设置
```

## Agent Loop

个人探索场景依次识别输入、选择 Provider、读取并检查字段、标准化歌曲、构建画像、理解目标、生成候选、计算相关性与新颖性、检查去重和歌手集中度、构建解释路线并持久化。好友场景统一双方 Track，计算重合与兼容度，识别特色、寻找相邻风格连接、计算 Bridge Score，检查平滑与均衡后保存桥梁歌单。

循环由 `backend/src/agent.rs` 实现。最大步数、超时和费用边界来自持久化设置；取消使用线程安全原子标志；每步写入 SQLite revision，SSE 端点轮询新 revision 后推送。配置 `OPENAI_API_KEY` 时，循环在确定性结果后调用 OpenAI-compatible `/chat/completions` 增强解释，使用返回的 usage 统计 Token 并按配置单价估算费用；失败重试后仍保留 Rust 结果并明确降级。服务重启时遗留的 queued/running 任务会标记为 failed，不会假装继续运行。

## 数据与可信度

统一 `Track` 保留标题、标准标题、歌手、专辑、Genre、年代、语言、时长、来源、外部 ID、版本、情绪、能量、流行度和元数据置信度。手动文本只提供歌手/歌名，因此不会虚构 Genre；Demo 元数据在所有相关页面标注为离线模拟。

## PlaylistWriter

trait 包含 `authorize`、`search_track`、`create_playlist`、`add_tracks` 和 `get_playlist_url`。预览端点仅检索，不执行账号修改；执行端点要求一次性 `preview_id` 和 `confirmed=true`，因此旧预览不能重复写入。默认创建新歌单。

Spotify 匹配优先 ISRC；回退时组合歌名、歌手、专辑、时长和版本分数，识别 Live、Remix、Remastered、Acoustic、Cover 与 Instrumental。高置信度自动选，中置信度返回候选，低置信度失败。写入按 100 首分批并逐批记录失败。

`platforms.rs` 提供平台能力的服务端真相源，并对公开链接实施协议、官方域名和重定向白名单。Spotify 与 YouTube 返回细粒度 `data_use`：读取、归属展示、传输和写回可用，画像、衍生指标、跨平台比较、LLM 与训练不可用；官方数据路径只连接预览与 `PlaylistWriter`，不连接 Agent/LLM 分析路径。

## 安全边界

- 平台密码永不进入 MelodyPath；OAuth 登录发生在平台页面。
- OAuth state 一次性且十分钟过期。
- access/refresh token 只在后端内存，浏览器持有 HttpOnly、SameSite 会话 cookie。
- 模型 API Key 只读环境变量，不进入设置 API 或 SQLite。
- Demo 虚拟 URL 使用 `demo://`；未完成的平台适配器返回可理解错误。
