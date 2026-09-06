# MelodyPath 课程 R1–R6 审计

更新时间：2026-09-06

| 要求 | 状态 | 证据与边界 |
|---|---|---|
| R1 Rust Core | **PASS** | Rust 完成导入、规范化、身份、Genre、元数据置信度、种子、Last.fm Provider、推荐评分、去重、版本分类、Compare、Copy 匹配、权限与 Agent 状态。LLM 不计算音乐事实。 |
| R2 UI | **PASS** | 浏览器覆盖 Analyze、Discover、换批、Compare、Version Radar、Copy、Agent、History、Settings 和平台连接状态。 |
| R3 Configurable LLM | **PARTIAL** | 配置界面、结构化决策与失败降级代码/自动测试通过；当前服务无可用 LLM Key，真实外部决策环尚未验收，运行明确为 deterministic fallback。 |
| R4 Realtime progress + interrupt | **PASS** | Agent 使用 SSE、cancel、timeout、checkpoint/resume；Copy 使用 run id、状态/SSE/cancel/resume。本地单进程已验；Copy run 尚不跨进程重启。 |
| R5 History | **PASS** | SQLite 保存 Agent goal、intent、data_state、decision_mode、plan、tools、result、timestamp、status、warnings 和 checkpoint；不保存 OAuth Secret。 |
| R6 Token + Cost | **PARTIAL** | usage 记录、费用估算与上限测试通过；真实非零 LLM usage/cost 尚未验收。fallback 浏览器运行为 0 token / 0 cost。 |

## 自动验证

最终 Rust 格式/检查/测试通过：105/105。路由小修后前端 `npm run lint` 与 `npm run build` 再次通过。结构化 Decision、非法工具、ToolResult 改变下一工具、步骤/费用上限、LLM 失败 fallback、真实 Analysis 绑定、无 Demo 泄漏均有测试。

## 真实验证

- Last.fm：真实 Provider 与五份真实文件串行浏览器通过。
- Agent：真实文件个人探索与真实文本好友比较通过；Decision Mode 为诚实的 `DETERMINISTIC_FALLBACK`。
- 2026-09-06 追加复核：Happy_Mix 50 首、REAL_FILE、13 个工具全部成功；bound analysis 与最终结果一致，12 首推荐与页面集合一致，Token/Cost 为 0。六个直达页面通过，新控制台错误 0。
- Spotify/YouTube Copy：代码、Mock、状态和安全闸门通过；真实双端 OAuth 与目标链接仍是 `MANUAL_AUTH_REQUIRED`。
