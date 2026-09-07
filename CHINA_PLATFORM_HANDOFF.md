# 中国平台专项交接

日期：2026-09-08。目录 `C:\Users\user\Downloads\MelodyPath`；分支 `feature/china-platform-links`；基线 `01f143c`，起始工作区干净。按网易云 → QQ → 酷狗 → 汽水顺序，每个平台最多一轮官方文档/公开页面/匿名 HTML 核查。不使用 Cookie、私有接口或解密。

## 网易云（本轮判断完成）

- Login/API：未找到本项目可直接接入的通用 Web 歌单 OAuth。官方开发者中心本轮打开失败；用户歌单授权、企业资格、商务合作、费用和普通开发者凭据申请规则均未取得可验证规范，不能把“未核实”写成“不存在”或“免费可用”。
- Public URL：匿名 HTTPS GET 官方公开歌单返回 HTML。页面声明 15 首；MusicPlaylist JSON-LD 的 itemListElement 实际只有 10 项，只有歌曲名和 URL，没有艺人和时长。页面另有编码的 song-list-pre-data，不解码、不调用站内接口。JSON-LD 的 numberOfItems 声明不能替代实际数组完整性检查。
- implemented：保留现有 URL/fragment/mobile 识别、canonical URL、ID、可访问性检查和文件/文本导入；不实现会返回不完整歌单的 parser。
- capability：ACCESSIBILITY_CHECK_ONLY + FILE_IMPORT_AVAILABLE；未升级 PUBLIC_PLAYLIST_IMPORT_AVAILABLE；未真人验收。
- limitation：本次样本不足以证明所有公开页面都不可读，但已足以否定“仅依靠该 JSON-LD 就能完整导入”。不以搜索结果拼歌单，不补 Unknown 艺人或 0 时长。
- 官方证据：[开发者入口](https://developer.music.163.com/)、[匿名公开歌单样本](https://music.163.com/playlist?id=7299150850)。样本来自公开索引，仅记结构和计数，不提交页面全文或歌单内容。
- tests：`cargo test --workspace platforms::tests`：10 passed / 0 failed，127 filtered out；普通测试未访问外网。

## QQ音乐

待完成；当前 ACCESSIBILITY_CHECK_ONLY + FILE_IMPORT_AVAILABLE。已开始查看官方音乐首页和腾讯连连文档；未得出新增能力结论。

## 酷狗音乐

本轮尚未审计；当前 URL_RECOGNITION_ONLY + FILE_IMPORT_AVAILABLE。

## 汽水音乐

本轮尚未审计；当前 URL_RECOGNITION_ONLY + FILE_IMPORT_AVAILABLE。

## 测试、修改、提交与恢复

- 当前修改：本交接文件。生产代码和能力矩阵未改动。
- 本地 checkpoint：本记录随 `Document NetEase public-page feasibility checkpoint` 提交；hash 用 `git log --oneline -- CHINA_PLATFORM_HANDOFF.md` 获取。
- 真人验收：当前没有新 URL reader，因此没有可宣称 WAITING_FOR_PUBLIC_PLAYLIST_VERIFICATION 的已实现功能；不要求用户登录或提供凭据。
- 下一步：取得平台测试结果并提交网易云判断，再完成 QQ 的单轮公开页面核查。若遇额度/上下文预警或研究停滞，停止研究，优先测试、更新本文并提交。
