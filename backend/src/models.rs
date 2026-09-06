use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DataState {
    None,
    RealFile,
    RealText,
    RealAccount,
    RealPublicLink,
    Demo,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MetadataStatus {
    Complete,
    Partial,
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedTrack {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub release_date: Option<String>,
    #[serde(default)]
    pub genres: Vec<String>,
    pub duration_ms: Option<u32>,
    pub energy_score: Option<f32>,
    pub source: String,
    pub original_row: String,
    pub metadata_status: MetadataStatus,
    pub metadata_confidence: f32,
    #[serde(default)]
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreviewRequest {
    pub name: Option<String>,
    pub file_name: Option<String>,
    pub format: String,
    pub content: String,
    pub data_state: DataState,
    pub text_order: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportPreview {
    pub id: String,
    pub name: String,
    pub file_name: Option<String>,
    pub data_state: DataState,
    pub source_label: String,
    pub total_rows: usize,
    pub parsed_count: usize,
    pub warning_count: usize,
    pub invalid_count: usize,
    pub detected_fields: Vec<String>,
    pub preview_tracks: Vec<ImportedTrack>,
    pub requires_column_confirmation: bool,
    pub text_order: String,
    pub questions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportAnalysisSummary {
    pub data_state: DataState,
    pub source_label: String,
    pub input_count: usize,
    pub parsed_count: usize,
    pub analyzed_count: usize,
    pub complete_metadata_count: usize,
    pub partial_metadata_count: usize,
    pub unmatched_count: usize,
    pub genre_matched_count: usize,
    pub genre_coverage: f32,
    pub energy_matched_count: usize,
    pub energy_coverage: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum VersionType {
    Original,
    Live,
    Concert,
    Remix,
    Remastered,
    Acoustic,
    Unplugged,
    BonusTrack,
    SpedUp,
    Slowed,
    RadioEdit,
    Cover,
    Instrumental,
    Karaoke,
    Reaction,
    Nightcore,
    Other,
    Unknown,
}

impl Default for VersionType {
    fn default() -> Self {
        Self::Original
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub id: String,
    pub title: String,
    pub normalized_title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub genres: Vec<String>,
    pub release_year: Option<u16>,
    pub language: Option<String>,
    pub duration_ms: Option<u32>,
    pub platform: String,
    pub platform_url: Option<String>,
    #[serde(default)]
    pub external_ids: HashMap<String, String>,
    #[serde(default)]
    pub version_type: VersionType,
    #[serde(default)]
    pub mood_tags: Vec<String>,
    pub energy_score: Option<f32>,
    pub popularity: Option<f32>,
    pub metadata_confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: String,
    pub name: String,
    pub owner_label: String,
    pub source: String,
    pub is_demo: bool,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TasteMetric {
    pub label: String,
    pub value: f32,
    pub display: String,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub track: Track,
    pub zone: String,
    pub reason: String,
    pub connection: String,
    pub expansion: String,
    pub match_score: f32,
    pub novelty_score: f32,
    pub candidate_source: String,
    pub match_confidence: f32,
    pub source_endpoint: String,
    pub seed_track: Option<String>,
    pub seed_artist: Option<String>,
    pub lastfm_similarity: Option<f32>,
    pub tags: Vec<String>,
    pub relaxation_level: u8,
    pub already_in_source_playlist: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecommendationSeed {
    pub title: String,
    pub artists: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecommendationQueryStats {
    pub successful_seed_count: usize,
    pub failed_seed_count: usize,
    pub raw_track_similar_count: usize,
    pub raw_artist_similar_count: usize,
    pub raw_artist_top_tracks_count: usize,
    pub raw_tag_top_tracks_count: usize,
    pub raw_candidate_count: usize,
    pub after_version_filter_count: usize,
    pub after_normalization_count: usize,
    pub after_deduplication_count: usize,
    pub after_source_exclusion_count: usize,
    pub after_artist_cap_count: usize,
    pub comfort_candidate_count: usize,
    pub expansion_candidate_count: usize,
    pub surprise_candidate_count: usize,
    pub tag_layer1_candidate_count: usize,
    pub tag_layer1_rejected_count: usize,
    pub tag_layer2_candidate_count: usize,
    pub tag_layer2_rejected_count: usize,
    #[serde(default)]
    pub core_tags: Vec<String>,
    #[serde(default)]
    pub tag_similar_success_count: usize,
    #[serde(default)]
    pub tag_similar_failure_count: usize,
    #[serde(default)]
    pub similar_tag_count: usize,
    #[serde(default)]
    pub layer1_tags: Vec<String>,
    #[serde(default)]
    pub layer2_tags: Vec<String>,
    #[serde(default)]
    pub tag_top_track_counts: Vec<TagCandidateTelemetry>,
    #[serde(default)]
    pub request_budget_exhausted_count: usize,
    #[serde(default)]
    pub request_budget_used_count: usize,
    #[serde(default)]
    pub retry_count: usize,
    #[serde(default)]
    pub genre_bridge_candidate_count: usize,
    #[serde(default)]
    pub second_hop_artist_candidate_count: usize,
    /// Backward-compatible alias for `after_source_exclusion_count`.
    pub deduplicated_candidate_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TagCandidateTelemetry {
    pub tag: String,
    pub layer: u8,
    pub candidate_count: usize,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationZoneSummary {
    pub zone: String,
    pub count: usize,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecommendationSummary {
    pub source_label: String,
    pub status: String,
    pub message: String,
    pub candidate_count: usize,
    pub zones: Vec<RecommendationZoneSummary>,
    #[serde(default)]
    pub seeds: Vec<RecommendationSeed>,
    #[serde(default)]
    pub query_stats: RecommendationQueryStats,
    #[serde(default)]
    pub comfort_pool: Vec<Recommendation>,
    #[serde(default)]
    pub expansion_pool: Vec<Recommendation>,
    #[serde(default)]
    pub surprise_pool: Vec<Recommendation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStep {
    pub genre: String,
    pub explanation: String,
    pub tracks: Vec<Track>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TasteReport {
    pub playlist_name: String,
    pub source_label: String,
    pub is_demo: bool,
    pub track_count: usize,
    pub genre_distribution: Vec<(String, usize)>,
    pub artist_distribution: Vec<(String, usize)>,
    pub era_distribution: Vec<(String, usize)>,
    pub album_distribution: Vec<(String, usize)>,
    pub duplicate_track_count: usize,
    pub collaboration_track_count: usize,
    pub genre_matched_count: usize,
    pub genre_coverage: f32,
    pub energy_matched_count: usize,
    pub energy_coverage: f32,
    pub metrics: Vec<TasteMetric>,
    pub core_preferences: Vec<String>,
    pub adjacent_preferences: Vec<String>,
    pub unexplored_preferences: Vec<String>,
    pub summary: String,
    pub confidence: f32,
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalDemo {
    #[serde(default)]
    pub analysis_id: String,
    pub playlist: Playlist,
    pub report: TasteReport,
    pub recommendations: Vec<Recommendation>,
    pub route: Vec<RouteStep>,
    pub recommendation_summary: RecommendationSummary,
    #[serde(default)]
    pub import_summary: Option<ImportAnalysisSummary>,
    #[serde(default)]
    pub unmatched_tracks: Vec<ImportedTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeTrack {
    pub track: Track,
    pub reason: String,
    #[serde(default)]
    pub reason_for_a: String,
    #[serde(default)]
    pub reason_for_b: String,
    #[serde(default)]
    pub shared_basis: Vec<String>,
    #[serde(default)]
    pub candidate_source: String,
    pub phase: String,
    pub bridge_score: f32,
    #[serde(default)]
    pub already_in_a: bool,
    #[serde(default)]
    pub already_in_b: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub user_a: String,
    pub user_b: String,
    pub metrics: Vec<TasteMetric>,
    pub track_count_a: usize,
    pub track_count_b: usize,
    pub shared_track_count: usize,
    pub shared_artists: Vec<String>,
    pub shared_genres: Vec<String>,
    pub user_a_signatures: Vec<String>,
    pub user_b_signatures: Vec<String>,
    pub summary: String,
    pub bridge_playlist: Vec<BridgeTrack>,
    pub is_demo: bool,
    pub data_source: String,
    pub saved_locally: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareRequest {
    pub analysis_a: PersonalDemo,
    pub analysis_b: PersonalDemo,
    #[serde(default)]
    pub save_locally: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternateVersionSearchRequest {
    pub track: Track,
    #[serde(default)]
    pub version_types: Vec<VersionType>,
    #[serde(default)]
    pub use_mock: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternateVersionCandidate {
    pub title: String,
    pub artists: Vec<String>,
    pub platform: String,
    pub version_type: VersionType,
    pub duration_ms: Option<u32>,
    pub official_status: String,
    pub match_confidence: f32,
    pub source_url: String,
    pub reason: String,
    pub is_alternate_version: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlternateVersionSearchResult {
    pub source_track: Track,
    pub provider: String,
    pub status: String,
    pub message: String,
    pub candidates: Vec<AlternateVersionCandidate>,
    pub is_mock: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransferTrack {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub isrc: Option<String>,
    pub source_platform: String,
    pub source_track_id: String,
    pub source_url: Option<String>,
    pub normalized_title: String,
    pub normalized_artists: Vec<String>,
    pub version_type: VersionType,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransferCandidate {
    pub target_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: Option<u32>,
    pub source_url: Option<String>,
    pub channel_name: Option<String>,
    pub official_status: String,
    pub version_type: VersionType,
    pub title_score: f32,
    pub artist_score: f32,
    pub duration_score: Option<f32>,
    pub version_score: f32,
    pub score: f32,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransferMatchStatus {
    MatchedHigh,
    MatchedAmbiguous,
    Unmatched,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferMatch {
    pub source_track: TransferTrack,
    pub candidates: Vec<TransferCandidate>,
    pub selected_target_id: Option<String>,
    pub status: TransferMatchStatus,
    pub score: f32,
    pub reason: String,
    pub requires_confirmation: bool,
    pub alternate_version_fallback: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferPreviewRequest {
    pub playlist_name: String,
    pub tracks: Vec<Track>,
    #[serde(default = "default_transfer_destination")]
    pub destination_platform: String,
    #[serde(default)]
    pub allow_alternate_versions: bool,
    #[serde(default)]
    pub use_mock: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferPreview {
    pub preview_id: String,
    pub playlist_name: String,
    pub source_count: usize,
    pub high_confidence_count: usize,
    pub ambiguous_count: usize,
    pub unmatched_count: usize,
    pub alternate_fallback_count: usize,
    pub matches: Vec<TransferMatch>,
    pub provider: String,
    pub destination_platform: String,
    pub status: String,
    pub message: String,
    pub requires_explicit_confirmation: bool,
    pub source_was_modified: bool,
    pub is_mock: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferExecuteRequest {
    pub preview_id: String,
    pub confirmed: bool,
    #[serde(default)]
    pub selections: HashMap<String, String>,
    #[serde(default = "default_private_visibility")]
    pub privacy: String,
}

fn default_private_visibility() -> String {
    "private".into()
}

fn default_transfer_destination() -> String {
    "youtube".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferTrackResult {
    pub source_track: TransferTrack,
    pub target_id: Option<String>,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferResult {
    pub run_id: String,
    pub preview_id: String,
    pub status: String,
    pub source_count: usize,
    pub matched_count: usize,
    pub written_count: usize,
    pub failed_count: usize,
    pub skipped_count: usize,
    pub unmatched_count: usize,
    pub progress: f32,
    pub playlist_id: Option<String>,
    pub playlist_url: Option<String>,
    pub report_csv_url: Option<String>,
    pub report_json_url: Option<String>,
    pub results: Vec<TransferTrackResult>,
    pub source_was_modified: bool,
    pub is_mock: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransferRun {
    pub id: String,
    pub preview_id: String,
    pub destination_platform: String,
    pub status: String,
    pub processed_count: usize,
    pub source_count: usize,
    pub progress: f32,
    pub result: Option<TransferResult>,
    pub error: Option<String>,
    pub is_mock: bool,
    pub revision: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DemoPayload {
    pub generated_at: String,
    pub disclosure: String,
    pub personal: PersonalDemo,
    pub comparison: ComparisonReport,
    pub available_playlists: Vec<Playlist>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum MatchStatus {
    Matched,
    NeedsConfirmation,
    Unmatched,
    Selected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchCandidate {
    pub target_track_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub channel_name: Option<String>,
    pub official_status: String,
    pub target_url: Option<String>,
    pub version_type: VersionType,
    pub confidence: f32,
    pub match_reason: String,
    pub available_in_market: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformTrackMatch {
    pub source_track: Track,
    pub target_platform: String,
    pub target_track_id: Option<String>,
    pub target_url: Option<String>,
    pub version_type: VersionType,
    pub confidence: f32,
    pub match_reason: String,
    pub status: MatchStatus,
    #[serde(default)]
    pub candidates: Vec<MatchCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackFailure {
    pub track: Track,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistExportResult {
    pub playlist_name: String,
    pub platform: String,
    pub playlist_url: Option<String>,
    pub requested_count: usize,
    pub added_count: usize,
    pub failed_count: usize,
    pub needs_confirmation_count: usize,
    pub successful_tracks: Vec<PlatformTrackMatch>,
    pub failed_tracks: Vec<TrackFailure>,
    pub ambiguous_tracks: Vec<PlatformTrackMatch>,
    pub is_demo: bool,
    pub disclosure: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WriterStatus {
    pub platform: String,
    pub label: String,
    pub availability: String,
    pub authorized: bool,
    pub is_demo: bool,
    pub message: String,
}

/// 服务端生成的平台能力说明。前端只展示该接口返回的事实，不自行宣称已接通。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformCapability {
    pub platform: String,
    pub auth_supported: bool,
    pub playlist_read_supported: bool,
    pub playlist_write_supported: bool,
    pub public_link_import_supported: bool,
    pub file_import_supported: bool,
    pub compare_supported: bool,
    pub transfer_source_supported: bool,
    pub transfer_destination_supported: bool,
    pub copy_source_supported: bool,
    pub copy_destination_supported: bool,
    pub playlist_read_for_copy: bool,
    pub playlist_read_for_compare: bool,
    pub playlist_read_for_recommendation: bool,
    pub alternate_version_search_supported: bool,
    pub status: String,
    pub reason: String,
    pub display_name: String,
    pub region: String,
    pub capability_status: String,
    pub status_label: String,
    pub account_connection: String,
    pub public_playlist_links: String,
    pub playlist_read: String,
    pub playlist_write: String,
    pub search_links: bool,
    pub requires_review: bool,
    pub configured: bool,
    pub official_docs_url: Option<String>,
    pub action_kind: String,
    pub description: String,
    pub policy_notice: Option<String>,
    pub data_use: DataUseCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataUseCapabilities {
    pub can_read_account_identity: bool,
    pub can_list_playlists: bool,
    pub can_read_playlist_items: bool,
    pub can_display_attributed_metadata: bool,
    pub can_transfer_playlist_metadata: bool,
    pub can_create_playlist: bool,
    pub can_add_items: bool,
    pub can_analyze_content: bool,
    pub can_derive_metrics: bool,
    pub can_cross_platform_compare: bool,
    pub can_send_to_llm: bool,
    pub can_train_model: bool,
    pub explanation: String,
}

impl DataUseCapabilities {
    pub fn unavailable(explanation: impl Into<String>) -> Self {
        Self {
            can_read_account_identity: false,
            can_list_playlists: false,
            can_read_playlist_items: false,
            can_display_attributed_metadata: false,
            can_transfer_playlist_metadata: false,
            can_create_playlist: false,
            can_add_items: false,
            can_analyze_content: false,
            can_derive_metrics: false,
            can_cross_platform_compare: false,
            can_send_to_llm: false,
            can_train_model: false,
            explanation: explanation.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfigurationStatus {
    pub platform: String,
    pub display_name: String,
    pub configured: bool,
    pub validation_status: String,
    pub required_environment_variables: Vec<String>,
    pub present_environment_variables: Vec<String>,
    pub missing_environment_variables: Vec<String>,
    pub redirect_uri: Option<String>,
    pub dashboard_url: String,
    pub setup_steps: Vec<String>,
    pub secrets_exposed_to_frontend: bool,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlaylistLinkRequest {
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlaylistLinkInspection {
    pub capability: PublicLinkCapability,
    pub url_valid: bool,
    pub playlist_id_valid: bool,
    pub platform: Option<String>,
    pub platform_label: Option<String>,
    pub recognized: bool,
    pub playlist_id: Option<String>,
    pub normalized_url: Option<String>,
    pub resolved_url: Option<String>,
    pub publicly_accessible: Option<bool>,
    pub access_status: String,
    pub structured_data_status: String,
    pub playlist_name: Option<String>,
    pub track_count: Option<usize>,
    pub preview_tracks: Vec<Track>,
    pub can_analyze: bool,
    pub message: String,
    pub next_step: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PublicLinkCapability {
    Unsupported,
    UrlRecognitionOnly,
    AccessibilityCheckOnly,
    PublicMetadataAvailable,
    TrackImportAvailable,
    AuthRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyConnectionStatus {
    pub write_authorized: bool,
    pub configured: bool,
    pub connected: bool,
    pub user_id: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub message: String,
    pub policy_notice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyPlaylistSummary {
    pub id: String,
    pub name: String,
    pub owner_name: String,
    pub track_count: usize,
    pub collaborative: bool,
    pub public: Option<bool>,
    pub spotify_url: Option<String>,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SpotifyImportRequest {
    pub playlist_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyImportedPlaylist {
    pub id: String,
    pub name: String,
    pub spotify_url: Option<String>,
    pub imported_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotifyImportResult {
    pub playlists: Vec<SpotifyImportedPlaylist>,
    pub tracks: Vec<Track>,
    pub track_count: usize,
    pub data_use: DataUseCapabilities,
    pub policy_notice: String,
    pub attribution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoutubeConnectionStatus {
    pub write_authorized: bool,
    pub configured: bool,
    pub connected: bool,
    pub channel_id: Option<String>,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub message: String,
    pub policy_notice: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoutubePlaylistSummary {
    pub id: String,
    pub name: String,
    pub owner_name: String,
    pub item_count: usize,
    pub privacy_status: Option<String>,
    pub youtube_url: String,
    pub image_url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct YoutubeImportRequest {
    pub playlist_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoutubeImportedPlaylist {
    pub id: String,
    pub name: String,
    pub youtube_url: String,
    pub imported_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YoutubeImportResult {
    pub playlists: Vec<YoutubeImportedPlaylist>,
    pub tracks: Vec<Track>,
    pub track_count: usize,
    pub data_use: DataUseCapabilities,
    pub policy_notice: String,
    pub attribution: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppleMusicBootstrap {
    pub configured: bool,
    pub developer_token: Option<String>,
    pub app_name: String,
    pub app_build: String,
    pub real_account_validation: String,
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ManualAnalyzeRequest {
    pub name: Option<String>,
    pub text: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportPreviewRequest {
    pub platform: String,
    pub playlist_name: String,
    pub tracks: Vec<Track>,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportPreview {
    pub preview_id: String,
    pub platform: String,
    pub playlist_name: String,
    pub matches: Vec<PlatformTrackMatch>,
    pub requested_count: usize,
    pub auto_matched_count: usize,
    pub needs_confirmation_count: usize,
    pub unmatched_count: usize,
    pub requires_explicit_confirmation: bool,
    pub is_demo: bool,
    pub disclosure: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExportExecuteRequest {
    pub preview_id: String,
    pub confirmed: bool,
    #[serde(default)]
    pub selections: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct StoredPreview {
    pub preview: ExportPreview,
    pub format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    pub endpoint: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub max_agent_steps: u32,
    pub request_timeout_seconds: u64,
    pub retry_limit: u32,
    pub max_cost_usd: f64,
    pub input_price_per_million: f64,
    pub output_price_per_million: f64,
    #[serde(default, skip_deserializing)]
    pub api_key_available: bool,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            endpoint: "https://api.openai.com/v1".into(),
            model: "gpt-5-mini".into(),
            temperature: 0.3,
            max_tokens: 1_200,
            max_agent_steps: 16,
            request_timeout_seconds: 30,
            retry_limit: 2,
            max_cost_usd: 0.50,
            input_price_per_million: 0.0,
            output_price_per_million: 0.0,
            api_key_available: std::env::var("OPENAI_API_KEY").is_ok(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateAgentTaskRequest {
    pub scenario: String,
    pub goal: Option<String>,
    #[serde(default)]
    pub analysis_id: Option<String>,
    #[serde(default)]
    pub analysis_a_id: Option<String>,
    #[serde(default)]
    pub analysis_b_id: Option<String>,
    #[serde(default)]
    pub transfer_preview_id: Option<String>,
    #[serde(default)]
    pub use_demo: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentDecision {
    pub action: String,
    pub next_tool: Option<String>,
    #[serde(default)]
    pub arguments: serde_json::Value,
    pub reason: String,
    pub finish: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentRunStatus {
    Planning,
    Running,
    WaitingUserConfirmation,
    BlockedExternalAuth,
    Completed,
    Failed,
    Cancelled,
}

impl AgentRunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Planning => "PLANNING",
            Self::Running => "RUNNING",
            Self::WaitingUserConfirmation => "WAITING_USER_CONFIRMATION",
            Self::BlockedExternalAuth => "BLOCKED_EXTERNAL_AUTH",
            Self::Completed => "COMPLETED",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentIntent {
    pub action: String,
    pub count: Option<usize>,
    pub novelty: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentGoal {
    pub user_goal: String,
    pub normalized_intent: AgentIntent,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AgentStepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentStep {
    pub step_id: String,
    pub label: String,
    pub tool: String,
    pub status: AgentStepStatus,
    pub attempts: u32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentPlan {
    pub scenario: String,
    pub steps: Vec<AgentStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AgentToolCall {
    pub call_id: String,
    pub step_id: String,
    pub tool: String,
    pub attempt: u32,
    pub started_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentToolResult {
    pub call_id: String,
    pub success: bool,
    pub recoverable: bool,
    pub summary: String,
    pub output: serde_json::Value,
    pub completed_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentState {
    pub status: AgentRunStatus,
    pub current_step: usize,
    pub completed_steps: Vec<String>,
    pub pending_steps: Vec<String>,
    pub retries: u32,
    pub progress: f32,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentRun {
    pub run_id: String,
    pub goal: AgentGoal,
    pub plan: AgentPlan,
    pub state: AgentState,
    pub tool_calls: Vec<AgentToolCall>,
    pub tool_results: Vec<AgentToolResult>,
    pub decision_mode: String,
    pub decisions: Vec<AgentDecision>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub estimated_cost_usd: f64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AgentTask {
    pub id: String,
    pub scenario: String,
    pub goal: String,
    pub status: String,
    pub current_step: i64,
    pub max_steps: i64,
    pub progress: f32,
    pub message: String,
    pub analysis_id: Option<String>,
    pub analysis_a_id: Option<String>,
    pub analysis_b_id: Option<String>,
    pub transfer_preview_id: Option<String>,
    pub use_demo: bool,
    pub data_state: String,
    pub decision_mode: String,
    pub decisions_json: String,
    pub normalized_intent_json: String,
    pub plan_json: String,
    pub completed_steps_json: String,
    pub pending_steps_json: String,
    pub tool_calls_json: String,
    pub tool_results_json: String,
    pub retries: i64,
    pub warnings_json: String,
    pub checkpoint_json: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub estimated_cost_usd: f64,
    pub result_json: Option<String>,
    pub error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub revision: i64,
}
