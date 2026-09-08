use crate::models::Track;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MetadataMatchStatus {
    HighMatch,
    MediumMatch,
    Unmatched,
}

impl MetadataMatchStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::HighMatch => "HIGH_MATCH",
            Self::MediumMatch => "MEDIUM_MATCH",
            Self::Unmatched => "UNMATCHED",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolutionOutcome {
    pub track: Track,
    pub status: MetadataMatchStatus,
    pub source: Option<String>,
    pub match_confidence: f32,
}

impl ResolutionOutcome {
    pub fn unmatched(track: Track) -> Self {
        Self {
            track,
            status: MetadataMatchStatus::Unmatched,
            source: None,
            match_confidence: 0.0,
        }
    }
}

#[async_trait]
pub trait MetadataResolver: Send + Sync {
    async fn resolve(&self, track: &Track) -> anyhow::Result<ResolutionOutcome>;
}
