export type VersionType = 'original' | 'live' | 'concert' | 'remix' | 'remastered' | 'acoustic' | 'unplugged' | 'bonus_track' | 'sped_up' | 'slowed' | 'radio_edit' | 'cover' | 'instrumental' | 'karaoke' | 'reaction' | 'nightcore' | 'other' | 'unknown'
export type DataState = 'NONE' | 'REAL_FILE' | 'REAL_TEXT' | 'REAL_ACCOUNT' | 'REAL_PUBLIC_LINK' | 'DEMO' | 'ERROR'
export type MetadataStatus = 'complete' | 'partial' | 'missing'

export interface ImportedTrack {
  title: string
  artists: string[]
  album?: string
  release_date?: string
  genres: string[]
  duration_ms?: number
  energy_score?: number
  source: string
  original_row: string
  metadata_status: MetadataStatus
  metadata_confidence: number
  warnings: string[]
}

export interface ImportPreviewRequest {
  name?: string
  file_name?: string
  format: string
  content: string
  data_state: 'REAL_FILE' | 'REAL_TEXT'
  text_order?: 'artist_title' | 'title_artist'
}

export interface ImportPreview {
  id: string
  name: string
  file_name?: string
  data_state: 'REAL_FILE' | 'REAL_TEXT'
  source_label: string
  total_rows: number
  parsed_count: number
  warning_count: number
  invalid_count: number
  detected_fields: string[]
  preview_tracks: ImportedTrack[]
  requires_column_confirmation: boolean
  text_order: 'artist_title' | 'title_artist'
  questions: string[]
}

export interface ImportAnalysisSummary {
  data_state: 'REAL_FILE' | 'REAL_TEXT'
  source_label: string
  input_count: number
  parsed_count: number
  analyzed_count: number
  complete_metadata_count: number
  partial_metadata_count: number
  unmatched_count: number
  genre_matched_count: number
  genre_coverage: number
  energy_matched_count: number
  energy_coverage: number
}

export interface Track {
  id: string
  title: string
  normalized_title: string
  artists: string[]
  album?: string
  genres: string[]
  release_year?: number
  language?: string
  duration_ms?: number
  platform: string
  platform_url?: string
  external_ids: Record<string, string>
  version_type: VersionType
  mood_tags: string[]
  energy_score?: number
  popularity?: number
  metadata_confidence: number
}

export interface TasteMetric { label: string; value: number; display: string; explanation: string }
export interface TasteReport {
  playlist_name: string
  source_label: string
  is_demo: boolean
  track_count: number
  genre_distribution: [string, number][]
  artist_distribution: [string, number][]
  era_distribution: [string, number][]
  album_distribution: [string, number][]
  duplicate_track_count: number
  collaboration_track_count: number
  genre_matched_count: number
  genre_coverage: number
  energy_matched_count: number
  energy_coverage: number
  metrics: TasteMetric[]
  core_preferences: string[]
  adjacent_preferences: string[]
  unexplored_preferences: string[]
  summary: string
  confidence: number
  limitations: string[]
}

export interface Playlist { id: string; name: string; owner_label: string; source: string; is_demo: boolean; tracks: Track[] }
export interface Recommendation { track: Track; zone: string; reason: string; connection: string; expansion: string; match_score: number; novelty_score: number; candidate_source: string; match_confidence: number; source_endpoint: string; seed_track?: string; seed_artist?: string; lastfm_similarity?: number; tags: string[]; relaxation_level: number; already_in_source_playlist: boolean }
export interface RecommendationZoneSummary { zone: string; count: number; message: string }
export interface RecommendationSeed { title: string; artists: string[] }
export interface RecommendationQueryStats {
  successful_seed_count: number
  failed_seed_count: number
  raw_track_similar_count: number
  raw_artist_similar_count: number
  raw_artist_top_tracks_count: number
  raw_tag_top_tracks_count: number
  raw_candidate_count: number
  after_version_filter_count: number
  after_normalization_count: number
  after_deduplication_count: number
  after_source_exclusion_count: number
  after_artist_cap_count: number
  comfort_candidate_count: number
  expansion_candidate_count: number
  surprise_candidate_count: number
  tag_layer1_candidate_count: number
  tag_layer1_rejected_count: number
  tag_layer2_candidate_count: number
  tag_layer2_rejected_count: number
  core_tags: string[]
  tag_similar_success_count: number
  tag_similar_failure_count: number
  similar_tag_count: number
  layer1_tags: string[]
  layer2_tags: string[]
  tag_top_track_counts: { tag: string; layer: number; candidate_count: number; source: string }[]
  request_budget_exhausted_count: number
  request_budget_used_count: number
  retry_count: number
  genre_bridge_candidate_count: number
  second_hop_artist_candidate_count: number
  deduplicated_candidate_count: number
}
export interface RecommendationSummary { source_label: string; status: string; message: string; candidate_count: number; zones: RecommendationZoneSummary[]; seeds: RecommendationSeed[]; query_stats: RecommendationQueryStats; comfort_pool: Recommendation[]; expansion_pool: Recommendation[]; surprise_pool: Recommendation[] }
export interface AlternateVersionCandidate { title: string; artists: string[]; platform: string; version_type: VersionType; duration_ms?: number; official_status: string; match_confidence: number; source_url: string; reason: string; is_alternate_version: boolean }
export interface AlternateVersionSearchResult { source_track: Track; provider: string; status: string; message: string; candidates: AlternateVersionCandidate[]; is_mock: boolean }
export interface TransferTrack { title: string; artists: string[]; album?: string; duration_ms?: number; isrc?: string; source_platform: string; source_track_id: string; source_url?: string; normalized_title: string; normalized_artists: string[]; version_type: VersionType }
export interface TransferCandidate { target_id: string; title: string; artists: string[]; duration_ms?: number; source_url?: string; channel_name?: string; official_status: string; version_type: VersionType; title_score: number; artist_score: number; duration_score?: number; version_score: number; score: number; reason: string }
export interface TransferMatch { source_track: TransferTrack; candidates: TransferCandidate[]; selected_target_id?: string; status: 'MATCHED_HIGH' | 'MATCHED_AMBIGUOUS' | 'UNMATCHED' | 'SKIPPED'; score: number; reason: string; requires_confirmation: boolean; alternate_version_fallback: boolean }
export interface TransferPreview { preview_id: string; playlist_name: string; source_count: number; high_confidence_count: number; ambiguous_count: number; unmatched_count: number; alternate_fallback_count: number; matches: TransferMatch[]; provider: string; destination_platform: 'spotify' | 'youtube'; status: string; message: string; requires_explicit_confirmation: boolean; source_was_modified: boolean; is_mock: boolean }
export interface TransferTrackResult { source_track: TransferTrack; target_id?: string; status: string; error?: string }
export interface TransferResult { run_id: string; preview_id: string; status: string; source_count: number; matched_count: number; written_count: number; failed_count: number; skipped_count: number; unmatched_count: number; progress: number; playlist_id?: string; playlist_url?: string; report_csv_url?: string; report_json_url?: string; results: TransferTrackResult[]; source_was_modified: boolean; is_mock: boolean }
export interface TransferRun { id: string; preview_id: string; destination_platform: 'spotify' | 'youtube'; status: 'QUEUED' | 'RUNNING' | 'CANCELLING' | 'CANCELLED' | 'FAILED' | 'COMPLETED'; processed_count: number; source_count: number; progress: number; result?: TransferResult; error?: string; is_mock: boolean; revision: number; created_at: number; updated_at: number }
export interface RouteStep { genre: string; explanation: string; tracks: Track[] }
export interface BridgeTrack { track: Track; reason: string; reason_for_a: string; reason_for_b: string; shared_basis: string[]; candidate_source: string; phase: string; bridge_score: number; already_in_a: boolean; already_in_b: boolean }
export interface ComparisonReport {
  user_a: string
  user_b: string
  metrics: TasteMetric[]
  track_count_a: number
  track_count_b: number
  shared_track_count: number
  shared_artists: string[]
  shared_genres: string[]
  user_a_signatures: string[]
  user_b_signatures: string[]
  summary: string
  bridge_playlist: BridgeTrack[]
  is_demo: boolean
  data_source: string
  saved_locally: boolean
}
export interface PersonalAnalysis { analysis_id: string; playlist: Playlist; report: TasteReport; recommendations: Recommendation[]; route: RouteStep[]; recommendation_summary: RecommendationSummary; import_summary?: ImportAnalysisSummary; unmatched_tracks: ImportedTrack[] }
export interface DemoPayload {
  generated_at: string
  disclosure: string
  personal: PersonalAnalysis
  comparison: ComparisonReport
  available_playlists: Playlist[]
}

export interface WriterStatus { platform: string; label: string; availability: string; authorized: boolean; is_demo: boolean; message: string }
export interface DataUseCapabilities {
  can_read_account_identity: boolean
  can_list_playlists: boolean
  can_read_playlist_items: boolean
  can_display_attributed_metadata: boolean
  can_transfer_playlist_metadata: boolean
  can_create_playlist: boolean
  can_add_items: boolean
  can_analyze_content: boolean
  can_derive_metrics: boolean
  can_cross_platform_compare: boolean
  can_send_to_llm: boolean
  can_train_model: boolean
  explanation: string
}
export interface PlatformCapability {
  platform: string
  auth_supported: boolean
  playlist_read_supported: boolean
  playlist_write_supported: boolean
  public_link_import_supported: boolean
  file_import_supported: boolean
  compare_supported: boolean
  transfer_source_supported: boolean
  transfer_destination_supported: boolean
  copy_source_supported: boolean
  copy_destination_supported: boolean
  playlist_read_for_copy: boolean
  playlist_read_for_compare: boolean
  playlist_read_for_recommendation: boolean
  alternate_version_search_supported: boolean
  status: string
  reason: string
  display_name: string
  region: string
  capability_status: string
  status_label: string
  account_connection: string
  public_playlist_links: string
  playlist_read: string
  playlist_write: string
  search_links: boolean
  requires_review: boolean
  configured: boolean
  official_docs_url?: string
  action_kind: string
  description: string
  policy_notice?: string
  data_use: DataUseCapabilities
}
export interface ProviderConfigurationStatus {
  platform: string
  display_name: string
  configured: boolean
  validation_status: string
  required_environment_variables: string[]
  present_environment_variables: string[]
  missing_environment_variables: string[]
  redirect_uri?: string
  developer_dashboard_url?: string
  setup_steps: string[]
  secrets_exposed: boolean
  message: string
}
export interface PlaylistLinkInspection {
  platform?: string
  platform_label?: string
  recognized: boolean
  playlist_id?: string
  normalized_url?: string
  resolved_url?: string
  structured_data_status?: string
  publicly_accessible?: boolean
  access_status: string
  playlist_name?: string
  track_count?: number
  preview_tracks: Track[]
  can_analyze: boolean
  message: string
  next_step: string
}
export interface SpotifyConnectionStatus {
  configured: boolean
  connected: boolean
  user_id?: string
  display_name?: string
  avatar_url?: string
  message: string
  policy_notice: string
}
export interface SpotifyPlaylistSummary {
  id: string
  name: string
  owner_name: string
  track_count: number
  collaborative: boolean
  public?: boolean
  spotify_url?: string
  image_url?: string
}
export interface SpotifyImportResult {
  playlists: { id: string; name: string; spotify_url?: string; imported_count: number }[]
  tracks: Track[]
  track_count: number
  data_use: DataUseCapabilities
  policy_notice: string
  attribution: string
}
export interface YouTubeConnectionStatus extends SpotifyConnectionStatus { channel_id?: string; channel_title?: string }
export interface YouTubePlaylistSummary { id: string; name: string; description: string; item_count: number; youtube_url: string; image_url?: string }
export interface YouTubeImportResult { playlists: { id: string; name: string; youtube_url: string; imported_count: number }[]; tracks: Track[]; track_count: number; data_use: DataUseCapabilities; policy_notice: string; attribution: string }
export interface AppleMusicBootstrap { configured: boolean; developer_token?: string; storefront?: string; validation_status: string; message: string; real_account_validation_completed: boolean }
export interface MatchCandidate {
  target_track_id: string
  title: string
  artists: string[]
  album?: string
  duration_ms?: number
  channel_name?: string
  official_status: string
  target_url?: string
  version_type: VersionType
  confidence: number
  match_reason: string
  available_in_market: boolean
}
export interface PlatformTrackMatch {
  source_track: Track
  target_platform: string
  target_track_id?: string
  target_url?: string
  version_type: VersionType
  confidence: number
  match_reason: string
  status: 'matched' | 'needs_confirmation' | 'unmatched' | 'selected'
  candidates: MatchCandidate[]
}
export interface ExportPreview {
  preview_id: string
  platform: string
  playlist_name: string
  matches: PlatformTrackMatch[]
  requested_count: number
  auto_matched_count: number
  needs_confirmation_count: number
  unmatched_count: number
  requires_explicit_confirmation: boolean
  is_demo: boolean
  disclosure?: string
}
export interface ExportResult {
  playlist_name: string
  platform: string
  playlist_url?: string
  requested_count: number
  added_count: number
  failed_count: number
  needs_confirmation_count: number
  successful_tracks: PlatformTrackMatch[]
  failed_tracks: { track: Track; reason: string }[]
  ambiguous_tracks: PlatformTrackMatch[]
  is_demo: boolean
  disclosure?: string
}

export interface AgentTask {
  id: string
  scenario: 'personal_exploration' | 'friend_bridge'
  goal: string
  status: 'PLANNING' | 'RUNNING' | 'WAITING_USER_CONFIRMATION' | 'BLOCKED_EXTERNAL_AUTH' | 'COMPLETED' | 'FAILED' | 'CANCELLED'
  current_step: number
  max_steps: number
  progress: number
  message: string
  analysis_id?: string
  analysis_a_id?: string
  analysis_b_id?: string
  transfer_preview_id?: string
  use_demo: boolean
  data_state: DataState
  decision_mode: 'LLM' | 'DETERMINISTIC_FALLBACK'
  decisions_json: string
  normalized_intent_json: string
  plan_json: string
  completed_steps_json: string
  pending_steps_json: string
  tool_calls_json: string
  tool_results_json: string
  retries: number
  warnings_json: string
  checkpoint_json?: string
  input_tokens: number
  output_tokens: number
  estimated_cost_usd: number
  result_json?: string
  error?: string
  created_at: number
  updated_at: number
  revision: number
}

export interface AgentStep {
  step_id: string
  label: string
  tool: string
  status: 'PENDING' | 'RUNNING' | 'COMPLETED' | 'FAILED' | 'CANCELLED'
  attempts: number
  last_error?: string
}

export interface AgentPlan { scenario: string; steps: AgentStep[] }
export interface AgentDecision { action: string; next_tool?: string; arguments: Record<string, unknown>; reason: string; finish: boolean }

export interface AgentSettings {
  endpoint: string
  model: string
  temperature: number
  max_tokens: number
  max_agent_steps: number
  request_timeout_seconds: number
  retry_limit: number
  max_cost_usd: number
  input_price_per_million: number
  output_price_per_million: number
  api_key_available: boolean
}
