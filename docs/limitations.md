# 限制、政策边界与验收状态

> MelodyPath是《程序设计训练（Rust语言）》AI Agent课程项目，同时将阶段性成果用于智理杯智能体大赛。

## Spotify 与 YouTube 数据隔离

Spotify 当前政策禁止分析 Spotify Content/Service 来生成衍生指标或画像，也禁止将其摄入 AI/ML。YouTube 政策要求只使用 API 提供的指标并限制独立衍生指标。代码因此返回细粒度 `data_use`：官方数据可读取、展示归属、传输和写回，但 `can_analyze_content`、`can_derive_metrics`、`can_cross_platform_compare`、`can_send_to_llm` 与 `can_train_model` 为 false。它们不会进入 Agent、LLM、历史画像或好友相似度路径。

这不等于禁用全部导入：用户仍可选择和预览官方歌单、转换为统一 Track 用于合法传输，并创建新歌单。完整分析使用用户主动输入且拥有处理权的数据、文件或明确标注 Demo。

## 公开分享链接

仅允许 HTTPS、官方域名和受约束的官方重定向；不发送用户 Cookie。页面检查会报告最终 URL、歌单 ID 和公开页面是否存在结构化痕迹。网易云仅导入公开 HTML / JSON-LD 明确暴露且可验证的歌曲，最多 20 首，未公开成员不推断；没有可用歌曲时保持可访问性检查。QQ 当前未声称真实曲目读取。

## 安全与部署

- 页面没有音乐平台密码、Cookie、Client Secret 或私钥输入框。
- OAuth state 使用一次性 HttpOnly cookie 校验；token 只在后端内存中，解除连接会删除。服务重启也会清空 token。
- 生产环境仍需加密凭据仓库、用户会话绑定、HTTPS、密钥轮换、审计与隐私删除流程。
- Apple Music 目前只有正式配置检测/bootstrap，尚未完成 MusicKit 真实账号和歌单读写验收。
- 开发者模式、账号资格、配额与平台审核会随平台政策变化；部署前须重新核验。
