# Platform-neutral Music Metadata Layer

更新时间：2026-09-08

## 数据流

```text
File / pasted `artist - title`
        ↓
Existing import parser
        ↓
MusicBrainz metadata resolver
        ↓
Normalized Track
        ↓
Analysis / Recommendation / platform export adapters
```

`backend/src/resolver/` 是平台无关的 metadata 边界。`MetadataResolver` 接收统一 `Track`，返回标准 Track、`HIGH_MATCH / MEDIUM_MATCH / UNMATCHED`、来源和确定性置信度。MusicBrainz 实现只使用公开后端 API，不需要用户账号或 API key；请求带应用 User-Agent，并按官方要求限制为平均每秒最多一次。

MusicBrainz 候选按标题、艺术家以及双方都有真实值时的时长进行确定性评分。达到阈值才替换标准 Track，并保存 recording MBID、专辑和时长等实际返回字段；没有候选、低置信候选、超时或 API 错误时返回 `UNMATCHED`，原始标题和艺术家继续进入分析，不生成补造字段。既有公开目录和本地画像仍是兼容降级，不改变 Apple connector。

## Spotify export

`backend/src/export/spotify_playlist.rs` 在既有 `PlaylistWriter` 和 Spotify OAuth connector 之上编排标准 Track 匹配。它不保存凭据、不复制 OAuth 逻辑，也不参与 Spotify import。前端只在已连接状态下从 Analysis 或 Recommendation 显示 `Create Spotify Playlist`；随后仍经过候选预览、歧义处理和明确确认，默认创建新的私有歌单。

Spotify 搜索返回的目标 ID 和来源只用于用户主动发起的写回，不进入画像、推荐、跨平台比较或 LLM。未来网易云、QQ音乐、酷狗、汽水等合法 adapter 可以消费同一 Track pipeline，而无需改变 parser、resolver 或分析核心。

## 失败与真实性边界

- MusicBrainz 未匹配不会删除输入歌曲，也不会阻断导入。
- `Source: MusicBrainz` 只在真实候选达到匹配阈值时显示。
- `Source: Spotify` 只表示 Spotify 官方搜索候选；不表示用户已确认或歌单已创建。
- 缺少 Spotify 写入授权时在搜索前停止；Mock 不代表真实 Spotify 写入验收。
