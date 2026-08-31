use crate::models::VersionType;

pub fn normalize_text(value: &str) -> String {
    let lowered = value.to_lowercase();
    let cleaned: String = lowered
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn detect_version(title: &str) -> VersionType {
    let text = normalize_text(title);
    if text.contains("live") {
        VersionType::Live
    } else if text.contains("remix") || text.contains("mix") {
        VersionType::Remix
    } else if text.contains("remaster") {
        VersionType::Remastered
    } else if text.contains("acoustic") || text.contains("unplugged") {
        VersionType::Acoustic
    } else if text.contains("cover") {
        VersionType::Cover
    } else if text.contains("instrumental") {
        VersionType::Instrumental
    } else {
        VersionType::Original
    }
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
    }

    #[test]
    fn similarity_is_deterministic() {
        assert_eq!(token_similarity("Plastic Love", "Plastic Love"), 1.0);
        assert!(token_similarity("Plastic Love", "Plastic Love Remastered") > 0.6);
    }
}
