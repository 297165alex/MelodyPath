# MelodyPath 后续 AI 开发约束

## 开始工作前

1. 修改任何代码前，先完整阅读 `README.md`、`PLAN.md` 和 `docs/` 中与当前任务相关的文档。
2. 尊重已有代码和真实验收记录；不要从头重建项目，不要撤销已完成的 Last.fm 推荐、本地分析、历史、Agent Loop、好友桥梁或导出模块。
3. 不得读取、输出、记录或提交 API Key、Client Secret、OAuth Token、Cookie、`.env`、数据库或私人歌单内容。

## 当前最高优先级

当前最高优先级是实现独立的 `/transfer` 页面及其后端流程。第一版只支持：

> Spotify 用户拥有或参与协作的一个歌单  
> → YouTube 候选搜索与确定性匹配  
> → 用户确认歧义结果  
> → 创建新的 YouTube 私有播放列表

不要在第一版同时加入 Apple Music、网易云、QQ音乐、酷狗或酷我。不要为了 Transfer 重构、删除或弱化现有推荐功能。

## 数据与匹配边界

- Spotify 数据不得发送给 LLM，不得用于模型训练、用户画像或与迁移无关的推荐。
- 曲目匹配必须使用可解释、可复现的确定性评分，至少考虑标题、艺术家、时长、官方频道/Topic 标识和 Live、Cover、Remix 等版本词。
- YouTube 每首歌曲的歧义候选必须在写入前让用户确认、改选、重新搜索或跳过。
- 用户明确确认前不得创建或写入目标播放列表。
- 永远不得修改或删除源 Spotify 歌单；目标端默认新建私有 YouTube 播放列表。
- 单首写入失败不能使整个迁移任务失败；流程必须提供进度、可中断状态和逐首结果。

## OAuth 与真实验收

- 只使用 Spotify 和 Google/YouTube 官方 OAuth，不接收账号密码、Cookie，不模拟登录，不使用私有接口。
- 缺少 Client 配置、Redirect URI、YouTube Data API、额度或用户手动授权时，必须立即停止并准确报告缺少项。
- 不得用 Mock、离线固定数据或已有 Demo 冒充真实平台连接。
- 只有真实登录两个平台并成功获得新 YouTube 播放列表链接，才能宣称真实迁移验收通过。

## 交接依据

实现顺序、可复用代码和人工配置边界见 `docs/teammate_handoff.md`。推荐模块当前限制见 `docs/recommendation_acceptance_report.md`。
