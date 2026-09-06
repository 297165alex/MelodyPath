pub mod apple;
pub mod spotify;
pub mod youtube;

use crate::models::{
    MatchCandidate, MatchStatus, PlatformTrackMatch, Track, VersionType, WriterStatus,
};
use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct CreatedPlaylist {
    pub id: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AddTracksOutcome {
    pub added_ids: Vec<String>,
    pub failures: HashMap<String, String>,
}

#[async_trait]
pub trait PlaylistWriter: Send + Sync {
    fn platform(&self) -> &'static str;
    async fn authorize(&self, auth_session: Option<&str>) -> Result<WriterStatus>;
    async fn search_track(
        &self,
        track: &Track,
        auth_session: Option<&str>,
    ) -> Result<PlatformTrackMatch>;
    async fn create_playlist(
        &self,
        name: &str,
        auth_session: Option<&str>,
    ) -> Result<CreatedPlaylist>;
    async fn add_tracks(
        &self,
        playlist_id: &str,
        track_ids: &[String],
        auth_session: Option<&str>,
    ) -> Result<AddTracksOutcome>;
    async fn get_playlist_url(
        &self,
        playlist: &CreatedPlaylist,
        _auth_session: Option<&str>,
    ) -> Result<Option<String>> {
        Ok(playlist.url.clone())
    }
}

#[derive(Debug, Clone)]
pub struct ExportFilePlaylistWriter;

#[async_trait]
impl PlaylistWriter for ExportFilePlaylistWriter {
    fn platform(&self) -> &'static str {
        "file"
    }

    async fn authorize(&self, _auth_session: Option<&str>) -> Result<WriterStatus> {
        Ok(status(
            "file",
            "导出歌曲清单",
            "available",
            true,
            false,
            "无需账号授权；确认后下载真实文件。",
        ))
    }

    async fn search_track(
        &self,
        track: &Track,
        _auth_session: Option<&str>,
    ) -> Result<PlatformTrackMatch> {
        Ok(exact_match(
            track,
            "file",
            "导出保留源歌曲信息，无需跨平台匹配",
        ))
    }

    async fn create_playlist(
        &self,
        name: &str,
        _auth_session: Option<&str>,
    ) -> Result<CreatedPlaylist> {
        Ok(CreatedPlaylist {
            id: name.to_string(),
            url: None,
        })
    }

    async fn add_tracks(
        &self,
        _playlist_id: &str,
        track_ids: &[String],
        _auth_session: Option<&str>,
    ) -> Result<AddTracksOutcome> {
        Ok(AddTracksOutcome {
            added_ids: track_ids.to_vec(),
            failures: HashMap::new(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct DemoPlaylistWriter;

#[async_trait]
impl PlaylistWriter for DemoPlaylistWriter {
    fn platform(&self) -> &'static str {
        "demo"
    }

    async fn authorize(&self, _auth_session: Option<&str>) -> Result<WriterStatus> {
        Ok(status(
            "demo",
            "Demo 虚拟歌单",
            "available",
            true,
            true,
            "演示完整流程，但不会写入任何真实平台。",
        ))
    }

    async fn search_track(
        &self,
        track: &Track,
        _auth_session: Option<&str>,
    ) -> Result<PlatformTrackMatch> {
        Ok(exact_match(track, "demo", "Demo 使用离线曲库进行流程演示"))
    }

    async fn create_playlist(
        &self,
        _name: &str,
        _auth_session: Option<&str>,
    ) -> Result<CreatedPlaylist> {
        let id = uuid::Uuid::new_v4().to_string();
        Ok(CreatedPlaylist {
            id: id.clone(),
            url: Some(format!("demo://playlist/{id}")),
        })
    }

    async fn add_tracks(
        &self,
        _playlist_id: &str,
        track_ids: &[String],
        _auth_session: Option<&str>,
    ) -> Result<AddTracksOutcome> {
        Ok(AddTracksOutcome {
            added_ids: track_ids.to_vec(),
            failures: HashMap::new(),
        })
    }
}

#[derive(Debug, Clone)]
pub struct UnavailablePlaylistWriter {
    pub platform_name: &'static str,
    pub display_name: &'static str,
    pub reason: &'static str,
}

#[async_trait]
impl PlaylistWriter for UnavailablePlaylistWriter {
    fn platform(&self) -> &'static str {
        self.platform_name
    }

    async fn authorize(&self, _auth_session: Option<&str>) -> Result<WriterStatus> {
        Ok(status(
            self.platform_name,
            self.display_name,
            "planned",
            false,
            false,
            self.reason,
        ))
    }

    async fn search_track(
        &self,
        _track: &Track,
        _auth_session: Option<&str>,
    ) -> Result<PlatformTrackMatch> {
        anyhow::bail!(self.reason)
    }

    async fn create_playlist(
        &self,
        _name: &str,
        _auth_session: Option<&str>,
    ) -> Result<CreatedPlaylist> {
        anyhow::bail!(self.reason)
    }

    async fn add_tracks(
        &self,
        _playlist_id: &str,
        _track_ids: &[String],
        _auth_session: Option<&str>,
    ) -> Result<AddTracksOutcome> {
        anyhow::bail!(self.reason)
    }
}

pub fn status(
    platform: &str,
    label: &str,
    availability: &str,
    authorized: bool,
    is_demo: bool,
    message: &str,
) -> WriterStatus {
    WriterStatus {
        platform: platform.into(),
        label: label.into(),
        availability: availability.into(),
        authorized,
        is_demo,
        message: message.into(),
    }
}

fn exact_match(track: &Track, platform: &str, reason: &str) -> PlatformTrackMatch {
    PlatformTrackMatch {
        source_track: track.clone(),
        target_platform: platform.into(),
        target_track_id: Some(track.id.clone()),
        target_url: track.platform_url.clone(),
        version_type: track.version_type.clone(),
        confidence: 1.0,
        match_reason: reason.into(),
        status: MatchStatus::Matched,
        candidates: vec![MatchCandidate {
            target_track_id: track.id.clone(),
            title: track.title.clone(),
            artists: track.artists.clone(),
            album: track.album.clone(),
            duration_ms: track.duration_ms,
            channel_name: None,
            official_status: if platform == "demo" {
                "mock_connector"
            } else {
                "source_metadata"
            }
            .into(),
            target_url: track.platform_url.clone(),
            version_type: VersionType::Original,
            confidence: 1.0,
            match_reason: reason.into(),
            available_in_market: true,
        }],
    }
}

pub fn unavailable_writers() -> Vec<UnavailablePlaylistWriter> {
    vec![
        UnavailablePlaylistWriter {
            platform_name: "apple_music",
            display_name: "Apple Music",
            reason: "已预留 MusicKit 适配器；当前版本未配置真实写入，建议导出清单。",
        },
        UnavailablePlaylistWriter {
            platform_name: "netease",
            display_name: "网易云音乐",
            reason: "暂无稳定、合规的公开写入 API；不会索取密码或私人 Cookie。",
        },
        UnavailablePlaylistWriter {
            platform_name: "qq_music",
            display_name: "QQ 音乐",
            reason: "暂无稳定、合规的公开写入 API；不会绕过登录或验证码。",
        },
    ]
}
