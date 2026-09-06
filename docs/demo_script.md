# MelodyPath 五分钟中文演示脚本

> 只演示当前真实状态。真实文件、真实 Provider、确定性 fallback、Mock 和 Demo 必须按页面标签准确称呼；未完成真人 OAuth 时不得展示为平台成功。

## 0:00—0:30｜问题与定位

“音乐资产分散在不同平台，单个平台的推荐又容易把人困在舒适区。MelodyPath 是一个 Rust 核心、可解释、可审计的专用音乐 Agent：它把真实歌单分析、探索推荐、好友比较、版本雷达和跨平台复制放进同一个 Web App。”

镜头：首页平台能力区。说明按钮由后端 Capability 决定；没有官方能力的平台不会出现虚假 Connect，也没有密码或 Cookie 输入框。

## 0:30—1:10｜真实歌单 Analyze

上传一份已获授权的真实测试 CSV，先停在导入预览：总行数、成功解析数、警告、字段和前 20 首。确认后指出 `REAL_FILE`、`is_demo=false`、实际参与分析数、Genre/Energy 覆盖率。

“文件不会在解析失败时切换 Demo；缺失 Genre 或 Energy 也不会被编造成零值。”

## 1:10—1:50｜Discover：Comfort / Expansion / Surprise

展示三区各 4 首、核心标签、候选来源、Last.fm 原始 similarity、Rust score 与 UI confidence。指出 Surprise 来自 tag 两层、Genre bridge 或 second-hop artist 的可解释桥梁，不使用随机或固定歌曲。

## 1:50—2:10｜换一批

点击某一区的“换一批”，说明它从当前 Analysis Session 已保存的候选池向后翻页，不重新请求 Last.fm，已展示歌曲不立即重复；池耗尽时诚实显示“已看完本次高质量候选”。

## 2:10—2:55｜Agent Tool-Using Loop

在 Agent 页面绑定刚才的真实 Analysis 并启动个人探索。

“页面展示的是结构化、可核验轨迹：User Goal → Decision → 白名单 Rust Tool → ToolResult → Continue/Replan/Finish。解析、身份、评分、去重和写入仍由 Rust 决定。当前若未配置 LLM，页面明确显示 `DETERMINISTIC_FALLBACK`，不会冒充真实 LLM。”

展示真实 track count、种子、候选数、动态分支、SSE 进度、取消/恢复、Token 与费用。

## 2:55—3:35｜Compare

打开 `/compare`，为 Friend A/B 选择两份真实导入来源，分别预览并确认。展示 Track、Artist、Genre、Tag 与互补度，以及双方都能解释的桥梁候选；强调临时比较默认不保存，空结果不由 Demo 补齐。

## 3:35—4:10｜Version Radar

打开 `/versions`，选择当前 Analysis。说明扫描按批次、请求上限运行，支持取消；候选按源歌曲分组，显示 Original/Live/Concert/Remix/Acoustic/Unplugged/Remaster、官方/Topic 信号、置信度与理由。未完成 YouTube OAuth 时只展示阻塞状态，不点击 Mock 冒充真实搜索。

## 4:10—4:45｜Copy Playlist

打开 `/transfer`：Spotify source → YouTube destination → Match → Preview → Resolve ambiguous → Confirm → Create new private playlist → Write → Report。

“主产品语义是复制，原 Spotify 歌单不会被修改或删除。写入前必须确认歧义与版本 fallback；单曲失败不终止整个任务；run id、SSE、取消和恢复可核验。当前真人 OAuth 未完成，因此本页应显示 `MANUAL_AUTH_REQUIRED`，而不是展示假的目标链接。”

## 4:45—5:00｜Rust、成本与安全

“LLM 只做高层决策和解释，Rust 保有全部音乐事实与权限边界。Agent 有步骤、超时、重试、Token 和费用上限；OAuth token 不进浏览器 JavaScript、不写日志，生产部署使用 HTTPS、安全 Cookie 和托管 Token Store。一个公开域名的配置已准备好，真实部署与平台授权仍需人工完成。”
