use crate::{
    models::{PlatformTrackMatch, Track},
    writers::PlaylistWriter,
};
use anyhow::{Result, bail};

/// Spotify-specific orchestration over the existing OAuth-backed connector.
/// It deliberately owns no credentials and does not alter import behavior.
pub struct SpotifyPlaylistExportService<'a> {
    writer: &'a dyn PlaylistWriter,
}

impl<'a> SpotifyPlaylistExportService<'a> {
    pub fn new(writer: &'a dyn PlaylistWriter) -> Self {
        Self { writer }
    }

    pub async fn match_tracks(
        &self,
        tracks: &[Track],
        auth_session: Option<&str>,
    ) -> Result<Vec<PlatformTrackMatch>> {
        let status = self.writer.authorize(auth_session).await?;
        if !status.authorized {
            bail!("AUTHENTICATION_MISSING: Spotify write authorization is required");
        }
        let mut matches = Vec::with_capacity(tracks.len());
        for track in tracks {
            matches.push(self.writer.search_track(track, auth_session).await?);
        }
        Ok(matches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        models::{MatchStatus, VersionType, WriterStatus},
        writers::{AddTracksOutcome, CreatedPlaylist},
    };
    use async_trait::async_trait;
    use std::collections::HashMap;

    struct MockSpotifyWriter {
        authorized: bool,
        match_status: MatchStatus,
    }

    #[async_trait]
    impl PlaylistWriter for MockSpotifyWriter {
        fn platform(&self) -> &'static str {
            "spotify"
        }

        async fn authorize(&self, _session: Option<&str>) -> Result<WriterStatus> {
            Ok(WriterStatus {
                platform: "spotify".into(),
                label: "Spotify".into(),
                availability: "available".into(),
                authorized: self.authorized,
                is_demo: false,
                message: String::new(),
            })
        }

        async fn search_track(
            &self,
            track: &Track,
            _session: Option<&str>,
        ) -> Result<PlatformTrackMatch> {
            let matched = self.match_status == MatchStatus::Matched;
            Ok(PlatformTrackMatch {
                source_track: track.clone(),
                target_platform: "spotify".into(),
                target_track_id: matched.then(|| "spotify-track-id".into()),
                target_url: None,
                version_type: VersionType::Original,
                confidence: if matched { 0.95 } else { 0.0 },
                match_reason: if matched { "matched" } else { "no match" }.into(),
                status: self.match_status.clone(),
                candidates: Vec::new(),
            })
        }

        async fn create_playlist(
            &self,
            _name: &str,
            _session: Option<&str>,
        ) -> Result<CreatedPlaylist> {
            unreachable!()
        }

        async fn add_tracks(
            &self,
            _playlist_id: &str,
            _track_ids: &[String],
            _session: Option<&str>,
        ) -> Result<AddTracksOutcome> {
            Ok(AddTracksOutcome {
                added_ids: Vec::new(),
                failures: HashMap::new(),
            })
        }
    }

    fn track() -> Track {
        Track {
            id: "track-1".into(),
            title: "Blueming".into(),
            normalized_title: "blueming".into(),
            artists: vec!["IU".into()],
            album: None,
            genres: Vec::new(),
            release_year: None,
            language: None,
            duration_ms: None,
            platform: "musicbrainz".into(),
            platform_url: None,
            external_ids: HashMap::new(),
            version_type: VersionType::Original,
            mood_tags: Vec::new(),
            energy_score: None,
            popularity: None,
            metadata_confidence: 0.95,
        }
    }

    #[tokio::test]
    async fn spotify_search_success() {
        let writer = MockSpotifyWriter {
            authorized: true,
            match_status: MatchStatus::Matched,
        };
        let matches = SpotifyPlaylistExportService::new(&writer)
            .match_tracks(&[track()], Some("session"))
            .await
            .unwrap();
        assert_eq!(
            matches[0].target_track_id.as_deref(),
            Some("spotify-track-id")
        );
    }

    #[tokio::test]
    async fn spotify_search_no_match_is_reported_without_fake_id() {
        let writer = MockSpotifyWriter {
            authorized: true,
            match_status: MatchStatus::Unmatched,
        };
        let matches = SpotifyPlaylistExportService::new(&writer)
            .match_tracks(&[track()], Some("session"))
            .await
            .unwrap();
        assert_eq!(matches[0].status, MatchStatus::Unmatched);
        assert!(matches[0].target_track_id.is_none());
    }

    #[tokio::test]
    async fn spotify_authentication_missing_stops_before_search() {
        let writer = MockSpotifyWriter {
            authorized: false,
            match_status: MatchStatus::Matched,
        };
        let error = SpotifyPlaylistExportService::new(&writer)
            .match_tracks(&[track()], None)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("AUTHENTICATION_MISSING"));
    }
}
