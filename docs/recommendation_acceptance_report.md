# MelodyPath 真实推荐最终验收报告

验收日期：2026-08-31  
项目目录：`C:\Users\user\Downloads\melody-path-local-analysis`  
最终状态：**阶段 2 未通过；已停止，未执行阶段 2.5、阶段 3 或阶段 4。**

## 1. 检查点

### 阶段 0：通过

- Rust 推荐代码、`RecommendationProvider`、`LastFmRecommendationProvider` 和现有测试仍然存在。
- 后端 `GET /health` 返回 HTTP 200。
- 前端实际运行于 `http://127.0.0.1:5174/`。
- Windows 用户级 Last.fm 配置存在；检查过程只读取“是否配置”的布尔状态，未读取、输出或记录配置内容。
- 真实单曲验证返回 `Last.fm Music Discovery API / ready`、`is_demo=false`，1 个种子查询成功并产生 80 个原始候选。

### 阶段 1：通过

以下文件已从 Downloads **复制**到项目 `testdata`；原文件未移动、修改或删除。四份副本均可读取，SHA-256 与来源文件一致。

- `testdata/Japanese.csv`：21,542 bytes
- `testdata/Korean.csv`：56,723 bytes
- `testdata/English.csv`：115,313 bytes
- `testdata/Chinese.csv`：228,012 bytes

### 阶段 2：未通过

四份文件均通过真实 Chrome 浏览器完成“选择文件 → 导入预览 → 确认并分析 → 打开探索推荐”流程。所有推荐请求均来自真实 Last.fm Provider；没有使用 Mock、Apple 关键词搜索、固定推荐池或 Demo 兜底。

未通过的直接原因：

1. English.csv 的舒适区出现 `Dancin - Krono Remix`，违反默认排除 Remix 等不同版本的验收要求。
2. English.csv 的拓展区同时出现 `Bang Bang` 和 `Bang Bang (Bonus Track)`，属于明显重复版本未合并。
3. Chinese.csv 原歌单已有 `晴天 — Jay Chou`，推荐中又出现 `晴天 — 周杰倫`；艺术家跨语言别名绕过了源歌单排除。
4. Chinese.csv 原歌单已有 `背對背擁抱 — JJ Lin`，推荐中又出现 `背對背擁抱 — 林俊傑`；同样属于艺术家别名漏排。
5. 当前响应只提供“去重及排除后”的合并数量，无法分别报告“仅去重后”和“再排除原歌单后”的两个独立数量。
6. 四次验收的惊喜区均为空；页面给出耗尽结论，但当前遥测没有分别暴露 `tag.getSimilar` 第一层、第二层以及各层 `tag.getTopTracks` 的候选和淘汰数量，无法完成逐层数量审计。

依照任务边界，本轮没有修改推荐算法来迎合测试，也没有继续阶段 2.5 或跨平台迁移 MVP。

## 2. 汇总对比

| 文件 | 总数 / 解析 / 分析 | Genre 覆盖 | Energy 覆盖 | 种子成功/失败 | track.getSimilar | artist 路径 | tag 路径 | 原始候选 | 去重及排除后 | 舒适/拓展/惊喜 | 用时 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| Japanese.csv | 76 / 76 / 76 | 75/76 (98.68%) | 76/76 (100%) | 10/0 | 142 | 32 | 20 | 202 | 181 | 4/4/0 | 9.31s |
| Korean.csv | 224 / 224 / 224 | 223/224 (99.55%) | 224/224 (100%) | 10/0 | 199 | 32 | 20 | 259 | 154 | 4/4/0 | 6.61s |
| English.csv | 435 / 435 / 435 | 208/435 (47.82%) | 435/435 (100%) | 10/0 | 180 | 32 | 20 | 240 | 172 | 4/4/0 | 12.64s |
| Chinese.csv | 781 / 781 / 781 | 778/781 (99.62%) | 781/781 (100%) | 10/0 | 82 | 32 | 20 | 142 | 110 | 4/4/0 | 8.84s |

说明：`artist 路径` 是当前响应暴露的 `raw_artist_similar_count`，对应 `artist.getSimilar → artist.getTopTracks` 候选；当前接口没有把两个调用阶段分别计数。`tag 路径` 是当前响应暴露的合并 `raw_tag_top_tracks_count`。

共同结论：

- 四份文件均为 `REAL_FILE`、`is_demo=false`。
- 页面均显示真实数据来源、Last.fm Provider、种子、查询统计和三个推荐区域。
- 四份歌单产生的展示推荐明显不同。
- 没有切换到 Demo；惊喜区不足时保持为空，没有固定歌曲补齐。
- 展示卡片分别显示 Last.fm similarity、Rust 匹配分数和 UI confidence；系统分数没有被无条件显示成 100%。但在当前实现中，track-similar 候选的 UI confidence 通常直接等于 Last.fm similarity，这是仍需产品定义澄清的限制。
- 展示候选的标签均为空；推荐理由来自 track similarity 或相似艺术家关系，而不是共享标签。

## 3. Japanese.csv

### 输入与 Provider

- 数据来源：`真实文件 · Japanese.csv`
- `is_demo=false`
- 总行数 / 成功解析 / 实际分析：76 / 76 / 76
- 完整 / 部分 / 未匹配元数据：75 / 1 / 0
- Genre：75/76（98.68%）
- Energy：76/76（100%）
- Provider：`Last.fm Music Discovery API`
- 状态：`ready`
- 种子成功 / 失败：10 / 0
- 原始候选：202；合并去重并排除源歌单后：181
- 原歌单精确重复：未发现
- Remix/Live/Sped Up 等版本候选：未发现

实际种子：

1. Dream lantern — RADWIMPS
2. KICK BACK — Kenshi Yonezu
3. たぶん — YOASOBI
4. Guren no Yumiya — Linked Horizon
5. 僕が死のうと思ったのは — Mika Nakashima
6. Call of Silence — Sawano Hiroyuki
7. とんぼ — Tsuyoshi Nagabuchi
8. NIGHT DANCER — imase
9. Ətˈæk 0n Tάɪtn — A.Shema
10. Akuma no Ko — Ai Higuchi

### 展示推荐

| 区域 | 推荐 | 关联种子 | 来源 | similarity | 系统 score | UI confidence | 标签 / 理由 |
|---|---|---|---|---:|---:|---:|---|
| 舒适 | Peace Sign — Kenshi Yonezu | KICK BACK | track.getSimilar | 100% | 66% | 100% | 无标签；听众相似关系 |
| 舒适 | Jiyuu no Tsubasa — Linked Horizon | Guren no Yumiya | track.getSimilar | 100% | 62% | 100% | 无标签；听众相似关系 |
| 舒适 | 雪の華 — 中島美嘉 | 僕が死のうと思ったのは | track.getSimilar | 100% | 60% | 100% | 无标签；听众相似关系 |
| 舒适 | attack on D — Sawano Hiroyuki | Call of Silence | track.getSimilar | 100% | 58% | 100% | 无标签；听众相似关系 |
| 拓展 | SOUVENIR — BUMP OF CHICKEN | RADWIMPS（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 无标签；相似艺术家代表曲 |
| 拓展 | Hello,world! — BUMP OF CHICKEN | RADWIMPS（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 无标签；相似艺术家代表曲 |
| 拓展 | 一途 — King Gnu | Kenshi Yonezu（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 无标签；相似艺术家代表曲 |
| 拓展 | Prayer X — King Gnu | Kenshi Yonezu（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 无标签；相似艺术家代表曲 |

惊喜区：0 首。页面原因：`第二层关联标签和较远相似艺术家网络候选已耗尽`。当前只能观察到 tag 合并原始数 20，无法分别得到第一层、第二层、源歌单排除、重复排除和区域截断数量。

## 4. Korean.csv

### 输入与 Provider

- 数据来源：`真实文件 · Korean.csv`
- `is_demo=false`
- 总行数 / 成功解析 / 实际分析：224 / 224 / 224
- 完整 / 部分 / 未匹配元数据：223 / 1 / 0
- Genre：223/224（99.55%）
- Energy：224/224（100%）
- 种子成功 / 失败：10 / 0
- 原始候选：259；合并去重并排除后：154
- 原歌单重复与不应出现的版本候选：未发现

实际种子：Alcohol-Free — TWICE；DINOSAUR — AKMU；CASE 143 — Stray Kids；As If It's Your Last — BLACKPINK；BEAUTIFUL — SEVENTEEN；Armageddon — aespa；ALREADY — i-dle；BAE BAE — BIGBANG；Attention — NewJeans；After LIKE — IVE。

### 展示推荐

| 区域 | 推荐 | 关联种子 | 来源 | similarity | score | confidence | 标签 / 理由 |
|---|---|---|---|---:|---:|---:|---|
| 舒适 | LOVE LEE — AKMU | DINOSAUR | track.getSimilar | 100% | 66% | 100% | 无标签；听众相似 |
| 舒适 | DOMINO — Stray Kids | CASE 143 | track.getSimilar | 100% | 64% | 100% | 无标签；听众相似 |
| 舒适 | How can I love the heartbreak, you're the one I love — AKMU | DINOSAUR | track.getSimilar | 89.41% | 60.91% | 89.41% | 无标签；听众相似 |
| 舒适 | Campfire — Seventeen | BEAUTIFUL | track.getSimilar | 100% | 60% | 100% | 无标签；听众相似 |
| 拓展 | Step Back — GOT the beat | aespa（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 无标签；相似艺术家代表曲 |
| 拓展 | Stamp On It — GOT the beat | aespa（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 无标签；相似艺术家代表曲 |
| 拓展 | Wishing On You — JIHYO | TWICE（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 无标签；相似艺术家代表曲 |
| 拓展 | Room — JIHYO | TWICE（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 无标签；相似艺术家代表曲 |

惊喜区：0 首；耗尽原因及遥测限制与 Japanese.csv 相同。

## 5. English.csv

### 输入与 Provider

- 数据来源：`真实文件 · English.csv`
- `is_demo=false`
- 总行数 / 成功解析 / 实际分析：435 / 435 / 435
- 完整 / 部分 / 未匹配元数据：208 / 227 / 0
- Genre：208/435（47.82%）
- Energy：435/435（100%）
- 种子成功 / 失败：10 / 0
- 原始候选：240；合并去重并排除后：172
- 失败：出现 Remix 候选，并出现明显重复版本。

实际种子：God is a woman — Ariana Grande；That’s Not How This Works - Sabrina’s Version — Charlie Puth / Dan + Shay / Sabrina Carpenter；One Kiss (with Dua Lipa) — Calvin Harris / Dua Lipa；ME! (feat. Brendon Urie of Panic! At The Disco) — Taylor Swift / Brendon Urie / Panic! At The Disco；Stuck with U (with Justin Bieber) — Ariana Grande / Justin Bieber；Die With A Smile — Lady Gaga / Bruno Mars；Animals — Maroon 5；Enemy (with JID) — Imagine Dragons / JID / Arcane / League of Legends；Bad Romance — Lady Gaga；Fallin' All In You — Shawn Mendes。

### 展示推荐

| 区域 | 推荐 | 关联种子 | 来源 | similarity | score | confidence | 结论 |
|---|---|---|---|---:|---:|---:|---|
| 舒适 | no tears left to cry — Ariana Grande | God is a woman | track.getSimilar | 100% | 68% | 100% | 通过 |
| 舒适 | breathin — Ariana Grande | God is a woman | track.getSimilar | 92.46% | 64.38% | 92.46% | 通过 |
| 舒适 | **Dancin - Krono Remix — Aaron Smith** | One Kiss | track.getSimilar | 100% | 64% | 100% | **失败：默认不应推荐 Remix** |
| 舒适 | Blade of Grass — Lady Gaga | Die With A Smile | track.getSimilar | 100% | 58% | 100% | 通过 |
| 拓展 | Leave The Door Open — Bruno Mars, Anderson .Paak, Silk Sonic | Bruno Mars（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 通过 |
| 拓展 | Smokin Out The Window — Bruno Mars, Anderson .Paak, Silk Sonic | Bruno Mars（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 通过 |
| 拓展 | **Bang Bang — Jessie J, Ariana Grande & Nicki Minaj** | Ariana Grande（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 与下一项明显重复 |
| 拓展 | **Bang Bang (Bonus Track) — Jessie J, Ariana Grande & Nicki Minaj** | Ariana Grande（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | **失败：重复版本未合并** |

惊喜区：0 首；耗尽原因及遥测限制与 Japanese.csv 相同。

## 6. Chinese.csv（大歌单与性能）

### 输入与 Provider

- 数据来源：`真实文件 · Chinese.csv`
- `is_demo=false`
- 总行数 / 成功解析 / 实际分析：781 / 781 / 781
- 完整 / 部分 / 未匹配元数据：778 / 3 / 0
- Genre：778/781（99.62%）
- Energy：781/781（100%）
- 种子成功 / 失败：10 / 0
- 原始候选：142；合并去重并排除后：110
- 从确认分析到响应并进入推荐页约 8.84 秒。
- 失败：艺术家跨语言别名导致原歌单歌曲重复推荐。

实际种子：Mojito — Jay Chou；不該 — Jay Chou / A-Mei Chang；一千年以後 — JJ Lin；一夜長大 — Fish Leong；A.I. 愛 — Leehom Wang；不是真的愛我 — Stefanie Sun；世界贈予我的 — Faye Wong；Don't Stop (Bring it all back) — JOLIN；今天妳要嫁給我 — JOLIN / David Tao；手心的薔薇 — JJ Lin / G.E.M.。

### 展示推荐

| 区域 | 推荐 | 关联种子 | 来源 | similarity | score | confidence | 结论 |
|---|---|---|---|---:|---:|---:|---|
| 舒适 | 愛笑的眼睛 — 林俊傑 | 一千年以後 | track.getSimilar | 100% | 64% | 100% | 通过 |
| 舒适 | 對不起我愛你 — 梁靜茹 | 一夜長大 | track.getSimilar | 100% | 62% | 100% | 通过 |
| 舒适 | **背對背擁抱 — 林俊傑** | 一千年以後 | track.getSimilar | 87.75% | 58.12% | 87.75% | **失败：源文件已有同曲 `JJ Lin` 版本** |
| 舒适 | 風箏 — 孫燕姿 | 不是真的愛我 | track.getSimilar | 100% | 58% | 100% | 通过 |
| 拓展 | 一路向北 — 周杰倫 | JJ Lin（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 通过 |
| 拓展 | **晴天 — 周杰倫** | JJ Lin（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | **失败：源文件已有 `晴天 — Jay Chou`** |
| 拓展 | Afterward — 孫燕姿 | Fish Leong（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 通过 |
| 拓展 | In The Beginning — 孫燕姿 | Fish Leong（艺术家） | artist.getSimilar → artist.getTopTracks | 100% | 64% | 100% | 通过 |

惊喜区：0 首；耗尽原因及遥测限制与 Japanese.csv 相同。

## 7. 自动检查

以下命令全部通过：

- `cargo fmt --all --check`
- `cargo check --workspace`
- `cargo test --workspace`：54 passed，0 failed
- `frontend/npm run lint`
- `frontend/npm run build`

自动测试通过但真实验收失败，说明现有单元测试没有覆盖：

- 非源歌曲本身的 Remix/Live 等版本候选全局过滤；
- `Bonus Track` 等明显重复版本；
- `Jay Chou ↔ 周杰倫`、`JJ Lin ↔ 林俊傑` 等跨语言艺术家别名下的源歌单排除。

## 8. 停止状态与人工处理建议

当前检查点：**阶段 2 失败，停止。**

未执行：

- 阶段 2.5 的 ZIP 备份；
- README 能力更新；
- `/transfer` 页面；
- Spotify → YouTube 迁移代码、Mock 测试或真实 OAuth 验收。

后续若允许进入修复轮次，最小修复范围应只针对候选版本过滤、重复版本归一化、艺术家别名参与源歌单排除，以及统计遥测拆分；修复后必须重新按四份文件顺序执行本报告中的浏览器验收。未经新的明确指令，不应开始跨平台迁移阶段。
