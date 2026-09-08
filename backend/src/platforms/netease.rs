//! Anonymous public JSON-LD metadata only. Never execute page scripts, decode
//! application state, or infer missing recordings through a metadata provider.
use regex::Regex;
use serde_json::Value;
use std::sync::LazyLock;

static JSON_LD: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r#"(?is)<script\b[^>]*\btype\s*=\s*["']application/ld\+json["'][^>]*>(.*?)</script\s*>"#,
    )
    .unwrap()
});

#[derive(Debug, Default)]
pub(super) struct PublicMetadata {
    pub name: Option<String>,
    /// Page-declared total, never the number of successfully imported tracks.
    pub declared_tracks: Option<usize>,
    pub listed_tracks: usize,
    pub status: &'static str,
}

impl PublicMetadata {
    pub fn explanation(&self) -> String {
        format!(
            "公开页面声明 {} 首，JSON-LD 实际列出 {} 项。{} 未生成 Track；请使用文件或文本进入现有导入预览与推荐流程。",
            self.declared_tracks
                .map(|n| n.to_string())
                .unwrap_or_else(|| "未知".into()),
            self.listed_tracks,
            match self.status {
                "public_track_list_incomplete" => "公开歌曲列表不完整。",
                "public_track_metadata_incomplete" => "歌曲缺少标题或艺人，不能转换为标准 Track。",
                "public_track_count_unverified" => "无法验证歌曲列表完整性。",
                "public_track_import_policy_unverified" =>
                    "结构存在，但尚未核实适用于本项目的自动曲目导入许可。",
                _ => "没有可用的公开歌单结构化数据。",
            }
        )
    }
}

fn has_type(value: &Value, kind: &str) -> bool {
    value["@type"].as_str() == Some(kind)
        || value["@type"]
            .as_array()
            .is_some_and(|types| types.iter().any(|t| t.as_str() == Some(kind)))
}

fn playlist(value: &Value) -> Option<&Value> {
    if has_type(value, "MusicPlaylist") {
        Some(value)
    } else {
        value
            .as_array()
            .or_else(|| value["@graph"].as_array())?
            .iter()
            .find(|item| has_type(item, "MusicPlaylist"))
    }
}

fn nonempty(value: &Value) -> Option<&str> {
    value.as_str().map(str::trim).filter(|s| !s.is_empty())
}

pub(super) fn extract(body: &[u8]) -> PublicMetadata {
    let Ok(text) = std::str::from_utf8(body) else {
        return PublicMetadata {
            status: "invalid_public_page_encoding",
            ..Default::default()
        };
    };
    for script in JSON_LD.captures_iter(text) {
        let Ok(json) = serde_json::from_str::<Value>(&script[1]) else {
            continue;
        };
        let Some(data) = playlist(&json) else {
            continue;
        };
        let items = data["track"]
            .as_array()
            .or_else(|| data["track"]["itemListElement"].as_array());
        let declared = data["numTracks"]
            .as_u64()
            .and_then(|n| usize::try_from(n).ok());
        let count = items.map_or(0, Vec::len);
        let status = if declared.is_none() {
            "public_track_count_unverified"
        } else if declared != Some(count) || items.is_none() {
            "public_track_list_incomplete"
        } else if items.is_some_and(|items| {
            items.iter().any(|item| {
                let track = if has_type(item, "ListItem") {
                    &item["item"]
                } else {
                    item
                };
                !has_type(track, "MusicRecording")
                    || nonempty(&track["name"]).is_none()
                    || !(nonempty(&track["byArtist"]["name"]).is_some()
                        || track["byArtist"].as_array().is_some_and(|artists| {
                            !artists.is_empty()
                                && artists
                                    .iter()
                                    .all(|artist| nonempty(&artist["name"]).is_some())
                        }))
            })
        }) {
            "public_track_metadata_incomplete"
        } else {
            "public_track_import_policy_unverified"
        };
        return PublicMetadata {
            name: nonempty(&data["name"]).map(|s| s.chars().take(300).collect()),
            declared_tracks: declared,
            listed_tracks: count,
            status,
        };
    }
    PublicMetadata {
        status: "no_public_playlist_structure",
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(value: Value) -> Vec<u8> {
        format!("<script type='application/ld+json'>{value}</script>").into_bytes()
    }

    #[test]
    fn partial_public_metadata_never_means_complete_tracks() {
        let result = extract(&page(serde_json::json!({
            "@type": ["CreativeWork", "MusicPlaylist"], "name": "公开测试歌单",
            "numTracks": 15, "track": {"numberOfItems": 15, "itemListElement": [
                {"@type":"ListItem", "item":{"@type":"MusicRecording", "name":"测试曲目"}}
            ]}
        })));
        assert_eq!(result.name.as_deref(), Some("公开测试歌单"));
        assert_eq!(result.declared_tracks, Some(15));
        assert_eq!(result.listed_tracks, 1);
        assert_eq!(result.status, "public_track_list_incomplete");
    }

    #[test]
    fn complete_count_without_artist_is_not_a_track() {
        let result = extract(&page(serde_json::json!({"@graph":[{
            "@type":"MusicPlaylist", "numTracks":1,
            "track":[{"@type":"MusicRecording", "name":"Synthetic"}]
        }]})));
        assert_eq!(result.status, "public_track_metadata_incomplete");
    }

    #[test]
    fn even_complete_structure_does_not_assert_platform_permission() {
        let result = extract(&page(serde_json::json!({
            "@type":"MusicPlaylist", "numTracks":1,
            "track":[{"@type":"MusicRecording", "name":"Synthetic", "byArtist":{"name":"Synthetic artist"}}]
        })));
        assert_eq!(result.status, "public_track_import_policy_unverified");
    }

    #[test]
    fn malformed_truncated_internal_state_and_unrelated_json_are_not_metadata() {
        for body in [
            "<script>window.__INITIAL_STATE__={\"name\":\"not public metadata\"}</script>",
            "<script type='application/ld+json'>{\"@type\":\"MusicPlaylist\"",
            "<script type='application/ld+json'>{invalid}</script>",
            "<script type='application/ld+json'>{\"@type\":\"MusicRecording\",\"name\":\"other\"}</script>",
            "<html>Please log in</html>",
        ] {
            let result = extract(body.as_bytes());
            assert!(result.name.is_none());
            assert!(result.declared_tracks.is_none());
            assert_eq!(result.listed_tracks, 0);
        }
    }
}
