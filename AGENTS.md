# MelodyPath 后续 AI 开发约束

## 开始工作前

1. 修改任何代码前，先完整阅读 `README.md`、`PLAN.md` 和 `docs/` 中与当前任务相关的文档。
2. 尊重已有代码和真实验收记录；不要从头重建项目，不要撤销已完成的 Last.fm 推荐、本地分析、历史、Agent Loop、好友桥梁或导出模块。
3. 不得读取、输出、记录或提交 API Key、Client Secret、OAuth Token、Cookie、`.env`、数据库或私人歌单内容。

## 当前最高优先级

当前最高优先级是完成独立 `/transfer` 流程的真实 OAuth 验收与必要收尾。页面、后端模型、确定性匹配、显式 Mock、确认闸门和报告已实现；不得把现有 Mock 验证误写为真实平台成功。第一版只支持：

> Spotify 用户拥有或参与协作的一个歌单  
> → YouTube 候选搜索与确定性匹配  
> → 用户确认歧义结果  
> → 创建新的 YouTube 私有播放列表

不要在第一版同时加入 Apple Music、网易云、QQ音乐、酷狗或酷我。不要为了 Transfer 重构、删除或弱化现有推荐功能。

## 当前实现位置

- `backend/src/transfer.rs`：Transfer 领域模型、确定性评分、预览、执行、取消与恢复核心逻辑。
- `backend/src/writers/spotify.rs`：官方 Spotify OAuth、只读歌单/曲目分页及既有 writer。
- `backend/src/writers/youtube.rs`：官方 Google OAuth、YouTube 搜索、视频时长、新建私有列表和逐首写入。
- `frontend/src/App.tsx`：当前包含独立 `/transfer`、`/compare` 与 Alternate Versions 用户流程组件。
- `frontend/src/ExportModal.tsx`：可复用的选择、匹配预览、歧义确认和执行交互。
- `docs/transfer_mvp_report.md`：已验证范围、真实 OAuth 阻塞与人工步骤。

## 数据与匹配边界

- Spotify 数据不得发送给 LLM，不得用于模型训练、用户画像或与迁移无关的推荐。
- 曲目匹配必须使用可解释、可复现的确定性评分，至少考虑标题、艺术家、时长、官方频道/Topic 标识和 Live、Cover、Remix 等版本词。
- YouTube 每首歌曲的歧义候选必须在写入前让用户确认、改选、重新搜索或跳过。
- 用户明确确认前不得创建或写入目标播放列表。
- 永远不得修改或删除源 Spotify 歌单；目标端默认新建私有 YouTube 播放列表。
- 单首写入失败不能使整个迁移任务失败；流程必须提供进度、可中断状态和逐首结果。
- Copy 执行已使用独立 run id，并提供状态查询、SSE、取消和恢复 API；run 登记当前仍是进程内状态，服务重启后必须重新生成预览。不得把“单进程可恢复”写成跨重启持久化。

## OAuth 与真实验收

- 只使用 Spotify 和 Google/YouTube 官方 OAuth，不接收账号密码、Cookie，不模拟登录，不使用私有接口。
- 缺少 Client 配置、Redirect URI、YouTube Data API、额度或用户手动授权时，必须立即停止并准确报告缺少项。
- 不得用 Mock、离线固定数据或已有 Demo 冒充真实平台连接。
- 只有真实登录两个平台并成功获得新 YouTube 播放列表链接，才能宣称真实迁移验收通过。

## 当前验收状态（2026-09-05）

- Last.fm：五份真实文件串行浏览器回归通过，全部 `is_demo=false`，三区与换批均使用真实候选。
- Agent：真实文件和真实文本绑定已在浏览器验证；当前环境没有 `OPENAI_API_KEY`，所以真实运行明确标记 `DETERMINISTIC_FALLBACK`，不得写成真实 LLM 调用已验收。
- Spotify / YouTube：代码、Mock 和能力矩阵已通过；真实 OAuth 仍需用户在官方页面授权，状态为 `MANUAL_AUTH_REQUIRED` / `BLOCKED_BY_CONFIGURATION`。
- Apple Music：仅 `IMPORT_ONLY`，不得把 MusicKit 的理论能力写成当前项目已实现。

## 交接依据

实现顺序、可复用代码和人工配置边界见 `docs/teammate_handoff.md`。推荐模块当前限制见 `docs/recommendation_acceptance_report.md`。
