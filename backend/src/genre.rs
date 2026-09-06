use crate::normalize::normalize_text;
use std::collections::HashSet;

pub fn canonicalize_genre(raw: &str) -> String {
    let key = normalize_text(raw.trim());
    let canonical = match key.as_str() {
        "pop" => "Pop",
        "bedroom pop" => "Bedroom Pop",
        "indie pop" => "Indie Pop",
        "soft pop" => "Soft Pop",
        "art pop" => "Art Pop",
        "acoustic pop" => "Acoustic Pop",
        "dance pop" => "Dance Pop",
        "electronic pop" => "Electronic Pop",
        "pop rock" => "Pop Rock",
        "synth pop" | "synthpop" => "Synthpop",
        "r b" | "r b soul" | "rhythm and blues" | "soul" => "R&B",
        "alternative r b" => "Alternative R&B",
        "korean r b" => "Korean R&B",
        "japanese r b" => "Japanese R&B",
        "indie r b" => "Indie R&B",
        "jazz r b" => "Jazz R&B",
        "neo soul" => "Neo Soul",
        "psychedelic soul" => "Psychedelic Soul",
        "hip hop" | "hiphop" | "hip hop rap" | "rap" => "Hip-Hop",
        "alternative hip hop" => "Alternative Hip-Hop",
        "jazz rap" => "Jazz Rap",
        "boom bap" => "Boom Bap",
        "k pop" => "K-Pop",
        "j pop" => "J-Pop",
        "c pop" => "C-Pop",
        "mandopop" => "Mandopop",
        "cantopop" => "Cantopop",
        "taiwanese pop" => "Taiwanese Pop",
        "malay pop" => "Malay Pop",
        "rock" => "Rock",
        "alternative rock" => "Alternative Rock",
        "indie rock" => "Indie Rock",
        "chinese rock" => "Chinese Rock",
        "folk rock" => "Folk Rock",
        "alternative" | "alternative indie" => "Alternative / Indie",
        "chinese indie" => "Chinese Indie",
        "dream pop" => "Dream Pop",
        "shoegaze" => "Shoegaze",
        "electronic" => "Electronic",
        "progressive house" => "Progressive House",
        "ambient" => "Ambient",
        "jazz" => "Jazz",
        "jazz pop" => "Jazz Pop",
        "modal jazz" => "Modal Jazz",
        "jazz funk" => "Jazz-Funk",
        "bossa nova" => "Bossa Nova",
        "classical" => "Classical",
        "modern classical" => "Modern Classical",
        "impressionism" => "Impressionism",
        "soundtrack" => "Soundtrack",
        "singer songwriter" => "Singer/Songwriter",
        "folk" => "Folk",
        "city pop" => "City Pop",
        "ballad" => "Ballad",
        "korean ballad" | "k ballad" => "Korean Ballad",
        "j pop ballad" | "japanese ballad" => "J-Pop Ballad",
        "mandopop ballad" | "chinese ballad" => "Mandopop Ballad",
        "christmas" => "Christmas",
        "emo" => "Emo",
        "other" | "其他" => "其他",
        "metadata pending" | "待补全" | "元数据待补全" => "元数据待补全",
        _ => return title_case_unknown(&key),
    };
    canonical.to_string()
}

pub fn normalize_genres<I, S>(genres: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = HashSet::new();
    genres
        .into_iter()
        .map(|genre| canonicalize_genre(genre.as_ref()))
        .filter(|genre| !genre.is_empty())
        .filter(|genre| seen.insert(genre_key(genre)))
        .collect()
}

pub fn genre_key(raw: &str) -> String {
    normalize_text(&canonicalize_genre(raw))
}

pub fn is_unknown_genre(raw: &str) -> bool {
    matches!(genre_key(raw).as_str(), "" | "待补全" | "元数据待补全")
}

pub fn genre_distance(left: &str, right: &str) -> u8 {
    let left = canonicalize_genre(left);
    let right = canonicalize_genre(right);
    let left_key = genre_key(&left);
    let right_key = genre_key(&right);
    if left_key == right_key {
        return 0;
    }
    if parent_key(&left_key).as_deref() == Some(right_key.as_str())
        || parent_key(&right_key).as_deref() == Some(left_key.as_str())
    {
        return 1;
    }
    if parent_key(&left_key).is_some() && parent_key(&left_key) == parent_key(&right_key) {
        return 2;
    }
    if explicit_neighbors(&left)
        .iter()
        .any(|genre| genre_key(genre) == right_key)
        || explicit_neighbors(&right)
            .iter()
            .any(|genre| genre_key(genre) == left_key)
    {
        return 2;
    }
    3
}

pub fn adjacent_genres(core: &[String]) -> Vec<String> {
    let core_keys: HashSet<_> = core.iter().map(|genre| genre_key(genre)).collect();
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for genre in core {
        for adjacent in explicit_neighbors(&canonicalize_genre(genre)) {
            let key = genre_key(adjacent);
            if !core_keys.contains(&key) && seen.insert(key) {
                result.push((*adjacent).to_string());
            }
        }
    }
    result.into_iter().take(4).collect()
}

pub fn exploration_genres(core: &[String], adjacent: &[String]) -> Vec<String> {
    let blocked: HashSet<_> = core
        .iter()
        .chain(adjacent.iter())
        .map(|genre| genre_key(genre))
        .collect();
    let mut seen = HashSet::new();
    let mut result = Vec::new();
    for genre in adjacent.iter().chain(core.iter()) {
        for candidate in explicit_neighbors(&canonicalize_genre(genre)) {
            let key = genre_key(candidate);
            if !blocked.contains(&key) && seen.insert(key) {
                result.push((*candidate).to_string());
            }
        }
    }
    result.into_iter().take(4).collect()
}

fn parent_key(key: &str) -> Option<String> {
    let parent = match key {
        "bedroom pop" | "soft pop" | "art pop" | "acoustic pop" | "dance pop"
        | "electronic pop" | "k pop" | "j pop" | "c pop" | "mandopop" | "cantopop"
        | "taiwanese pop" | "malay pop" => "pop",
        "alternative r b" | "korean r b" | "japanese r b" | "indie r b" | "jazz r b"
        | "neo soul" | "psychedelic soul" => "r b",
        "alternative rock" | "indie rock" | "chinese rock" | "folk rock" | "pop rock" => "rock",
        "alternative hip hop" | "jazz rap" | "boom bap" => "hip hop",
        "jazz pop" | "modal jazz" | "jazz funk" | "bossa nova" => "jazz",
        "modern classical" | "impressionism" | "soundtrack" => "classical",
        "progressive house" | "ambient" | "synthpop" => "electronic",
        "korean ballad" | "j pop ballad" | "mandopop ballad" => "ballad",
        "indie pop" | "dream pop" | "shoegaze" | "chinese indie" => "alternative indie",
        _ => return None,
    };
    Some(parent.to_string())
}

fn explicit_neighbors(genre: &str) -> &'static [&'static str] {
    match genre_key(genre).as_str() {
        "pop" => &["Indie Pop", "Synthpop", "Singer/Songwriter", "R&B"],
        "bedroom pop" | "soft pop" | "art pop" | "acoustic pop" | "dance pop" => {
            &["Indie Pop", "Synthpop", "R&B"]
        }
        "indie pop" => &["Pop", "Dream Pop", "Alternative / Indie"],
        "synthpop" => &["Pop", "Electronic", "Dream Pop"],
        "r b" => &["Alternative R&B", "Neo Soul", "Jazz R&B", "Pop"],
        "alternative r b" => &["Korean R&B", "Neo Soul", "Dream Pop", "R&B"],
        "korean r b" => &["Alternative R&B", "K-Pop", "Neo Soul"],
        "japanese r b" => &["J-Pop", "Neo Soul", "City Pop"],
        "neo soul" => &["R&B", "Jazz", "Jazz R&B", "Alternative Hip-Hop"],
        "k pop" => &["Korean R&B", "Dance Pop", "Synthpop"],
        "j pop" | "city pop" => &["Japanese R&B", "Dream Pop", "Synthpop"],
        "c pop" | "mandopop" | "cantopop" | "taiwanese pop" => {
            &["Chinese Indie", "R&B", "Singer/Songwriter"]
        }
        "rock" | "alternative rock" | "indie rock" | "chinese rock" => {
            &["Alternative / Indie", "Dream Pop", "Folk Rock"]
        }
        "alternative indie" | "chinese indie" => &["Indie Pop", "Dream Pop", "Folk Rock"],
        "hip hop" | "alternative hip hop" => &["Jazz Rap", "R&B", "Neo Soul"],
        "electronic" | "progressive house" => &["Synthpop", "Dream Pop", "Ambient"],
        "jazz" | "jazz pop" | "modal jazz" => &["Bossa Nova", "Neo Soul", "Classical"],
        "classical" | "modern classical" | "impressionism" => &["Soundtrack", "Ambient", "Jazz"],
        "singer songwriter" | "folk" => &["Folk Rock", "Indie Pop", "Pop"],
        "ballad" => &["Singer/Songwriter", "R&B", "Neo Soul"],
        "korean ballad" => &["Korean R&B", "Ballad"],
        "j pop ballad" => &["Japanese R&B", "Singer/Songwriter", "City Pop"],
        "mandopop ballad" => &["Mandopop", "R&B", "Singer/Songwriter"],
        _ => &[],
    }
}

fn title_case_unknown(key: &str) -> String {
    key.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(first) if first.is_ascii_alphabetic() => {
                    format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
                }
                Some(first) => format!("{first}{}", chars.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_case_aliases_and_separators() {
        assert_eq!(canonicalize_genre(" pop "), "Pop");
        assert_eq!(canonicalize_genre("POP"), "Pop");
        assert_eq!(canonicalize_genre("bedroom pop"), "Bedroom Pop");
        assert_eq!(canonicalize_genre("Bedroom Pop"), "Bedroom Pop");
        assert_eq!(canonicalize_genre("indie-pop"), "Indie Pop");
        assert_eq!(canonicalize_genre("r&b"), "R&B");
        assert_eq!(canonicalize_genre("alternative r&b"), "Alternative R&B");
        assert_eq!(canonicalize_genre("korean r&b"), "Korean R&B");
        assert_eq!(canonicalize_genre("hip-hop"), "Hip-Hop");
        assert_eq!(canonicalize_genre("c-pop"), "C-Pop");
    }

    #[test]
    fn korean_and_alternative_rnb_are_adjacent() {
        assert_eq!(genre_distance("Korean R&B", "Alternative R&B"), 2);
        assert!(
            adjacent_genres(&["korean r&b".into()])
                .iter()
                .any(|genre| genre == "Alternative R&B")
        );
    }

    #[test]
    fn ballad_family_has_explainable_two_hop_bridge() {
        assert_eq!(canonicalize_genre("k-ballad"), "Korean Ballad");
        let adjacent = adjacent_genres(&["Korean Ballad".into()]);
        assert!(adjacent.contains(&"Korean R&B".into()));
        let exploration = exploration_genres(&["Korean Ballad".into()], &adjacent);
        assert!(
            exploration
                .iter()
                .any(|genre| genre == "Alternative R&B" || genre == "Neo Soul")
        );
    }

    #[test]
    fn exploration_never_invents_global_random_fallbacks() {
        assert!(exploration_genres(&["Unmapped Scene".into()], &[]).is_empty());
    }
}
