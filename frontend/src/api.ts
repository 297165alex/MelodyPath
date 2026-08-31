import type { AgentSettings, AgentTask, AppleMusicBootstrap, DemoPayload, ExportPreview, ExportResult, ImportPreview, ImportPreviewRequest, PlatformCapability, PlaylistLinkInspection, ProviderConfigurationStatus, SpotifyConnectionStatus, SpotifyImportResult, SpotifyPlaylistSummary, PersonalAnalysis, Track, WriterStatus, YouTubeConnectionStatus, YouTubeImportResult, YouTubePlaylistSummary } from './types'

async function request<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, { ...init, credentials: 'include' })
  if (!response.ok) {
    const body = await response.json().catch(() => ({ error: `请求失败：${response.status}` })) as { error?: string }
    throw new Error(body.error ?? `请求失败：${response.status}`)
  }
  return response.json() as Promise<T>
}

export const api = {
  demo: () => request<DemoPayload>('/api/demo'),
  statuses: () => request<WriterStatus[]>('/api/writers/status'),
  capabilities: () => request<PlatformCapability[]>('/api/platforms/capabilities'),
  inspectPlaylistLink: (url: string) => request<PlaylistLinkInspection>('/api/playlists/inspect-link', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ url }),
  }),
  spotifyMe: () => request<SpotifyConnectionStatus>('/api/spotify/me'),
  spotifyPlaylists: () => request<SpotifyPlaylistSummary[]>('/api/spotify/playlists'),
  importSpotifyPlaylists: (playlistIds: string[]) => request<SpotifyImportResult>('/api/spotify/import', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ playlist_ids: playlistIds }),
  }),
  disconnectSpotify: () => request<{ disconnected: boolean; local_token_deleted: boolean; message: string }>('/api/spotify/disconnect', { method: 'POST' }),
  providerConfig: (platform: 'spotify' | 'youtube' | 'apple') => request<ProviderConfigurationStatus>(`/api/config/${platform}`),
  checkProviderConfig: (platform: 'spotify' | 'youtube') => request<ProviderConfigurationStatus>(`/api/config/${platform}/check`, { method: 'POST' }),
  youtubeMe: () => request<YouTubeConnectionStatus>('/api/youtube/me'),
  youtubePlaylists: () => request<YouTubePlaylistSummary[]>('/api/youtube/playlists'),
  importYouTubePlaylists: (playlistIds: string[]) => request<YouTubeImportResult>('/api/youtube/import', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ playlist_ids: playlistIds }),
  }),
  disconnectYouTube: () => request<{ disconnected: boolean; local_token_deleted: boolean; message: string }>('/api/youtube/disconnect', { method: 'POST' }),
  appleBootstrap: () => request<AppleMusicBootstrap>('/api/apple/musickit/bootstrap'),
  analyzeManual: (name: string, text: string) => request<PersonalAnalysis>('/api/analyze/manual', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ name, text }),
  }),
  previewImport: (input: ImportPreviewRequest) => request<ImportPreview>('/api/imports/preview', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(input),
  }),
  analyzeImport: (id: string) => request<PersonalAnalysis>(`/api/imports/${id}/analyze`, { method: 'POST' }),
  previewExport: (platform: string, playlistName: string, tracks: Track[], format: string) => request<ExportPreview>('/api/exports/preview', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ platform, playlist_name: playlistName, tracks, format }),
  }),
  executeExport: (previewId: string, selections: Record<string, string>) => request<ExportResult>('/api/exports/execute', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ preview_id: previewId, confirmed: true, selections }),
  }),
  createTask: (scenario: AgentTask['scenario'], goal?: string) => request<AgentTask>('/api/tasks', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ scenario, goal }),
  }),
  tasks: () => request<AgentTask[]>('/api/tasks'),
  cancelTask: (id: string) => request<AgentTask>(`/api/tasks/${id}/cancel`, { method: 'POST' }),
  settings: () => request<AgentSettings>('/api/settings'),
  saveSettings: (settings: AgentSettings) => request<AgentSettings>('/api/settings', {
    method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify(settings),
  }),
}
