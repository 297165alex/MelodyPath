use crate::models::Track;
use chrono::{Duration as ChronoDuration, NaiveDate, Utc};
use reqwest::{Client, redirect::Policy};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, time::Duration};
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseRadarRequest {
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReleaseUpdate {
    pub title: String,
    pub artist: String,
    pub release_date: String,
    pub release_type: String,
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReleaseRadarResult {
    pub provider: String,
    pub status: String,
    pub message: String,
    pub new_releases: Vec<ReleaseUpdate>,
    pub upcoming_albums: Vec<ReleaseUpdate>,
    pub artist_updates: Vec<ReleaseUpdate>,
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(rename = "release-groups", default)]
    release_groups: Vec<ReleaseGroup>,
}

#[derive(Debug, Clone, Deserialize)]
struct ReleaseGroup {
    id: String,
    title: String,
    #[serde(rename = "first-release-date")]
    first_release_date: Option<String>,
    #[serde(rename = "primary-type")]
    primary_type: Option<String>,
    #[serde(skip)]
    artist: String,
}

pub async fn scan(request: ReleaseRadarRequest) -> ReleaseRadarResult {
    let artists = request
        .tracks
        .iter()
        .flat_map(|track| track.artists.iter())
        .map(|artist| artist.trim())
        .filter(|artist| !artist.is_empty())
        .fold(
            (Vec::new(), HashSet::new()),
            |(mut ordered, mut seen), artist| {
                let key = artist.to_lowercase();
                if seen.insert(key) && ordered.len() < 5 {
                    ordered.push(artist.to_string());
                }
                (ordered, seen)
            },
        )
        .0;
    if artists.is_empty() {
        return empty("empty", "No update available");
    }
    let client = Client::builder()
        .timeout(Duration::from_secs(8))
        .redirect(Policy::none())
        .user_agent("MelodyPath/0.2 (+https://github.com/; release-radar)")
        .build()
        .expect("static client configuration");
    let mut groups = Vec::new();
    let mut failures = 0;
    for (index, artist) in artists.iter().enumerate() {
        if index > 0 {
            sleep(Duration::from_secs(1)).await;
        }
        let query = format!(
            "artist:\"{}\" AND status:official",
            artist.replace('"', "\\\"")
        );
        let response = client
            .get("https://musicbrainz.org/ws/2/release-group")
            .query(&[("query", query.as_str()), ("fmt", "json"), ("limit", "12")])
            .send()
            .await;
        match response {
            Ok(response) if response.status().is_success() => {
                match response.json::<SearchResponse>().await {
                    Ok(payload) => {
                        groups.extend(payload.release_groups.into_iter().map(|mut group| {
                            group.artist = artist.clone();
                            group
                        }))
                    }
                    Err(_) => failures += 1,
                }
            }
            _ => failures += 1,
        }
    }
    let (new_releases, upcoming_albums, artist_updates) = classify(groups, Utc::now().date_naive());
    let count = new_releases.len() + upcoming_albums.len() + artist_updates.len();
    let status = if count == 0 && failures == artists.len() {
        "error"
    } else if count == 0 {
        "empty"
    } else if failures > 0 {
        "partial"
    } else {
        "ready"
    };
    ReleaseRadarResult {
        provider: "MusicBrainz public metadata".into(),
        status: status.into(),
        message: if status == "error" {
            "Release data is temporarily unavailable.".into()
        } else if count == 0 {
            "No update available".into()
        } else {
            format!("Found {count} verifiable release updates; {failures} artist queries failed.")
        },
        new_releases,
        upcoming_albums,
        artist_updates,
    }
}

fn empty(status: &str, message: &str) -> ReleaseRadarResult {
    ReleaseRadarResult {
        provider: "MusicBrainz public metadata".into(),
        status: status.into(),
        message: message.into(),
        new_releases: vec![],
        upcoming_albums: vec![],
        artist_updates: vec![],
    }
}

fn classify(
    groups: Vec<ReleaseGroup>,
    today: NaiveDate,
) -> (Vec<ReleaseUpdate>, Vec<ReleaseUpdate>, Vec<ReleaseUpdate>) {
    let mut dated = groups
        .into_iter()
        .filter_map(|group| {
            let raw_date = group.first_release_date.as_deref()?;
            let date = parse_date(raw_date)?;
            Some((
                date,
                ReleaseUpdate {
                    title: group.title,
                    artist: group.artist,
                    release_date: raw_date.into(),
                    release_type: group.primary_type.unwrap_or_else(|| "Release".into()),
                    source_url: format!("https://musicbrainz.org/release-group/{}", group.id),
                },
            ))
        })
        .collect::<Vec<_>>();
    dated.sort_by(|left, right| right.0.cmp(&left.0));
    let new_releases = dated
        .iter()
        .filter(|(date, _)| *date <= today && *date >= today - ChronoDuration::days(365))
        .map(|(_, update)| update.clone())
        .take(8)
        .collect();
    let mut upcoming_albums = dated
        .iter()
        .filter(|(date, update)| *date > today && update.release_type.eq_ignore_ascii_case("album"))
        .map(|(_, update)| update.clone())
        .collect::<Vec<_>>();
    upcoming_albums.sort_by(|left, right| left.release_date.cmp(&right.release_date));
    upcoming_albums.truncate(8);
    let mut seen = HashSet::new();
    let artist_updates = dated
        .into_iter()
        .filter_map(|(_, update)| seen.insert(update.artist.to_lowercase()).then_some(update))
        .take(8)
        .collect();
    (new_releases, upcoming_albums, artist_updates)
}

fn parse_date(value: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .ok()
        .or_else(|| NaiveDate::parse_from_str(&format!("{value}-01"), "%Y-%m-%d").ok())
        .or_else(|| NaiveDate::parse_from_str(&format!("{value}-01-01"), "%Y-%m-%d").ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn group(id: &str, title: &str, artist: &str, date: &str, kind: &str) -> ReleaseGroup {
        ReleaseGroup {
            id: id.into(),
            title: title.into(),
            first_release_date: Some(date.into()),
            primary_type: Some(kind.into()),
            artist: artist.into(),
        }
    }

    #[test]
    fn release_radar_classifies_success_empty_and_upcoming() {
        let today = NaiveDate::from_ymd_opt(2026, 9, 9).unwrap();
        let (new_releases, upcoming, updates) = classify(
            vec![
                group("new", "New EP", "A", "2026-08-01", "EP"),
                group("future", "Next Album", "B", "2026-12-01", "Album"),
                group("old", "Old", "C", "2020", "Album"),
            ],
            today,
        );
        assert_eq!(new_releases[0].title, "New EP");
        assert_eq!(upcoming[0].title, "Next Album");
        assert_eq!(updates.len(), 3);
        assert!(classify(Vec::new(), today).0.is_empty());
    }
}
