import { useEffect, useMemo, useState } from 'react'
import { api } from './api'
import ExportModal from './ExportModal'
import type { AgentSettings, AgentTask, BridgeTrack, ComparisonReport, DataState, DemoPayload, ImportPreview, ImportPreviewRequest, PersonalAnalysis, PlatformCapability, PlaylistLinkInspection, ProviderConfigurationStatus, Recommendation, SpotifyConnectionStatus, SpotifyImportResult, SpotifyPlaylistSummary, Track, WriterStatus, YouTubeConnectionStatus, YouTubeImportResult, YouTubePlaylistSummary } from './types'

type Tab = 'home' | 'taste' | 'recommend' | 'compare' | 'agent' | 'history' | 'settings'
type ExportTarget = { platform: string; tracks: Track[]; label: string }

const navItems: { id: Tab; label: string }[] = [
  { id: 'home', label: '开始' }, { id: 'taste', label: '品味地图' }, { id: 'recommend', label: '探索推荐' },
  { id: 'compare', label: '好友桥梁' }, { id: 'agent', label: 'Agent 运行' }, { id: 'history', label: '历史' }, { id: 'settings', label: '设置' },
]

export default function App() {
  const [tab, setTab] = useState<Tab>('home')
  const [demo, setDemo] = useState<DemoPayload | null>(null)
  const [statuses, setStatuses] = useState<WriterStatus[]>([])
  const [capabilities, setCapabilities] = useState<PlatformCapability[]>([])
  const [spotify, setSpotify] = useState<SpotifyConnectionStatus | null>(null)
  const [youtube, setYoutube] = useState<YouTubeConnectionStatus | null>(null)
  const [loading, setLoading] = useState(true)
  const [fatalError, setFatalError] = useState('')
  const [manualPersonal, setManualPersonal] = useState<PersonalAnalysis | null>(null)
  const [dataState, setDataState] = useState<DataState>('NONE')
  const [exportTarget, setExportTarget] = useState<ExportTarget | null>(null)

  useEffect(() => {
    Promise.all([api.demo(), api.statuses(), api.capabilities(), api.spotifyMe(), api.youtubeMe()])
      .then(([payload, writerStatuses, platformCapabilities, spotifyStatus, youtubeStatus]) => {
        setDemo(payload); setStatuses(writerStatuses); setCapabilities(platformCapabilities); setSpotify(spotifyStatus); setYoutube(youtubeStatus)
      })
      .catch((reason: unknown) => setFatalError(reason instanceof Error ? reason.message : '后端连接失败'))
      .finally(() => setLoading(false))
  }, [])

  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    if (params.get('spotify') === 'connected' || params.get('youtube') === 'connected') {
      Promise.all([api.statuses(), api.spotifyMe(), api.youtubeMe()]).then(([writerStatuses, spotifyStatus, youtubeStatus]) => {
        setStatuses(writerStatuses); setSpotify(spotifyStatus); setYoutube(youtubeStatus)
      }).catch(() => undefined)
      window.history.replaceState({}, '', '/')
    }
  }, [])

  const refreshConnections = async () => {
    const [writerStatuses, platformCapabilities, spotifyStatus, youtubeStatus] = await Promise.all([api.statuses(), api.capabilities(), api.spotifyMe(), api.youtubeMe()])
    setStatuses(writerStatuses); setCapabilities(platformCapabilities); setSpotify(spotifyStatus); setYoutube(youtubeStatus)
  }

  const openExport = (platform: string, tracks: Track[], label: string) => setExportTarget({ platform, tracks, label })
  const activePersonal = dataState === 'DEMO'
    ? demo?.personal ?? null
    : dataState === 'REAL_FILE' || dataState === 'REAL_TEXT'
      ? manualPersonal
      : null

  if (loading) return <div className="loading-screen"><Logo /><span>正在唤醒音乐路径…</span></div>
  if (fatalError || !demo) return <div className="loading-screen error-screen"><Logo /><h1>暂时无法连接 Rust 后端</h1><p>{fatalError}</p><code>cargo run -p melody-path-api</code></div>

  return <div className="app-shell">
    <header className="topbar">
      <button className="brand" onClick={() => setTab('home')}><Logo /><span><strong>MelodyPath</strong><small>可解释音乐探索 Agent</small></span></button>
      <nav aria-label="主导航">{navItems.map((item) => <button className={tab === item.id ? 'active' : ''} onClick={() => setTab(item.id)} key={item.id}>{item.label}</button>)}</nav>
      <div className="header-status"><span className="pulse" />Rust API 在线</div>
    </header>

    <main>
      {tab === 'home' && <Home demo={demo} capabilities={capabilities} spotify={spotify} youtube={youtube} onRefreshConnections={refreshConnections} onDemo={() => { setManualPersonal(null); setDataState('DEMO'); setTab('taste') }} onManual={(personal, state) => { setManualPersonal(personal); setDataState(state); setTab('taste') }} onImportError={() => { setManualPersonal(null); setDataState('ERROR') }} onCompare={() => setTab('compare')} onExport={openExport} />}
      {tab === 'taste' && activePersonal && <TastePage personal={activePersonal} canExplore={Boolean(activePersonal.recommendation_summary)} onExplore={() => setTab('recommend')} />}
      {tab === 'recommend' && activePersonal && <RecommendationPage personal={activePersonal} onExport={openExport} />}
      {tab === 'compare' && <ComparePage report={demo.comparison} onExport={openExport} />}
      {tab === 'agent' && <AgentPage />}
      {tab === 'history' && <HistoryPage />}
      {tab === 'settings' && <SettingsPage />}
    </main>

    <footer className="site-footer"><span>MelodyPath 是《程序设计训练（Rust语言）》AI Agent 课程项目，同时将阶段性成果用于智理杯智能体大赛。</span><span>Demo 数据明确标注 · 不推断年龄或心理状态</span></footer>
    {exportTarget && <ExportModal initialPlatform={exportTarget.platform} tracks={exportTarget.tracks} sourceLabel={exportTarget.label} statuses={statuses} onClose={() => setExportTarget(null)} />}
  </div>
}

function Logo() {
  return <svg className="logo" viewBox="0 0 42 42" aria-hidden="true"><path d="M8 29c5-12 9-3 13-14 3-8 8-5 13-10"/><circle cx="8" cy="29" r="3"/><circle cx="21" cy="15" r="3"/><circle cx="34" cy="5" r="3"/></svg>
}

function Home({ demo, capabilities, spotify, youtube, onRefreshConnections, onDemo, onManual, onImportError, onCompare, onExport }: {
  demo: DemoPayload
  capabilities: PlatformCapability[]
  spotify: SpotifyConnectionStatus | null
  youtube: YouTubeConnectionStatus | null
  onRefreshConnections: () => Promise<void>
  onDemo: () => void
  onManual: (personal: PersonalAnalysis, state: 'REAL_FILE' | 'REAL_TEXT') => void
  onImportError: () => void
  onCompare: () => void
  onExport: (platform: string, tracks: Track[], label: string) => void
}) {
  const [text, setText] = useState('BIBI - Kazino\nDEAN - instagram\nMariya Takeuchi - Plastic Love')
  const [name, setName] = useState('我的歌单')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [link, setLink] = useState('')
  const [linkBusy, setLinkBusy] = useState(false)
  const [linkResult, setLinkResult] = useState<PlaylistLinkInspection | null>(null)
  const [moreOpen, setMoreOpen] = useState(true)
  const [spotifyPickerOpen, setSpotifyPickerOpen] = useState(false)
  const [youtubePickerOpen, setYoutubePickerOpen] = useState(false)
  const [configPlatform, setConfigPlatform] = useState<'spotify' | 'youtube' | 'apple' | null>(null)
  const [importPreview, setImportPreview] = useState<ImportPreview | null>(null)
  const [pendingImport, setPendingImport] = useState<ImportPreviewRequest | null>(null)

  const analyzeLegacy = async (overrideText: string, overrideName: string) => {
    setBusy(true); setError('')
    try { onManual(await api.analyzeManual(overrideName, overrideText), 'REAL_TEXT') }
    catch (reason) { onImportError(); setError(reason instanceof Error ? reason.message : '分析失败') }
    finally { setBusy(false) }
  }

  const prepareImport = async (request: ImportPreviewRequest) => {
    setBusy(true); setError(''); setImportPreview(null); setPendingImport(request)
    try { setImportPreview(await api.previewImport(request)) }
    catch (reason) { onImportError(); setError(reason instanceof Error ? reason.message : '导入解析失败') }
    finally { setBusy(false) }
  }

  const confirmImport = async () => {
    if (!importPreview) return
    setBusy(true); setError('')
    try { onManual(await api.analyzeImport(importPreview.id), importPreview.data_state) }
    catch (reason) { onImportError(); setError(reason instanceof Error ? reason.message : '真实数据分析失败') }
    finally { setBusy(false) }
  }

  const inspectLink = async () => {
    setLinkBusy(true); setError(''); setLinkResult(null)
    try { setLinkResult(await api.inspectPlaylistLink(link)) }
    catch (reason) { setError(reason instanceof Error ? reason.message : '链接检查失败') }
    finally { setLinkBusy(false) }
  }

  const confirmLink = () => {
    if (!linkResult?.can_analyze || linkResult.preview_tracks.length === 0) return
    const lines = linkResult.preview_tracks.map((track) => `${track.artists.join(', ')} - ${track.title}`).join('\n')
    void analyzeLegacy(lines, linkResult.playlist_name ?? `${linkResult.platform_label ?? '公开'}歌单`)
  }

  const loadFile = async (file?: File) => {
    if (!file) return
    try {
      const raw = await file.text()
      setName(file.name.replace(/\.[^.]+$/, ''))
      const format = file.name.split('.').pop()?.toLowerCase() ?? ''
      await prepareImport({ name: file.name.replace(/\.[^.]+$/, ''), file_name: file.name, format, content: raw, data_state: 'REAL_FILE' })
      setMoreOpen(true)
    } catch (reason) { onImportError(); setError(reason instanceof Error ? reason.message : '文件读取失败') }
  }

  const focusLink = () => {
    document.getElementById('public-link-panel')?.scrollIntoView({ behavior: 'smooth' })
    window.setTimeout(() => document.getElementById('playlist-link')?.focus(), 450)
  }

  const disconnectSpotify = async () => {
    setError('')
    try { await api.disconnectSpotify(); await onRefreshConnections() }
    catch (reason) { setError(reason instanceof Error ? reason.message : '解除连接失败') }
  }

  const disconnectYoutube = async () => {
    setError('')
    try { await api.disconnectYouTube(); await onRefreshConnections() }
    catch (reason) { setError(reason instanceof Error ? reason.message : '解除连接失败') }
  }

  return <>
    <section className="hero">
      <div className="hero-copy"><span className="eyebrow">LOCAL-FIRST MUSIC ANALYSIS</span><h1>不登录账号，<br/><em>也能先分析歌单。</em></h1><p>直接粘贴“歌手 - 歌名”清单或上传文件。Rust 后端会在本机完成统计，联网时仅查询公开音乐目录补全 Genre、年代与能量；随后生成真正基于这份歌单的探索路线和推荐。</p><div className="hero-actions"><button className="primary big" onClick={() => document.getElementById('more-import')?.scrollIntoView({ behavior: 'smooth' })}>立即本地分析 <span>→</span></button><button className="secondary big" onClick={() => document.getElementById('connection-panel')?.scrollIntoView({ behavior: 'smooth' })}>连接音乐平台</button><button className="secondary big" onClick={onCompare}>与朋友比较</button></div><div className="trust-row"><span>✓ 无需账号密码</span><span>✓ Rust 本机分析</span><span>✓ 元数据失败也可降级运行</span></div></div>
      <div className="hero-map" aria-label="音乐探索路线示意"><div className="orbit orbit-one"/><div className="orbit orbit-two"/><GenreNode x="12%" y="62%" name="Korean R&B" tone="coral"/><GenreNode x="40%" y="35%" name="Alt. R&B" tone="gold"/><GenreNode x="70%" y="18%" name="Neo Soul" tone="mint"/><GenreNode x="73%" y="72%" name="Dream Pop" tone="blue"/><svg viewBox="0 0 500 400"><path d="M85 265 C155 245 150 170 224 163 S315 88 382 90"/><path className="dashed" d="M224 163 C285 188 318 286 390 280"/></svg><div className="map-caption"><strong>Genre 不是标签墙</strong><span>它是一张可以解释的路线图</span></div></div>
    </section>

    <section className="connection-section" id="connection-panel">
      <div className="section-heading wide"><span className="eyebrow">OFFICIAL CONNECTIONS FIRST</span><h2>连接你的音乐平台</h2><p>只走平台官方授权、MusicKit 或 SDK。登录发生在平台官方页面，MelodyPath 页面永远不会要求你输入音乐平台密码。</p></div>
      <div className="platform-group"><div className="platform-group-title"><strong>可操作平台</strong><span>能力状态由 Rust 后端实时返回</span></div><div className="platform-grid">
        {capabilities.filter((item) => ['spotify', 'youtube_music', 'apple_music', 'netease', 'qq_music'].includes(item.platform)).map((capability) => {
          const isSpotify = capability.platform === 'spotify'
          const isYoutube = capability.platform === 'youtube_music'
          const connected = isSpotify ? spotify?.connected : isYoutube ? youtube?.connected : false
          const displayName = isSpotify ? spotify?.display_name : isYoutube ? youtube?.channel_title ?? youtube?.display_name : undefined
          return <article className={`platform-card status-${connected ? 'connected' : capability.capability_status}`} key={capability.platform}><div className="platform-card-head"><span className={`platform-mark mark-${capability.platform}`}>{capability.display_name.slice(0, 1)}</span><div><h3>{capability.display_name}</h3><span className="capability-label">{connected ? `已连接 · ${displayName}` : capability.configured && capability.action_kind === 'connect' ? '已配置，可连接' : capability.status_label}</span></div></div><p>{capability.description}</p><div className="capability-facts"><span>读取：{humanCapability(capability.playlist_read)}</span><span>写入：{humanCapability(capability.playlist_write)}</span></div>{capability.policy_notice && <small>{capability.policy_notice}</small>}<div className="platform-actions">
            {isSpotify && spotify?.connected ? <><button className="primary" onClick={() => setSpotifyPickerOpen(true)}>选择我的歌单</button><button className="text-button" onClick={() => void disconnectSpotify()}>解除连接</button></> : isYoutube && youtube?.connected ? <><button className="primary" onClick={() => setYoutubePickerOpen(true)}>选择我的播放列表</button><button className="text-button" onClick={() => void disconnectYoutube()}>解除连接</button></> : capability.action_kind === 'connect' && capability.configured ? <a className="primary" href={isYoutube ? '/api/youtube/authorize' : '/api/spotify/authorize'}>前往官方授权</a> : (isSpotify || isYoutube || capability.platform === 'apple_music') ? <button className="secondary" onClick={() => setConfigPlatform(isSpotify ? 'spotify' : isYoutube ? 'youtube' : 'apple')}>配置向导</button> : capability.action_kind === 'paste_link' ? <button className="secondary" onClick={focusLink}>粘贴公开链接</button> : <button className="secondary" disabled>{capability.status_label}</button>}
            {capability.official_docs_url && <a className="docs-link" href={capability.official_docs_url} target="_blank" rel="noreferrer">官方说明 ↗</a>}
          </div></article>
        })}
      </div></div>
      <details className="more-platforms"><summary>更多平台（尚未实现或等待资格）</summary><div className="platform-grid">{capabilities.filter((item) => !['spotify', 'youtube_music', 'apple_music', 'netease', 'qq_music'].includes(item.platform)).map((item) => <article className="platform-card" key={item.platform}><h3>{item.display_name}</h3><p>{item.description}</p><span className="capability-label">{item.status_label}</span></article>)}</div></details>
      {spotify && <div className={`spotify-policy ${spotify.connected ? 'connected' : ''}`}><strong>{spotify.connected ? `Spotify 已连接：${spotify.display_name}` : 'Spotify 授权状态'}</strong><span>{spotify.message}</span><small>{spotify.policy_notice}</small></div>}
    </section>

    <section className="link-section" id="public-link-panel"><div className="section-heading"><span className="eyebrow">PUBLIC PLAYLIST LINK</span><h2>或者粘贴公开歌单链接</h2><p>自动识别平台、解析歌单 ID，并只访问官方域名检查公开可访问性。读取不到时会说明原因，不会偷偷改用账号 Cookie 或逆向登录。</p></div><div className="input-card link-card"><label className="field"><span>网易云、QQ音乐、Spotify 或 YouTube 公开分享链接</span><div className="link-input-row"><input id="playlist-link" type="url" placeholder="https://music.163.com/playlist?id=…" value={link} onChange={(event) => setLink(event.target.value)} onKeyDown={(event) => { if (event.key === 'Enter') void inspectLink() }}/><button className="primary" disabled={linkBusy || !link.trim()} onClick={() => void inspectLink()}>{linkBusy ? '正在检查…' : '识别并检查'}</button></div></label>{linkResult && <div className={`link-result ${linkResult.can_analyze ? 'ready' : ''}`}><div><span className={`access-dot access-${linkResult.access_status}`}/><strong>{linkResult.recognized ? `${linkResult.platform_label} · ${linkResult.playlist_id ? `ID ${linkResult.playlist_id}` : '链接已识别'}` : '未识别链接'}</strong></div><p>{linkResult.message}</p>{linkResult.preview_tracks.length > 0 && <><div className="link-preview"><span>前 {Math.min(10, linkResult.preview_tracks.length)} 首预览</span>{linkResult.preview_tracks.slice(0, 10).map((track) => <div key={track.id}><strong>{track.title}</strong><small>{track.artists.join(', ')}</small></div>)}</div><button className="primary" disabled={!linkResult.can_analyze} onClick={confirmLink}>确认并分析</button></>}<small>{linkResult.next_step}</small>{!linkResult.can_analyze && <button className="secondary" onClick={() => { setMoreOpen(true); document.getElementById('more-import')?.scrollIntoView({ behavior: 'smooth' }) }}>改为直接粘贴歌曲清单</button>}</div>}{error && <p className="error-box">{error}</p>}<p className="fine-print">“页面可访问”不等于“曲目已读取”。只有拿到可验证的合法曲目数据后，按钮才会显示“确认并分析”。</p></div></section>

    <section className="more-import-section" id="more-import"><details open={moreOpen} onToggle={(event) => setMoreOpen(event.currentTarget.open)}><summary><span><strong>无需登录，导入真实歌单</strong><small>先由 Rust 可靠解析并预览，确认后才会分析</small></span><b>{moreOpen ? '−' : '+'}</b></summary><div className="fallback-grid"><div className="input-card"><span className="eyebrow">REAL PLAYLIST IMPORT</span><h3>文件或批量文本</h3><p className="fallback-copy">支持 CSV、TSV、JSON、TXT、M3U/M3U8。解析失败会显示真实错误，绝不会切换到 Demo。</p><label className="field"><span>歌单名称</span><input value={name} onChange={(event) => setName(event.target.value)} /></label><label className="field"><span>批量文本</span><textarea rows={7} value={text} onChange={(event) => setText(event.target.value)} /></label><div className="input-actions"><label className="secondary upload-button">选择真实文件<input type="file" accept=".txt,.csv,.tsv,.json,.m3u,.m3u8" onChange={(event) => void loadFile(event.target.files?.[0])}/></label><button className="primary" onClick={() => void prepareImport({ name, format: 'txt', content: text, data_state: 'REAL_TEXT' })} disabled={busy}>{busy ? '正在解析…' : '解析并预览文本'}</button></div><p className="fine-print">文本支持“歌手 - 歌名”“歌名 — 歌手”“歌手 | 歌名”和“歌名 TAB 歌手”；顺序不确定时会要求确认。</p></div><div className="demo-fallback"><span className="demo-badge">DEMO MODE</span><h3>明确体验示例</h3><p>{demo.disclosure}</p><button className="secondary" onClick={onDemo}>体验 Demo</button><div><strong>只有点击本按钮才显示 Demo</strong><span>真实导入失败不会进入这里。</span></div></div></div>{error && <p className="error-box">{error}</p>}{importPreview && <ImportPreviewPanel preview={importPreview} busy={busy} onOrder={(order) => pendingImport && void prepareImport({ ...pendingImport, text_order: order })} onConfirm={() => void confirmImport()} />}</details></section>
    {spotifyPickerOpen && <SpotifyPlaylistPicker onClose={() => setSpotifyPickerOpen(false)} onExport={onExport} />}
    {youtubePickerOpen && <YouTubePlaylistPicker onClose={() => setYoutubePickerOpen(false)} onExport={onExport} />}
    {configPlatform && <ConfigurationWizard platform={configPlatform} onClose={() => setConfigPlatform(null)} onChecked={onRefreshConnections} />}
  </>
}

function ImportPreviewPanel({ preview, busy, onOrder, onConfirm }: { preview: ImportPreview; busy: boolean; onOrder: (order: 'artist_title' | 'title_artist') => void; onConfirm: () => void }) {
  return <section className="import-preview panel"><div className="panel-title"><div><span className="eyebrow">IMPORT PREVIEW · {preview.data_state}</span><h3>{preview.file_name ?? preview.source_label}</h3></div><strong>is_demo=false</strong></div><div className="import-stats"><span><b>{preview.total_rows}</b>总行数</span><span><b>{preview.parsed_count}</b>成功解析</span><span><b>{preview.warning_count}</b>警告</span><span><b>{preview.invalid_count}</b>无法解析</span></div><p>检测字段：{preview.detected_fields.join(' · ') || '文本列'}</p>{preview.questions.map((question) => <p className="warning-box" key={question}>{question}</p>)}{preview.requires_column_confirmation && <div className="order-confirm"><strong>请选择文本列含义</strong><button className="secondary" onClick={() => onOrder('artist_title')}>左侧歌手，右侧歌名</button><button className="secondary" onClick={() => onOrder('title_artist')}>左侧歌名，右侧歌手</button></div>}<div className="import-track-list"><div className="import-track-head"><span>#</span><span>歌名</span><span>歌手</span><span>专辑</span><span>状态</span></div>{preview.preview_tracks.map((track, index) => <div className="import-track-row" key={`${track.original_row}-${index}`}><span>{index + 1}</span><strong>{track.title}</strong><span>{track.artists.join(' / ')}</span><span>{track.album ?? '—'}</span><em>{track.metadata_status}</em></div>)}</div><footer className="preview-actions"><span>预览前 {Math.min(20, preview.parsed_count)} 首；确认后全部 {preview.parsed_count} 首参与基础分析。</span><button className="primary" disabled={busy || preview.requires_column_confirmation} onClick={onConfirm}>{busy ? '正在分析…' : '确认并分析真实数据'}</button></footer></section>
}

function humanCapability(value: string) {
  const labels: Record<string, string> = {
    official_api: '官方 API', official_api_transfer_only: '官方 API（仅传输）', needs_configuration: '需要配置',
    requires_official_credentials: '需正式资格', requires_platform_approval: '需平台审核', planned: '计划中',
    not_verified: '未验证', not_verified_for_public_web: '无通用网页接口', official_sdk_only: '仅官方 SDK',
  }
  return labels[value] ?? value
}

function ConfigurationWizard({ platform, onClose, onChecked }: { platform: 'spotify' | 'youtube' | 'apple'; onClose: () => void; onChecked: () => Promise<void> }) {
  const [status, setStatus] = useState<ProviderConfigurationStatus | null>(null)
  const [busy, setBusy] = useState(true)
  const [error, setError] = useState('')
  const load = async (validate = false) => {
    setBusy(true); setError('')
    try {
      const next = validate && platform !== 'apple' ? await api.checkProviderConfig(platform) : await api.providerConfig(platform)
      setStatus(next); await onChecked()
    } catch (reason) { setError(reason instanceof Error ? reason.message : '配置检查失败') }
    finally { setBusy(false) }
  }
  useEffect(() => { void load() }, [platform]) // eslint-disable-line react-hooks/exhaustive-deps
  return <div className="modal-backdrop"><section className="export-modal config-wizard" role="dialog" aria-modal="true"><header className="modal-header"><div><span className="eyebrow">LOCAL DEVELOPER SETUP</span><h2>{status?.display_name ?? platform} 配置向导</h2></div><button className="icon-button" onClick={onClose}>×</button></header>{busy && <p>正在检查后端环境变量…</p>}{error && <p className="error-box">{error}</p>}{status && <><div className={`configuration-verdict ${status.configured ? 'ready' : ''}`}><strong>{status.configured ? '配置已就绪' : '完成配置即可使用'}</strong><span>{status.message}</span></div>{status.redirect_uri && <label className="field"><span>Developer Console 中必须填写的精确 Redirect URI</span><code className="redirect-code">{status.redirect_uri}</code></label>}<div className="config-columns"><div><h3>缺少的环境变量</h3>{status.missing_environment_variables.length ? <ul>{status.missing_environment_variables.map((name) => <li><code>{name}</code></li>)}</ul> : <p>无</p>}</div><div><h3>已检测到</h3>{status.present_environment_variables.length ? <ul>{status.present_environment_variables.map((name) => <li><code>{name}</code></li>)}</ul> : <p>无</p>}</div></div><ol className="setup-steps">{status.setup_steps.map((step) => <li key={step}>{step}</li>)}</ol><p className="fine-print">此页面只显示变量名和状态，永远不会显示 Client Secret、私钥或 token。修改 .env 后需要重启 Rust 后端。</p><div className="modal-actions">{status.developer_dashboard_url && <a className="secondary" href={status.developer_dashboard_url} target="_blank" rel="noreferrer">打开官方控制台 ↗</a>}<button className="primary" disabled={busy} onClick={() => void load(true)}>检查配置</button></div>{platform === 'apple' && <p className="policy-box">MusicKit 连接器与配置检测已预留，但本项目尚未完成真实 Apple Music 账号验收，因此不会显示虚假的“登录成功”。</p>}</>}</section></div>
}

function YouTubePlaylistPicker({ onClose, onExport }: { onClose: () => void; onExport: (platform: string, tracks: Track[], label: string) => void }) {
  const [playlists, setPlaylists] = useState<YouTubePlaylistSummary[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [result, setResult] = useState<YouTubeImportResult | null>(null)
  const [busy, setBusy] = useState(true)
  const [error, setError] = useState('')
  useEffect(() => { api.youtubePlaylists().then(setPlaylists).catch((reason: unknown) => setError(reason instanceof Error ? reason.message : '读取 YouTube 播放列表失败')).finally(() => setBusy(false)) }, [])
  const toggle = (id: string) => setSelected((current) => { const next = new Set(current); next.has(id) ? next.delete(id) : next.add(id); return next })
  const importSelected = async () => { setBusy(true); setError(''); try { setResult(await api.importYouTubePlaylists([...selected])) } catch (reason) { setError(reason instanceof Error ? reason.message : '读取失败') } finally { setBusy(false) } }
  return <div className="modal-backdrop"><section className="export-modal spotify-picker" role="dialog" aria-modal="true"><header className="modal-header"><div><span className="eyebrow">YOUTUBE DATA API · OFFICIAL</span><h2>{result ? '确认导入的曲目' : '选择你拥有的播放列表'}</h2></div><button className="icon-button" onClick={onClose}>×</button></header><div className="spotify-restriction"><strong>合规边界</strong><span>官方 API 数据用于你主动请求的预览、传输和写回，不发送给 LLM，也不计算独立衍生画像。</span></div>{busy && <p>正在读取…</p>}{error && <p className="error-box">{error}</p>}{!busy && !result && <><div className="spotify-playlist-list">{playlists.map((playlist) => <label className={selected.has(playlist.id) ? 'spotify-playlist selected' : 'spotify-playlist'} key={playlist.id}><input type="checkbox" checked={selected.has(playlist.id)} onChange={() => toggle(playlist.id)}/>{playlist.image_url ? <img src={playlist.image_url} alt=""/> : <span className="playlist-placeholder">▶</span>}<span><strong>{playlist.name}</strong><small>{playlist.item_count} 项</small></span><a href={playlist.youtube_url} target="_blank" rel="noreferrer">YouTube ↗</a></label>)}</div><div className="modal-actions"><button className="secondary" onClick={onClose}>取消</button><button className="primary" disabled={!selected.size} onClick={() => void importSelected()}>读取所选播放列表</button></div></>}{result && <><div className="import-summary"><strong>{result.track_count}</strong><span>首条目已清理标题噪声并转换为统一 Track</span></div><p className="policy-box">{result.policy_notice}</p><div className="spotify-track-preview">{result.tracks.slice(0, 10).map((track, index) => <div key={track.id}><span>{index + 1}</span><div><strong>{track.title}</strong><small>{track.artists.join(', ')}</small></div>{track.platform_url && <a href={track.platform_url} target="_blank" rel="noreferrer">YouTube ↗</a>}</div>)}</div><div className="modal-actions"><button className="secondary" onClick={() => setResult(null)}>返回</button><button className="primary" onClick={() => { onExport('youtube', result.tracks, result.playlists.map((p) => p.name).join(' + ')); onClose() }}>预览并创建新的 YouTube 播放列表</button></div></>}</section></div>
}

function SpotifyPlaylistPicker({ onClose, onExport }: { onClose: () => void; onExport: (platform: string, tracks: Track[], label: string) => void }) {
  const [playlists, setPlaylists] = useState<SpotifyPlaylistSummary[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [result, setResult] = useState<SpotifyImportResult | null>(null)
  const [busy, setBusy] = useState(true)
  const [error, setError] = useState('')

  useEffect(() => {
    api.spotifyPlaylists().then(setPlaylists).catch((reason: unknown) => setError(reason instanceof Error ? reason.message : '读取 Spotify 歌单失败')).finally(() => setBusy(false))
  }, [])

  const toggle = (id: string) => setSelected((current) => {
    const next = new Set(current)
    if (next.has(id)) next.delete(id); else next.add(id)
    return next
  })
  const importSelected = async () => {
    setBusy(true); setError('')
    try { setResult(await api.importSpotifyPlaylists([...selected])) }
    catch (reason) { setError(reason instanceof Error ? reason.message : 'Spotify 歌单读取失败') }
    finally { setBusy(false) }
  }

  return <div className="modal-backdrop"><section className="export-modal spotify-picker" role="dialog" aria-modal="true" aria-label="选择 Spotify 歌单"><header className="modal-header"><div><span className="eyebrow">SPOTIFY · OFFICIAL API</span><h2>{result ? '确认导入的曲目' : '选择可访问的歌单'}</h2></div><button className="icon-button" onClick={onClose}>×</button></header><div className="spotify-restriction"><strong>合规边界</strong><span>这里只读取你主动选择的歌单用于传输与写回。Spotify 内容不会进入 LLM、画像、相似度或推荐计算。</span></div>{busy && <p className="picker-loading">正在通过 Spotify 官方 API 读取…</p>}{error && <p className="error-box">{error}</p>}{!busy && !result && <><div className="spotify-playlist-list">{playlists.map((playlist) => <label className={selected.has(playlist.id) ? 'spotify-playlist selected' : 'spotify-playlist'} key={playlist.id}><input type="checkbox" checked={selected.has(playlist.id)} onChange={() => toggle(playlist.id)}/>{playlist.image_url ? <img src={playlist.image_url} alt=""/> : <span className="playlist-placeholder">♫</span>}<span><strong>{playlist.name}</strong><small>{playlist.owner_name} · {playlist.track_count} 首{playlist.collaborative ? ' · 协作歌单' : ''}</small></span>{playlist.spotify_url && <a href={playlist.spotify_url} target="_blank" rel="noreferrer" onClick={(event) => event.stopPropagation()}>Spotify ↗</a>}</label>)}{playlists.length === 0 && <p className="empty-row">当前账号没有 API 可访问的歌单，或开发者应用权限仍受限。</p>}</div><div className="modal-actions"><button className="secondary" onClick={onClose}>取消</button><button className="primary" disabled={selected.size === 0 || busy} onClick={() => void importSelected()}>确认读取 {selected.size} 个歌单</button></div></>}{result && <><div className="import-summary"><strong>{result.track_count}</strong><span>首去重曲目已转换为统一 Track 结构</span></div><p className="policy-box">{result.policy_notice}</p><div className="spotify-track-preview">{result.tracks.slice(0, 10).map((track, index) => <div key={track.id}><span>{String(index + 1).padStart(2, '0')}</span><div><strong>{track.title}</strong><small>{track.artists.join(', ')}{track.album ? ` · ${track.album}` : ''}</small></div>{track.platform_url && <a href={track.platform_url} target="_blank" rel="noreferrer">Spotify ↗</a>}</div>)}</div>{result.track_count > 10 && <p className="fine-print">这里只预览前 10 首；下一步可以逐首取消选择，并在实际创建前再次确认。</p>}<p className="fine-print">{result.attribution}</p><div className="modal-actions"><button className="secondary" onClick={() => setResult(null)}>返回重选</button><button className="primary" disabled={result.tracks.length === 0} onClick={() => { onExport('spotify', result.tracks, result.playlists.map((playlist) => playlist.name).join(' + ')); onClose() }}>预览并创建新的 Spotify 歌单</button></div></>}</section></div>
}

function GenreNode({ x, y, name, tone }: { x: string; y: string; name: string; tone: string }) { return <div className={`genre-node ${tone}`} style={{ left: x, top: y }}><span/><strong>{name}</strong></div> }

function TastePage({ personal, canExplore, onExplore }: { personal: PersonalAnalysis; canExplore: boolean; onExplore: () => void }) {
  const { report, playlist, import_summary: summary, unmatched_tracks: unmatched } = personal
  const maxGenre = Math.max(...report.genre_distribution.map((item) => item[1]), 1)
  return <div className="page-width report-page">
    <PageIntro eyebrow={summary?.data_state === 'REAL_FILE' ? 'REAL FILE ANALYSIS' : summary?.data_state === 'REAL_TEXT' ? 'REAL TEXT ANALYSIS' : 'PERSONAL TASTE MAP'} title={report.playlist_name} copy={report.summary} badge={report.is_demo ? 'DEMO DATA' : `${report.source_label} · is_demo=false`} />
    {summary && <section className="real-analysis-summary panel"><div className="panel-title"><div><span className="eyebrow">VERIFIED REAL INPUT</span><h3>真实数据分析覆盖</h3></div><strong>{Math.round(summary.genre_coverage * 100)}% Genre 覆盖率</strong></div><div className="import-stats analysis"><span><b>{summary.input_count}</b>输入总数</span><span><b>{summary.parsed_count}</b>成功解析</span><span><b>{summary.analyzed_count}</b>实际参与分析</span><span><b>{summary.complete_metadata_count}</b>完整元数据</span><span><b>{summary.partial_metadata_count}</b>部分元数据</span><span><b>{summary.unmatched_count}</b>未匹配</span></div><p>数据来源：{summary.source_label} · Genre：{summary.genre_matched_count}/{summary.analyzed_count} · Energy：{summary.energy_matched_count}/{summary.analyzed_count}</p></section>}
    <div className="metric-grid">{report.metrics.map((metric) => <MetricCard key={metric.label} {...metric} />)}</div>
    {!report.is_demo && <div className="basic-stat-grid"><span><b>{report.album_distribution.length}</b>专辑数</span><span><b>{report.duplicate_track_count}</b>重复歌曲</span><span><b>{report.collaboration_track_count}</b>合作歌曲</span><span><b>{report.artist_distribution.length}</b>歌手数</span></div>}
    <div className="report-grid"><section className="panel chart-panel"><div className="panel-title"><div><span className="eyebrow">GENRE SIGNAL</span><h3>你的声音地形</h3></div><span>共 {report.track_count} 首</span></div><div className="bar-chart">{report.genre_distribution.slice(0, 7).map(([genre, count]) => <div className="bar-row" key={genre}><span>{genre}</span><div><i style={{ width: `${count / maxGenre * 100}%` }}/></div><b>{count}</b></div>)}</div></section><section className="panel zones-panel"><div className="panel-title"><div><span className="eyebrow">TASTE ZONES</span><h3>核心、相邻与空白</h3></div></div><TasteZone label="核心区" values={report.core_preferences} tone="core"/><TasteZone label="相邻区" values={report.adjacent_preferences} tone="adjacent"/><TasteZone label="待探索" values={report.unexplored_preferences} tone="blank"/></section></div>
    <section className="panel source-tracks"><div className="panel-title"><div><span className="eyebrow">SOURCE TRACKS</span><h3>全部真实分析歌曲</h3></div><span>{playlist.tracks.length} 首 · 元数据置信度 {Math.round(report.confidence * 100)}%</span></div><div className="compact-track-grid">{playlist.tracks.map((track) => <TrackLine track={track} key={track.id}/>)}</div></section>
    {summary && unmatched.length > 0 && <section className="panel unmatched-section"><div className="panel-title"><div><span className="eyebrow">UNMATCHED TRACKS</span><h3>元数据暂未匹配</h3></div><strong>{unmatched.length} 首仍参与基础分析</strong></div>{unmatched.map((track, index) => <div className="unmatched-row" key={`${track.original_row}-${index}`}><span>{index + 1}</span><strong>{track.title}</strong><span>{track.artists.join(' / ')}</span><em>metadata_status=missing</em><small>{track.warnings.join('；')}</small></div>)}</section>}
    <div className="limitation-note"><strong>数据说明</strong>{report.limitations.map((item) => <span key={item}>{item}</span>)}</div>
    {canExplore && <div className="page-cta"><div><span className="eyebrow">NEXT: DISCOVERY ROUTE</span><h2>从熟悉出发，但不在熟悉处打转。</h2></div><button className="primary big" onClick={onExplore}>查看基于当前歌单的推荐 →</button></div>}
  </div>
}

function MetricCard({ label, display, explanation, value }: { label: string; display: string; explanation: string; value: number }) { return <article className="metric-card"><div className="ring" style={{ '--value': `${value * 360}deg` } as React.CSSProperties}><span>{display}</span></div><div><strong>{label}</strong><p>{explanation}</p></div></article> }
function TasteZone({ label, values, tone }: { label: string; values: string[]; tone: string }) { return <div className={`taste-zone ${tone}`}><strong>{label}</strong><div>{values.map((value) => <span key={value}>{value}</span>)}</div></div> }
function TrackLine({ track }: { track: Track }) { return <div className="track-line"><span className="album-placeholder">♪</span><span><strong>{track.title}</strong><small>{track.artists.join(', ')} · {track.album ?? '专辑未知'} · {track.release_year ?? '年份未知'}</small></span><em>{track.genres[0] ?? '元数据暂未匹配'}</em></div> }

function RecommendationPage({ personal, onExport }: { personal: PersonalAnalysis; onExport: (platform: string, tracks: Track[], label: string) => void }) {
  const tracks = personal.recommendations.map((item) => item.track)
  const zones = ['舒适区', '拓展区', '惊喜区']
  const routeLabel = personal.route.map((step) => step.genre).join(' → ') || '音乐探索路线'
  const summary = personal.recommendation_summary
  const stats = summary.query_stats
  return <div className="page-width">
    <PageIntro eyebrow="EXPLAINABLE RECOMMENDATION" title="一条听得懂的探索路线" copy="真实模式以 Last.fm 听众相似关系、相似艺术家和关联标签生成候选，再由 Rust 本地评分；Genre 缺失不会淘汰强相似候选。" badge={`${tracks.length} TRACKS · ${personal.report.is_demo ? 'DEMO DATA' : 'REAL DATA · is_demo=false'}`} />
    <section className={`recommendation-status panel status-${summary.status}`}><div><span className="eyebrow">RECOMMENDATION SOURCE</span><h3>{summary.source_label}</h3><p>{summary.message}</p></div><div className="recommendation-coverage"><span><b>{personal.report.genre_matched_count}/{personal.report.track_count}</b>Genre 覆盖 · {Math.round(personal.report.genre_coverage * 100)}%</span><span><b>{personal.report.energy_matched_count}/{personal.report.track_count}</b>Energy 覆盖 · {Math.round(personal.report.energy_coverage * 100)}%</span><span><b>{summary.candidate_count}</b>真实候选</span><span><b>{personal.report.is_demo ? 'true' : 'false'}</b>is_demo</span></div><small>当前数据来源：{personal.report.source_label}</small></section>
    {!personal.report.is_demo && <section className="panel recommendation-evidence"><div><span className="eyebrow">SELECTED SEEDS</span><h3>实际采用的种子歌曲 · {summary.seeds.length} 首</h3><div className="seed-list">{summary.seeds.map((seed) => <span key={`${seed.title}-${seed.artists.join('-')}`}><b>{seed.title}</b><small>{seed.artists.join(', ')}</small></span>)}</div></div><div><span className="eyebrow">LAST.FM QUERY REPORT</span><div className="query-stats"><span><b>{stats.successful_seed_count}</b>成功种子</span><span><b>{stats.failed_seed_count}</b>失败种子</span><span><b>{stats.raw_track_similar_count}</b>track.getSimilar</span><span><b>{stats.raw_artist_similar_count}</b>artist.getSimilar</span><span><b>{stats.raw_tag_top_tracks_count}</b>tag.getTopTracks</span><span><b>{stats.raw_candidate_count}</b>原始歌曲候选</span><span><b>{stats.deduplicated_candidate_count}</b>去重及排除后</span></div></div></section>}
    {tracks.length > 0 && <ExportToolbar tracks={tracks} label={routeLabel} onExport={onExport}/>} 
    {personal.route.length > 0 ? <section className="route-panel"><div className="route-line"/>{personal.route.map((step, index) => <div className="route-step" key={`${step.genre}-${index}`}><span className="step-number">0{index + 1}</span><div><strong>{step.genre}</strong><p>{step.explanation}</p><small>{step.tracks.map((track) => track.title).join(' · ')}</small></div>{index < personal.route.length - 1 && <b>→</b>}</div>)}</section> : <div className="zone-empty route-empty">暂无可验证的 Genre 路线；{summary.message}</div>}
    {zones.map((zone) => { const items = personal.recommendations.filter((item) => item.zone === zone); const zoneSummary = summary.zones.find((item) => item.zone === zone); return <section className={`recommend-zone zone-${zone}`} key={zone}><div className="zone-heading"><span>{zone === '舒适区' ? '01' : zone === '拓展区' ? '02' : '03'}</span><div><h2>{zone} <small>{items.length} 首</small></h2><p>{zone === '舒适区' ? '延续现有偏好，低风险找到新歌' : zone === '拓展区' ? '保留熟悉锚点，引入新的音乐语言' : '差异更大，但每一步都有连接依据'}</p></div></div>{items.length > 0 ? <div className="recommend-grid">{items.map((item) => <RecommendationCard item={item} key={item.track.id}/>)}</div> : <div className="zone-empty"><strong>暂无足够的真实候选</strong><span>{zoneSummary?.message ?? summary.message}</span></div>}</section> })}
  </div>
}

function RecommendationCard({ item }: { item: Recommendation }) { return <article className="recommend-card"><div className="recommend-top"><span className="album-art">{item.track.title.slice(0, 1)}</span><div><h3>{item.track.title}</h3><p>{item.track.artists.join(', ')}</p></div>{item.track.platform_url && <a href={item.track.platform_url} target="_blank" rel="noreferrer" aria-label="Last.fm 曲目页面">↗</a>}</div><div className="tag-row">{item.tags.length > 0 ? item.tags.map((tag) => <span key={tag}>{tag}</span>) : <span>标签未返回</span>}</div><p className="candidate-source">{item.candidate_source} · {item.source_endpoint} · 置信度 {Math.round(item.match_confidence * 100)}% · 放宽级别 {item.relaxation_level}</p>{(item.seed_track || item.seed_artist) && <p className="candidate-seed">关联种子：{item.seed_track ? `《${item.seed_track}》` : item.seed_artist}{item.lastfm_similarity != null ? ` · Last.fm 相似度 ${Math.round(item.lastfm_similarity * 100)}%` : ''}</p>}<p className="reason">{item.reason}</p><div className="explain-pair"><div><small>连接依据</small><span>{item.connection}</span></div><div><small>拓展方向</small><span>{item.expansion}</span></div></div><div className="score-row"><Score label="匹配" value={item.match_score}/><Score label="新颖" value={item.novelty_score}/></div></article> }
function Score({ label, value }: { label: string; value: number }) { return <div><span>{label}</span><i><b style={{ width: `${value * 100}%` }}/></i><strong>{Math.round(value * 100)}</strong></div> }

function ComparePage({ report, onExport }: { report: ComparisonReport; onExport: (platform: string, tracks: Track[], label: string) => void }) {
  const tracks = report.bridge_playlist.map((item) => item.track)
  return <div className="page-width"><PageIntro eyebrow="CROSS-PLATFORM FRIEND MATCH" title="两种品味，一座声音桥梁" copy={report.summary} badge="A × B · DEMO"/><div className="people-card"><Person label={report.user_a} initials="A" values={report.user_a_signatures} tone="coral"/><div className="compatibility"><span>品味相似度</span><strong>46</strong><small>%</small><i>互补度 81%</i></div><Person label={report.user_b} initials="B" values={report.user_b_signatures} tone="blue"/></div><div className="metric-grid comparison-metrics">{report.metrics.map((metric) => <MetricCard key={metric.label} {...metric}/>)}</div><section className="panel bridge-section"><div className="panel-title"><div><span className="eyebrow">BRIDGE PLAYLIST</span><h3>A ↔ B 的桥梁歌单</h3></div><span>变化平滑 · 双方均衡</span></div><ExportToolbar tracks={tracks} label="A 与 B 的桥梁歌单" onExport={onExport}/><div className="bridge-list">{report.bridge_playlist.map((item, index) => <BridgeRow item={item} index={index} key={item.track.id}/>)}</div></section></div>
}
function Person({ label, initials, values, tone }: { label: string; initials: string; values: string[]; tone: string }) { return <div className={`person ${tone}`}><span>{initials}</span><div><strong>{label}</strong><p>{values.join(' · ')}</p></div></div> }
function BridgeRow({ item, index }: { item: BridgeTrack; index: number }) { return <div className="bridge-row"><span className="bridge-index">{String(index + 1).padStart(2, '0')}</span><span className="album-placeholder">♪</span><div className="bridge-track"><strong>{item.track.title}</strong><small>{item.track.artists.join(', ')} · {item.track.genres.join(' / ')}</small></div><span className="phase-chip">{item.phase}</span><p>{item.reason}</p><b>{Math.round(item.bridge_score * 100)}</b></div> }

function ExportToolbar({ tracks, label, onExport }: { tracks: Track[]; label: string; onExport: (platform: string, tracks: Track[], label: string) => void }) { return <div className="export-toolbar"><div><span className="eyebrow">SAVE YOUR PATH</span><strong>选择歌曲，预览匹配，再创建新歌单</strong></div><div><button className="spotify-button" onClick={() => onExport('spotify', tracks, label)}>● 保存到 Spotify</button><button onClick={() => onExport('apple_music', tracks, label)}>♪ 保存到 Apple Music</button><button onClick={() => onExport('youtube', tracks, label)}>▶ 保存到 YouTube</button><button className="export-button" onClick={() => onExport('file', tracks, label)}>⇩ 导出歌曲清单</button><button className="demo-flow-button" onClick={() => onExport('demo', tracks, label)}>演示写入流程</button></div></div> }

function AgentPage() {
  const [task, setTask] = useState<AgentTask | null>(null)
  const [scenario, setScenario] = useState<AgentTask['scenario']>('personal_exploration')
  const [error, setError] = useState('')
  useEffect(() => {
    if (!task || ['completed', 'failed', 'cancelled'].includes(task.status)) return
    const source = new EventSource(`/api/tasks/${task.id}/events`)
    source.addEventListener('task', (event) => setTask(JSON.parse((event as MessageEvent).data) as AgentTask))
    source.onerror = () => source.close()
    return () => source.close()
  }, [task?.id, task?.status])
  const start = async () => { setError(''); try { setTask(await api.createTask(scenario)) } catch (reason) { setError(reason instanceof Error ? reason.message : '无法创建任务') } }
  const cancel = async () => { if (task) setTask(await api.cancelTask(task.id)) }
  return <div className="page-width"><PageIntro eyebrow="RUST AGENT LOOP" title="看见 Agent 如何做决定" copy="这不是前端动画：每一步由 Rust 后端调度、写入 SQLite，并通过 SSE 实时推送。可随时中断，终态与资源用量会保留在历史中。" badge="LIVE SSE"/><div className="agent-layout"><section className="panel agent-control"><h3>选择音乐场景</h3><label className={scenario === 'personal_exploration' ? 'scenario active' : 'scenario'}><input type="radio" checked={scenario === 'personal_exploration'} onChange={() => setScenario('personal_exploration')}/><span>01</span><div><strong>个人 Genre 探索</strong><p>Korean R&B → Alternative R&B → Neo Soul</p></div></label><label className={scenario === 'friend_bridge' ? 'scenario active' : 'scenario'}><input type="radio" checked={scenario === 'friend_bridge'} onChange={() => setScenario('friend_bridge')}/><span>02</span><div><strong>跨平台好友桥梁</strong><p>计算重合、互补与平滑过渡歌单</p></div></label><button className="primary big full" disabled={task?.status === 'running'} onClick={start}>启动 Rust Agent</button>{error && <p className="error-box">{error}</p>}</section><section className="panel agent-terminal"><div className="terminal-head"><span><i/> agent.runtime</span>{task && <code>{task.id.slice(0, 8)}</code>}</div>{!task ? <div className="terminal-empty"><Logo/><p>选择场景并启动，执行轨迹将在这里实时出现。</p></div> : <><div className="progress-orbit"><div className="progress-number">{Math.round(task.progress * 100)}<small>%</small></div><svg viewBox="0 0 120 120"><circle cx="60" cy="60" r="52"/><circle className="progress-value" cx="60" cy="60" r="52" style={{ strokeDashoffset: 327 - 327 * task.progress }}/></svg></div><div className="agent-message"><span>STEP {String(task.current_step).padStart(2, '0')}</span><h3>{task.message}</h3><p>{task.goal}</p></div><div className="usage-grid"><span><small>状态</small><strong>{task.status}</strong></span><span><small>Input tokens</small><strong>{task.input_tokens}</strong></span><span><small>Output tokens</small><strong>{task.output_tokens}</strong></span><span><small>估算费用</small><strong>${task.estimated_cost_usd.toFixed(4)}</strong></span></div>{task.status === 'running' && <button className="danger-button" onClick={cancel}>中断任务</button>}{task.error && <p className="error-box">{task.error}</p>}{task.status === 'completed' && <p className="success-box">结果已持久化，可在“历史”中重新加载。Demo 未调用 LLM，因此 Token 和费用如实为 0。</p>}</>}</section></div></div>
}

function HistoryPage() {
  const [tasks, setTasks] = useState<AgentTask[]>([])
  const [selected, setSelected] = useState<AgentTask | null>(null)
  const [error, setError] = useState('')
  const load = () => api.tasks().then(setTasks).catch((reason: unknown) => setError(reason instanceof Error ? reason.message : '读取失败'))
  useEffect(() => { load() }, [])
  const totals = useMemo(() => tasks.reduce((sum, task) => ({ tokens: sum.tokens + task.input_tokens + task.output_tokens, cost: sum.cost + task.estimated_cost_usd }), { tokens: 0, cost: 0 }), [tasks])
  return <div className="page-width"><PageIntro eyebrow="PERSISTED HISTORY" title="任务不会随页面消失" copy="任务状态、目标、执行步数、结果、Token 与费用保存在 SQLite。服务重启时，未完成任务会被明确标记为中断。" badge="SQLITE"/><div className="history-summary"><span><strong>{tasks.length}</strong>历史任务</span><span><strong>{totals.tokens}</strong>累计 Tokens</span><span><strong>${totals.cost.toFixed(4)}</strong>估算费用</span><button className="secondary" onClick={load}>刷新</button></div>{error && <p className="error-box">{error}</p>}<section className="panel history-table"><div className="table-row table-head"><span>场景 / 目标</span><span>状态</span><span>步骤</span><span>Tokens</span><span>费用</span><span/></div>{tasks.map((task) => <button className="table-row" onClick={() => setSelected(task)} key={task.id}><span><strong>{task.scenario === 'personal_exploration' ? '个人 Genre 探索' : '好友桥梁歌单'}</strong><small>{task.goal}</small></span><span><i className={`task-status ${task.status}`}/>{task.status}</span><span>{task.current_step} / {task.max_steps}</span><span>{task.input_tokens + task.output_tokens}</span><span>${task.estimated_cost_usd.toFixed(4)}</span><span>查看 →</span></button>)}{tasks.length === 0 && <div className="empty-row">还没有历史任务。到“Agent 运行”启动一个场景。</div>}</section>{selected && <section className="panel history-detail"><button className="icon-button" onClick={() => setSelected(null)}>×</button><span className="eyebrow">SAVED RESULT</span><h3>{selected.goal}</h3><p>{selected.message}</p>{selected.error && <p className="error-box">{selected.error}</p>}<pre>{selected.result_json ? JSON.stringify(JSON.parse(selected.result_json), null, 2).slice(0, 4000) : '该任务没有结果数据。'}</pre></section>}</div>
}

function SettingsPage() {
  const [settings, setSettings] = useState<AgentSettings | null>(null)
  const [message, setMessage] = useState('')
  const [error, setError] = useState('')
  useEffect(() => { api.settings().then(setSettings).catch((reason: unknown) => setError(reason instanceof Error ? reason.message : '读取失败')) }, [])
  if (!settings) return <div className="page-width"><p>{error || '正在读取设置…'}</p></div>
  const number = (key: keyof AgentSettings, value: string) => setSettings({ ...settings, [key]: Number(value) })
  const save = async () => { setError(''); setMessage(''); try { setSettings(await api.saveSettings(settings)); setMessage('配置已保存到 SQLite。API Key 始终只从环境变量读取。') } catch (reason) { setError(reason instanceof Error ? reason.message : '保存失败') } }
  return <div className="page-width"><PageIntro eyebrow="AGENT CONFIGURATION" title="模型、预算与运行边界" copy="模型配置可保存；密钥不会出现在网页或数据库。离线 Demo 始终可运行，启用真实 LLM 后才会产生 Token 与费用。" badge={settings.api_key_available ? 'API KEY READY' : 'DEMO ONLY'}/><section className="panel settings-form"><div className="settings-grid"><label className="field"><span>OpenAI-compatible Endpoint</span><input value={settings.endpoint} onChange={(e) => setSettings({ ...settings, endpoint: e.target.value })}/></label><label className="field"><span>模型名称</span><input value={settings.model} onChange={(e) => setSettings({ ...settings, model: e.target.value })}/></label><label className="field"><span>Temperature</span><input type="number" min="0" max="2" step="0.1" value={settings.temperature} onChange={(e) => number('temperature', e.target.value)}/></label><label className="field"><span>最大输出 Tokens</span><input type="number" min="1" value={settings.max_tokens} onChange={(e) => number('max_tokens', e.target.value)}/></label><label className="field"><span>最大 Agent 步数</span><input type="number" min="12" max="64" value={settings.max_agent_steps} onChange={(e) => number('max_agent_steps', e.target.value)}/></label><label className="field"><span>请求超时（秒）</span><input type="number" min="2" max="300" value={settings.request_timeout_seconds} onChange={(e) => number('request_timeout_seconds', e.target.value)}/></label><label className="field"><span>失败重试上限</span><input type="number" min="0" max="5" value={settings.retry_limit} onChange={(e) => number('retry_limit', e.target.value)}/></label><label className="field"><span>单任务费用上限（USD）</span><input type="number" min="0" step="0.01" value={settings.max_cost_usd} onChange={(e) => number('max_cost_usd', e.target.value)}/></label><label className="field"><span>输入价格 / 百万 Tokens</span><input type="number" min="0" step="0.01" value={settings.input_price_per_million} onChange={(e) => number('input_price_per_million', e.target.value)}/></label><label className="field"><span>输出价格 / 百万 Tokens</span><input type="number" min="0" step="0.01" value={settings.output_price_per_million} onChange={(e) => number('output_price_per_million', e.target.value)}/></label></div><div className="key-status"><span className={`status-dot ${settings.api_key_available ? 'online' : ''}`}/><div><strong>OPENAI_API_KEY：{settings.api_key_available ? '环境变量已配置' : '未配置'}</strong><p>本页永远不接收或展示 API Key。</p></div></div>{message && <p className="success-box">{message}</p>}{error && <p className="error-box">{error}</p>}<button className="primary" onClick={save}>保存运行配置</button></section></div>
}

function PageIntro({ eyebrow, title, copy, badge }: { eyebrow: string; title: string; copy: string; badge: string }) { return <header className="page-intro"><div><span className="eyebrow">{eyebrow}</span><h1>{title}</h1><p>{copy}</p></div><span className="page-badge">{badge}</span></header> }
