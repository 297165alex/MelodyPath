use crate::models::VersionType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedTrackVersion {
    pub base_title: String,
    pub version_type: VersionType,
}

pub fn normalize_text(value: &str) -> String {
    let lowered = value.to_lowercase();
    let cleaned: String = lowered
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn detect_version(title: &str) -> VersionType {
    parse_track_version(title).version_type
}

pub fn parse_track_version(title: &str) -> ParsedTrackVersion {
    let normalized = normalize_text(title);
    let structured = structured_version_suffix(title);
    let detected = structured
        .as_ref()
        .and_then(|(_, suffix)| version_from_qualifier(&normalize_text(suffix)))
        .or_else(|| version_from_unstructured_title(&normalized))
        .unwrap_or(VersionType::Original);
    let base_title = if detected == VersionType::Original {
        normalized.clone()
    } else if let Some((prefix, _)) = structured {
        normalize_text(prefix)
    } else {
        strip_unstructured_version(&normalized)
    };
    ParsedTrackVersion {
        base_title: if base_title.is_empty() {
            normalized
        } else {
            base_title
        },
        version_type: detected,
    }
}

pub fn is_alternate_version(version: &VersionType) -> bool {
    !matches!(version, VersionType::Original | VersionType::Unknown)
}

fn structured_version_suffix(title: &str) -> Option<(&str, &str)> {
    let mut candidates = Vec::new();
    for separator in [" - ", " – ", " — ", " | "] {
        for (index, _) in title.match_indices(separator) {
            let suffix_start = index + separator.len();
            let suffix = &title[suffix_start..];
            if version_from_qualifier(&normalize_text(suffix)).is_some() {
                candidates.push((index, &title[..index], suffix));
            }
        }
    }
    for open in ['(', '[', '{'] {
        for (index, _) in title.match_indices(open) {
            let suffix = &title[index..];
            if version_from_qualifier(&normalize_text(suffix)).is_some() {
                candidates.push((index, &title[..index], suffix));
            }
        }
    }
    candidates
        .into_iter()
        .min_by_key(|(index, _, _)| *index)
        .map(|(_, prefix, suffix)| (prefix.trim(), suffix.trim()))
}

fn version_from_unstructured_title(normalized: &str) -> Option<VersionType> {
    let padded = format!(" {normalized} ");
    let contextual = [
        " live at ",
        " live from ",
        " live version ",
        " concert at ",
        " concert version ",
        " acoustic version ",
        " unplugged version ",
        " remix version ",
        " remastered ",
        " remaster ",
        " sped up ",
        " slowed down ",
        " radio edit ",
        " bonus track ",
    ];
    let terminal = [
        " live",
        " concert",
        " remix",
        " acoustic",
        " unplugged",
        " remastered",
        " remaster",
        " instrumental",
        " karaoke",
        " cover",
        " reaction",
        " nightcore",
        " slowed",
    ];
    if contextual.iter().any(|marker| padded.contains(marker))
        || terminal.iter().any(|marker| normalized.ends_with(marker))
    {
        version_from_qualifier(normalized)
    } else {
        None
    }
}

fn version_from_qualifier(normalized: &str) -> Option<VersionType> {
    let padded = format!(" {normalized} ");
    let contains = |phrase: &str| padded.contains(&format!(" {phrase} "));
    if contains("bonus track") {
        Some(VersionType::BonusTrack)
    } else if contains("radio edit") {
        Some(VersionType::RadioEdit)
    } else if contains("sped up") {
        Some(VersionType::SpedUp)
    } else if contains("slowed") || contains("slowed down") {
        Some(VersionType::Slowed)
    } else if contains("nightcore") {
        Some(VersionType::Nightcore)
    } else if contains("reaction") {
        Some(VersionType::Reaction)
    } else if contains("karaoke") {
        Some(VersionType::Karaoke)
    } else if contains("instrumental") {
        Some(VersionType::Instrumental)
    } else if contains("unplugged") {
        Some(VersionType::Unplugged)
    } else if contains("acoustic") {
        Some(VersionType::Acoustic)
    } else if contains("remastered") || contains("remaster") {
        Some(VersionType::Remastered)
    } else if contains("remix") {
        Some(VersionType::Remix)
    } else if contains("concert") {
        Some(VersionType::Concert)
    } else if contains("live") {
        Some(VersionType::Live)
    } else if contains("cover") {
        Some(VersionType::Cover)
    } else {
        None
    }
}

fn strip_unstructured_version(normalized: &str) -> String {
    let markers = [
        " bonus track",
        " radio edit",
        " sped up",
        " slowed down",
        " slowed",
        " nightcore",
        " reaction",
        " karaoke",
        " instrumental",
        " unplugged",
        " acoustic",
        " remastered",
        " remaster",
        " remix",
        " concert",
        " live",
        " cover",
    ];
    let end = markers
        .iter()
        .filter_map(|marker| normalized.find(marker))
        .min()
        .unwrap_or(normalized.len());
    normalized[..end].trim().to_string()
}

pub fn token_similarity(a: &str, b: &str) -> f32 {
    use std::collections::HashSet;
    let a_norm = normalize_text(a);
    let b_norm = normalize_text(b);
    if a_norm == b_norm {
        return 1.0;
    }
    let a_tokens: HashSet<_> = a_norm.split_whitespace().collect();
    let b_tokens: HashSet<_> = b_norm.split_whitespace().collect();
    if a_tokens.is_empty() || b_tokens.is_empty() {
        return 0.0;
    }
    let intersection = a_tokens.intersection(&b_tokens).count() as f32;
    let union = a_tokens.union(&b_tokens).count() as f32;
    intersection / union
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_punctuation_and_case() {
        assert_eq!(normalize_text("BIBI — Kazino!"), "bibi kazino");
    }

    #[test]
    fn detects_recording_versions() {
        assert_eq!(
            detect_version("Gravity (Live at Wembley)"),
            VersionType::Live
        );
        assert_eq!(detect_version("Stay - Acoustic"), VersionType::Acoustic);
        assert_eq!(detect_version("Hello"), VersionType::Original);
        assert_eq!(detect_version("Dancin - Krono Remix"), VersionType::Remix);
        assert_eq!(
            detect_version("Bang Bang (Bonus Track)"),
            VersionType::BonusTrack
        );
        assert_eq!(detect_version("Midnight - Sped Up"), VersionType::SpedUp);
        assert_eq!(detect_version("Live Forever"), VersionType::Original);
        assert_eq!(detect_version("Remix to Ignition"), VersionType::Original);
        assert_eq!(
            parse_track_version("Dancin - Krono Remix").base_title,
            "dancin"
        );
        assert_eq!(
            parse_track_version("Bang Bang (Bonus Track)").base_title,
            "bang bang"
        );
    }

    #[test]
    fn similarity_is_deterministic() {
        assert_eq!(token_similarity("Plastic Love", "Plastic Love"), 1.0);
        assert!(token_similarity("Plastic Love", "Plastic Love Remastered") > 0.6);
    }
}
