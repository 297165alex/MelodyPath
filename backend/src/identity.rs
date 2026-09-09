use crate::{
    models::{Track, VersionType},
    normalize::{normalize_text, parse_track_version},
};
use std::collections::{BTreeSet, HashMap};

/// Centralized aliases only. Add reviewed aliases here instead of scattering
/// special cases through recommendation, transfer, or writer code.
const ARTIST_ALIAS_GROUPS: &[(&str, &[&str])] = &[
    (
        "jay chou",
        &["jay chou", "周杰倫", "周杰伦", "chou chieh lun"],
    ),
    (
        "jj lin",
        &["jj lin", "林俊傑", "林俊杰", "lin jun jie", "林俊傑 jj"],
    ),
];
const REVIEWED_ARTIST_NAMES: &[&str] = &[
    "taylor swift",
    "coldplay",
    "yoasobi",
    "bts",
    "newjeans",
    "iu",
    "jay chou",
    "周杰伦",
];

const TRACK_ID_KEYS: &[&str] = &["mbid", "isrc", "spotify", "youtube", "itunes"];
const ARTIST_ID_KEYS: &[&str] = &[
    "artist_mbid",
    "musicbrainz_artist_id",
    "spotify_artist_id",
    "youtube_channel_id",
    "lastfm_artist_mbid",
];

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ArtistIdentity {
    pub stable_id: Option<String>,
    pub canonical_name: String,
}

impl ArtistIdentity {
    pub fn from_name(name: &str) -> Self {
        Self {
            stable_id: None,
            canonical_name: canonical_artist_name(name),
        }
    }

    pub fn matches(&self, other: &Self) -> bool {
        match (&self.stable_id, &other.stable_id) {
            (Some(left), Some(right)) => left == right,
            _ => !self.canonical_name.is_empty() && self.canonical_name == other.canonical_name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackIdentity {
    pub stable_id: Option<String>,
    pub normalized_title: String,
    pub normalized_artists: BTreeSet<String>,
    pub version_type: VersionType,
}

impl TrackIdentity {
    pub fn from_track(track: &Track) -> Self {
        let parsed = parse_track_version(&track.title);
        Self {
            stable_id: stable_track_id(&track.external_ids),
            normalized_title: parsed.base_title,
            normalized_artists: artist_identity_keys(track),
            version_type: parsed.version_type,
        }
    }

    pub fn normalized_key(&self) -> String {
        if let Some(stable_id) = &self.stable_id {
            return format!("id:{stable_id}");
        }
        format!(
            "title:{}|artists:{}|version:{:?}",
            self.normalized_title,
            self.normalized_artists
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(";"),
            self.version_type
        )
    }

    pub fn same_recording(&self, other: &Self) -> bool {
        if let (Some(left), Some(right)) = (&self.stable_id, &other.stable_id) {
            if left == right {
                return true;
            }
            if id_namespace(left) == id_namespace(right) {
                return false;
            }
        }
        self.normalized_title == other.normalized_title
            && self.version_type == other.version_type
            && self
                .normalized_artists
                .iter()
                .any(|artist| other.normalized_artists.contains(artist))
    }
}

pub fn canonical_artist_name(name: &str) -> String {
    let normalized = normalize_text(name);
    for (canonical, aliases) in ARTIST_ALIAS_GROUPS {
        if aliases
            .iter()
            .any(|alias| normalize_text(alias) == normalized)
        {
            return (*canonical).to_string();
        }
    }
    normalized
}

pub fn is_known_artist_name(name: &str) -> bool {
    let normalized = normalize_text(name);
    REVIEWED_ARTIST_NAMES.contains(&normalized.as_str())
        || ARTIST_ALIAS_GROUPS.iter().any(|(canonical, aliases)| {
            normalize_text(canonical) == normalized
                || aliases
                    .iter()
                    .any(|alias| normalize_text(alias) == normalized)
        })
}

pub fn artist_identity_keys(track: &Track) -> BTreeSet<String> {
    let mut identities = BTreeSet::new();
    if let Some(stable) = stable_artist_id(&track.external_ids) {
        identities.insert(format!("id:{stable}"));
    }
    for credit in &track.artists {
        let canonical = canonical_artist_name(credit);
        if !canonical.is_empty() {
            identities.insert(format!("name:{canonical}"));
        }
        for component in split_artist_credit(credit) {
            let canonical = canonical_artist_name(component);
            if !canonical.is_empty() {
                identities.insert(format!("name:{canonical}"));
            }
        }
    }
    identities
}

pub fn primary_artist_key(track: &Track) -> String {
    stable_artist_id(&track.external_ids)
        .map(|id| format!("id:{id}"))
        .or_else(|| {
            track
                .artists
                .first()
                .map(|artist| format!("name:{}", canonical_artist_name(artist)))
        })
        .unwrap_or_default()
}

pub fn same_recording(left: &Track, right: &Track) -> bool {
    TrackIdentity::from_track(left).same_recording(&TrackIdentity::from_track(right))
}

pub fn normalized_track_key(track: &Track) -> String {
    TrackIdentity::from_track(track).normalized_key()
}

fn stable_track_id(ids: &HashMap<String, String>) -> Option<String> {
    stable_id(ids, TRACK_ID_KEYS)
}

fn stable_artist_id(ids: &HashMap<String, String>) -> Option<String> {
    stable_id(ids, ARTIST_ID_KEYS)
}

fn stable_id(ids: &HashMap<String, String>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        ids.get(*key)
            .map(|value| normalize_text(value))
            .filter(|value| !value.is_empty())
            .map(|value| format!("{key}:{value}"))
    })
}

fn id_namespace(value: &str) -> &str {
    value.split_once(':').map(|(key, _)| key).unwrap_or(value)
}

fn split_artist_credit(value: &str) -> impl Iterator<Item = &str> {
    value
        .split([';', ',', '&', '/'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalize::detect_version;
    use uuid::Uuid;

    fn track(title: &str, artist: &str) -> Track {
        Track {
            id: Uuid::new_v4().to_string(),
            title: title.into(),
            normalized_title: normalize_text(title),
            artists: vec![artist.into()],
            album: None,
            genres: Vec::new(),
            release_year: None,
            language: None,
            duration_ms: None,
            platform: "test".into(),
            platform_url: None,
            external_ids: HashMap::new(),
            version_type: detect_version(title),
            mood_tags: Vec::new(),
            energy_score: None,
            popularity: None,
            metadata_confidence: 1.0,
        }
    }

    #[test]
    fn aliases_share_one_central_identity() {
        assert_eq!(
            canonical_artist_name("Jay Chou"),
            canonical_artist_name("周杰倫")
        );
        assert_eq!(
            canonical_artist_name("周杰伦"),
            canonical_artist_name("Jay Chou")
        );
        assert_eq!(
            canonical_artist_name("JJ Lin"),
            canonical_artist_name("林俊傑")
        );
        assert_eq!(
            canonical_artist_name("林俊杰"),
            canonical_artist_name("JJ Lin")
        );
    }

    #[test]
    fn version_semantics_are_part_of_track_identity() {
        let original = track("Dancin", "Aaron Smith");
        let remix = track("Dancin - Krono Remix", "Aaron Smith");
        assert!(!same_recording(&original, &remix));
        assert_ne!(
            normalized_track_key(&original),
            normalized_track_key(&remix)
        );
    }

    #[test]
    fn aliases_match_the_same_recording() {
        assert!(same_recording(
            &track("晴天", "Jay Chou"),
            &track("晴天", "周杰倫")
        ));
        assert!(same_recording(
            &track("背對背擁抱", "JJ Lin"),
            &track("背對背擁抱", "林俊傑")
        ));
    }
}
