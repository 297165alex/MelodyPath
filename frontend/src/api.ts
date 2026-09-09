import type { AgentSettings, AgentTask, AlternateVersionSearchResult, AppleMusicBootstrap, ComparisonReport, DemoPayload, ExportPreview, ExportResult, ImportPreview, ImportPreviewRequest, PlatformCapability, PlaylistLinkInspection, ProviderConfigurationStatus, SpotifyConnectionStatus, SpotifyImportResult, SpotifyPlaylistSummary, PersonalAnalysis, Track, TransferPreview, TransferResult, TransferRun, VersionType, WriterStatus, YouTubeConnectionStatus, YouTubeImportResult, YouTubePlaylistSummary } from './types'

async function request<T>(url: string, init?: RequestInit): Promise<T> {
  const response = await fetch(url, { ...init, credentials: 'include' })
  if (!response.ok) {
    const body = await response.json().catch(() => ({ error: `请求失败：${response.status}` })) as { error?: string }
    throw new Error(body.error ?? `请求失败：${response.status}`)
  }
  return response.json() as Promise<T>
}

async function analyzeImport(id: string, onProgress?: (phase: 'metadata' | 'recommendation') => void): Promise<PersonalAnalysis> {
  if (!id) throw new Error('缺少 import_id，请重新选择歌单生成预览。')
  if (!onProgress) return request<PersonalAnalysis>(`/api/imports/${encodeURIComponent(id)}/analyze`, { method: 'POST' })
  const response = await fetch(`/api/imports/${encodeURIComponent(id)}/analyze`, {
    method: 'POST', credentials: 'include', headers: { Accept: 'application/x-ndjson' },
  })
  if (!response.ok) {
    const body = await response.json().catch(() => ({}))
    throw new Error(body.error ?? `分析失败：HTTP ${response.status}`)
  }
  // Retain compatibility with servers returning the existing JSON contract.
  if (!response.headers.get('content-type')?.includes('application/x-ndjson')) return response.json()
  const reader = response.body?.getReader()
  if (!reader) throw new Error('分析响应为空，请重试。')
  const decoder = new TextDecoder()
  let buffer = ''
  try {
    while (true) {
      const { value, done } = await reader.read()
      buffer += decoder.decode(value, { stream: !done })
      const lines = buffer.split('\n')
      buffer = lines.pop() ?? ''
      if (done && buffer.trim()) lines.push(buffer)
      for (const line of lines) {
        if (!line.trim()) continue
        const event = JSON.parse(line)
        if (event.error) throw new Error(event.error)
        if (event.phase === 'metadata' || event.phase === 'recommendation') onProgress(event.phase)
        if (event.result?.analysis_id && event.result?.playlist) return event.result as PersonalAnalysis
      }
      if (done) throw new Error('分析连接中断，未收到完整结果。预览已保留，请重试。')
    }
  } finally { await reader.cancel().catch(() => {}); reader.releaseLock() }
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
  analyzeImport,
  compareAnalyses: (analysisA: PersonalAnalysis, analysisB: PersonalAnalysis) => request<ComparisonReport>('/api/compare', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ analysis_a: analysisA, analysis_b: analysisB, save_locally: false }),
  }),
  searchAlternateVersions: (track: Track, versionTypes: VersionType[], useMock = false) => request<AlternateVersionSearchResult>('/api/alternate-versions/search', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ track, version_types: versionTypes, use_mock: useMock }),
  }),
  previewTransfer: (playlistName: string, tracks: Track[], destinationPlatform: 'spotify' | 'youtube', allowAlternateVersions: boolean, useMock: boolean) => request<TransferPreview>('/api/transfers/preview', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ playlist_name: playlistName, tracks, destination_platform: destinationPlatform, allow_alternate_versions: allowAlternateVersions, use_mock: useMock }),
  }),
  executeTransfer: (previewId: string, selections: Record<string, string>) => request<TransferResult>('/api/transfers/execute', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ preview_id: previewId, confirmed: true, selections, privacy: 'private' }),
  }),
  createTransferRun: (previewId: string, selections: Record<string, string>) => request<TransferRun>('/api/transfers/runs', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ preview_id: previewId, confirmed: true, selections, privacy: 'private' }),
  }),
  transferRun: (id: string) => request<TransferRun>(`/api/transfers/runs/${id}`),
  cancelTransferRun: (id: string) => request<TransferRun>(`/api/transfers/runs/${id}/cancel`, { method: 'POST' }),
  resumeTransferRun: (id: string) => request<TransferRun>(`/api/transfers/runs/${id}/resume`, { method: 'POST' }),
  previewExport: (platform: string, playlistName: string, tracks: Track[], format: string) => request<ExportPreview>('/api/exports/preview', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ platform, playlist_name: playlistName, tracks, format }),
  }),
  executeExport: (previewId: string, selections: Record<string, string>) => request<ExportResult>('/api/exports/execute', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ preview_id: previewId, confirmed: true, selections }),
  }),
  createTask: (scenario: AgentTask['scenario'], goal: string | undefined, binding: { analysis_id?: string; analysis_a_id?: string; analysis_b_id?: string; use_demo: boolean }) => request<AgentTask>('/api/tasks', {
    method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ scenario, goal, ...binding }),
  }),
  tasks: () => request<AgentTask[]>('/api/tasks'),
  cancelTask: (id: string) => request<AgentTask>(`/api/tasks/${id}/cancel`, { method: 'POST' }),
  resumeTask: (id: string) => request<AgentTask>(`/api/tasks/${id}/resume`, { method: 'POST' }),
  settings: () => request<AgentSettings>('/api/settings'),
  saveSettings: (settings: AgentSettings) => request<AgentSettings>('/api/settings', {
    method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify(settings),
  }),
}
