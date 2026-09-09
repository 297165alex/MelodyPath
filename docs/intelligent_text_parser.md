# Intelligent Text Parser 与 Taste Profile

日期：2026-09-09。

## 文本解析边界

调用链保持为 `Rule Parser → 可选 LLM structure fallback → Import Preview → 用户确认 → MetadataResolver → Analysis / Recommendation`。规则层支持中英文、日文、韩文、Unicode NFKC、横线、`by`、书名号/日韩引号、括号、编号、斜杠、冒号及可验证的无分隔符艺人。规则结果完整且高置信度时不会调用模型。

LLM fallback 只允许处理用户主动粘贴的 `REAL_TEXT`，不处理文件、Spotify、YouTube、Apple 或中国平台返回的数据。请求最多 100 行 / 20,000 字符；模型只能返回原顺序、相同行数的 `{ title, artist, confidence }`。Rust 随后要求 title 和 artist 非空、confidence 至少 0.8，且规范化后的两个字段都真实存在于对应原行。任一检查失败时不接受部分猜测，统一返回 `Need confirmation`。结构化成功也只表示拆分成功，不表示歌曲真实存在；确认后仍由 MetadataResolver 验证。

自动测试使用本地合成 OpenAI-compatible HTTP 响应，不读取或证明真实模型配置，不构成真实 LLM 验收。

## Taste Profile

新增统一 `RankingItem { name, count, rank, tied }`。竞争排名按数量生成，例如两项同为 10 首时均为 Rank 1 且 `tied=true`，下一名从 Rank 3 开始。Taste Profile 展示 Top Artists、Top Albums、Language Distribution 和 Genres，仅消费 Resolver 结果不是 `UNMATCHED` 的 Track。未匹配歌曲继续保留在源歌曲和既有基础分析中，但不进入该画像排名。

## 中国平台

QQ Music 只接受精确 HTTPS 官方主机和数字歌单路由，随后只解析公开 HTML 内的 `MusicPlaylist` JSON-LD。标题与艺人缺一即跳过；曲目 URL 只保留 QQ 官方 HTTPS 地址，否则回退到已验证的公开歌单 URL。页面不可用、页面壳、无完整曲目或结构不可信时返回 `ACCESSIBILITY_CHECK_ONLY`，不生成 Track。酷狗沿用相同公开 metadata 安全边界；汽水仍只做 capability detection。
