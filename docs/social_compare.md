# 两份歌单临时比较（`/compare`）

更新时间：2026-09-09

## 用户流程

`/compare` 默认让 Friend A 与 Friend B 分别选择 Local file，也允许两侧粘贴 CSV/TSV/JSON/TXT/M3U/M3U8 批量文本。两侧输入都必须先经过统一 Rust 导入预览；只有两侧都确认后才执行比较。默认 `saved_locally=false`，不会建立公开好友账号，也不会默认把另一人的歌单写入历史。

## 确定性指标

`backend/src/engine.rs` 计算：

- Track overlap：规范化曲目身份的 Jaccard 重合度。
- Artist overlap：集中艺人身份/别名后的重合度。
- Genre overlap：统一 Genre 后的重合度。
- Tag overlap：真实元数据标签重合度。
- Language compatibility：依据 `zh/en/ja/ko` 比较两侧语言结构，避免桥梁候选被单一语言占满。
- Diversity complementarity：两份歌单多样性结构的互补度。
- Overall compatibility：上述指标的确定性加权结果。

共同探索候选分成 `Safe for Both / Bridge / Adventure Together`。评分同时考虑 Genre、Artist、Mood 与 Language，并对单阶段同语言候选设上限。任何候选都必须同时排除 A 和 B 两份源歌单，并保留对 A、对 B、共同依据、Provider 与分数；理由明确说明它如何连接 A 与 B 的偏好。真实候选不足时列表保持为空，不使用固定桥梁曲目或 Demo 补齐。

桥梁响应保留既有 `bridge_score`，并增加同值的兼容字段 `score`，核心输出可按 `{ track, score, reason }` 消费。`reason` 必须同时提到用户 A 与用户 B 的偏好连接，不能只复述候选标签。

## 隐私与平台边界

- 文件和文本用于本次临时比较；保存好友数据需要独立、明确的产品实现和用户同意。
- Spotify 内容受平台政策约束，不应进入画像或 LLM；不可读取的链接应改为官方连接或用户主动上传的导出文件。
- 比较本身不需要 LLM。可选解释增强不能改变确定性分数。

## 验证结果

- Rust 测试覆盖相同歌单、完全不同、部分重合、不同允许来源和候选排除双方源歌单。
- 本地浏览器用两份各 3 首的真实批量文本完成解析预览和确认比较：结果为 `REAL_TEMPORARY_COMPARISON`、`is_demo=false`，0 共同曲目、0 共同艺人。
- 浏览器已用两份真实批量文本完成临时比较：A/B 各 3 首，`is_demo=false`，默认不保存，产生 7 首确定性桥梁候选。随后绑定同一比较运行好友 Agent，真实重合结果选择 `shared_affinity_strategy`；低重合分支由单元测试覆盖。
- Spotify 与 YouTube 账号 API 数据不进入 Compare；用户主动提供的文件或文本仍可执行完整本地比较。
