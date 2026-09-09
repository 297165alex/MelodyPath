import { useSpotifyWrite } from './useSpotifyWrite'
import { readPlaylistFile } from './importFile'
import { useEffect, useMemo, useRef, useState } from 'react'
import { api } from './api'
import ExportModal from './ExportModal'
import { TaskProgress, type TaskProgressState } from './TaskProgress'
import type { AgentDecision, AgentPlan, AgentSettings, AgentTask, AlternateVersionSearchResult, BridgeTrack, ComparisonReport, DataState, DemoPayload, ImportPreview, ImportPreviewRequest, PersonalAnalysis, PlatformCapability, PlaylistLinkInspection, ProviderConfigurationStatus, RankingItem, Recommendation, ReleaseRadarResult, ReleaseUpdate, SpotifyConnectionStatus, SpotifyAnalysisPreview, SpotifyPlaylistSummary, Track, TransferPreview, TransferResult, TransferRun, VersionType, WriterStatus, YouTubeConnectionStatus, YouTubeImportResult, YouTubePlaylistSummary } from './types'

type Tab = 'home' | 'taste' | 'recommend' | 'compare' | 'versions' | 'transfer' | 'agent' | 'history' | 'settings'
type ExportTarget = { platform: string; tracks: Track[]; label: string }

const navItems: { id: Tab; label: string }[] = [
  { id: 'home', label: '开始' }, { id: 'taste', label: '品味地图' }, { id: 'recommend', label: '探索推荐' },
  { id: 'compare', label: '好友桥梁' }, { id: 'versions', label: '版本雷达' }, { id: 'transfer', label: '跨平台复制' }, { id: 'agent', label: 'Agent 运行' }, { id: 'history', label: '历史' }, { id: 'settings', label: '设置' },
]

function tabForPath(path: string): Tab { if (path === '/analysis') return 'taste'; if (path === '/discover') return 'recommend'; if (path === '/agent') return 'agent'; if (path === '/compare') return 'compare'; if (path === '/versions') return 'versions'; if (path === '/transfer') return 'transfer'; return 'home' }

export default function App() {
  const [tab, setTab] = useState<Tab>(() => tabForPath(window.location.pathname))
  const [menuOpen, setMenuOpen] = useState(false)
  const menuButton = useRef<HTMLButtonElement>(null)
  const [demo, setDemo] = useState<DemoPayload | null>(null)
  const [statuses, setStatuses] = useState<WriterStatus[]>([])
  const [capabilities, setCapabilities] = useState<PlatformCapability[]>([])
  const [spotify, setSpotify] = useState<SpotifyConnectionStatus | null>(null)
  const [youtube, setYoutube] = useState<YouTubeConnectionStatus | null>(null)
  const [loading, setLoading] = useState(true)
  const [fatalError, setFatalError] = useState('')
  const [connectionErrors, setConnectionErrors] = useState<string[]>([])
  const [manualPersonal, setManualPersonal] = useState<PersonalAnalysis | null>(null)
  const [dataState, setDataState] = useState<DataState>('NONE')
  const [exportTarget, setExportTarget] = useState<ExportTarget | null>(null)
  const [oauthNotice, setOauthNotice] = useState<{ ok: boolean; message: string } | null>(null)
  const [comparisonBinding, setComparisonBinding] = useState<{ analysis_a_id: string; analysis_b_id: string } | null>(null)

  useEffect(() => {
    Promise.all([api.demo(), api.statuses(), api.capabilities(), api.spotifyMe().catch(() => { setConnectionErrors(current => [...current, 'Spotify Error · 无法核验后端会话，请检查服务后刷新。']); return null }), api.youtubeMe().catch(() => { setConnectionErrors(current => [...current, 'YouTube Error · 无法核验后端会话，请检查服务后刷新。']); return null })])
      .then(([payload, writerStatuses, platformCapabilities, spotifyStatus, youtubeStatus]) => {
        setDemo(payload); setStatuses(writerStatuses); setCapabilities(platformCapabilities); setSpotify(spotifyStatus); setYoutube(youtubeStatus)
      })
      .catch((reason: unknown) => setFatalError(reason instanceof Error ? reason.message : '后端连接失败'))
      .finally(() => setLoading(false))
  }, [])

  useEffect(() => {
    const onPopState = () => { setTab(tabForPath(window.location.pathname)); setMenuOpen(false) }
    window.addEventListener('popstate', onPopState)
    return () => window.removeEventListener('popstate', onPopState)
  }, [])

  const navigate = (next: Tab) => {
    if (menuOpen) menuButton.current?.focus()
    setMenuOpen(false)
    setTab(next)
    const path = next === 'taste' ? '/analysis' : next === 'recommend' ? '/discover' : next === 'agent' ? '/agent' : next === 'compare' ? '/compare' : next === 'versions' ? '/versions' : next === 'transfer' ? '/transfer' : '/'
    if (window.location.pathname !== path) window.history.pushState({}, '', path)
  }

  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    const oauth = params.get('oauth')
    if (params.get('provider') === 'spotify' && oauth && window.opener) window.opener.postMessage({ type: 'melody-spotify-oauth', status: oauth }, window.location.origin)
    const provider = params.get('provider') === 'youtube' ? 'YouTube' : 'Spotify'
    if (oauth === 'connected') {
      setOauthNotice({ ok: false, message: `${provider}：正在向后端核验连接状态…` })
      const verifySession = async () => {
        try {
          const status = provider === 'YouTube' ? await api.youtubeMe() : await api.spotifyMe()
          if (provider === 'YouTube') setYoutube(status as YouTubeConnectionStatus)
          else setSpotify(status as SpotifyConnectionStatus)
          setOauthNotice({ ok: status.connected, message: status.connected ? `${provider} Connected · 后端已确认授权会话和身份。` : `${provider} Error · ${status.message}` })
        } catch {
          setOauthNotice({ ok: false, message: `${provider} Error · 无法核验后端会话，请检查服务后重试；没有判定为已连接。` })
        }
      }
      void verifySession()
      window.history.replaceState({}, '', '/')
    } else if (oauth === 'error') {
      const messages: Record<string, string> = {
        youtube_token_timeout: 'Google 授权码交换超时：后端未能及时连接 token endpoint，请检查本机代理是否运行后重新连接。',
        youtube_token_network: 'Google 授权码交换网络失败：请检查后端的 YouTube 代理或网络连接，再重新授权。',
        youtube_token_rejected: 'Google 拒绝了授权码交换，请检查 Client 配置与回调地址，并重新发起授权。',
        youtube_token_response_invalid: 'Google 授权码交换返回了无效响应，未建立会话，请重新连接。',
        youtube_channel_required: '授权码交换成功，但当前账号没有可访问的 YouTube 频道。请先在 YouTube 建立频道后重试。',
        youtube_identity_failed: '授权码交换成功，但 YouTube 频道身份读取失败，请检查网络、YouTube Data API 和只读权限。',
        youtube_session_save_failed: 'YouTube 身份验证成功，但本机加密会话保存失败；未建立浏览器连接，请检查本机存储权限。',
        config_required: 'CONFIG_REQUIRED：请先在配置向导检查开发者配置和回调地址。',
        state_mismatch: '授权安全校验失败或已过期，请在同一浏览器从连接按钮重新开始。',
        provider_error: '平台拒绝了授权请求，请在官方页面检查应用配置、账号资格和授权范围后重试。',
        permission_denied: 'YouTube 读取失败，请检查 API 是否启用、OAuth 权限和额度。',
        channel_required: '当前账号没有可访问的 YouTube 频道，请检查账号后重新授权。',
        authorization_cancelled: '授权已取消，没有保存连接。',
        missing_code: '授权回调缺少授权码，请从“连接”按钮重新开始。',
        missing_state: '授权回调缺少安全校验，请从“连接”按钮重新开始。',
        authorization_failed: '授权未能完成，可能已过期或配置不一致，请重新连接。',
      }
      setOauthNotice({ ok: false, message: `${provider}：${messages[params.get('reason') ?? ''] ?? '授权未完成，请重新连接。'}` })
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
    : dataState === 'REAL_FILE' || dataState === 'REAL_TEXT' || dataState === 'REAL_PUBLIC_LINK' || dataState === 'REAL_ACCOUNT'
      ? manualPersonal
      : null

  if (loading) return <div className="loading-screen"><Logo /><span>正在唤醒音乐路径…</span></div>
  if (fatalError || !demo) return <div className="loading-screen error-screen"><Logo /><h1>暂时无法连接 Rust 后端</h1><p>{fatalError}</p><code>cargo run -p melody-path-api</code></div>

  return <div className="app-shell">
    <header className="topbar" onKeyDown={(event) => { if (event.key === 'Escape' && menuOpen) { setMenuOpen(false); menuButton.current?.focus() } }}>
      <button className="brand" onClick={() => navigate('home')}><Logo /><span><strong>MelodyPath</strong><small>可解释音乐探索 Agent</small></span></button>
      <button ref={menuButton} className="mobile-menu-toggle" aria-expanded={menuOpen} aria-controls="main-navigation" onClick={() => setMenuOpen(open => !open)}><span aria-hidden="true">{menuOpen ? '×' : '☰'}</span>{menuOpen ? '关闭菜单' : '菜单'}</button>
      <nav id="main-navigation" className={menuOpen ? 'is-open' : ''} aria-label="主导航">{navItems.map((item) => <button className={tab === item.id ? 'active' : ''} aria-current={tab === item.id ? 'page' : undefined} onClick={() => navigate(item.id)} key={item.id}>{item.label}<small className="mobile-nav-description">{({ home: 'Import · 导入歌单', taste: 'Music Profile · 了解音乐偏好', recommend: 'Discover · 发现新音乐', compare: 'Compare · 比较两份歌单', versions: 'Version Radar · 发现不同版本', transfer: 'Copy Playlist · 复制歌单', agent: 'Agent · 查看任务执行过程', history: 'History · 回看任务结果', settings: 'Settings · 调整运行配置' })[item.id]}</small></button>)}</nav>
      <div className="header-status"><span className="pulse" />Rust API 在线</div>
    </header>

    <main>
      {[...new Set(connectionErrors)].map(message => <p className="error-box" role="alert" key={message}>{message}</p>)}
      {oauthNotice && <div role={oauthNotice.ok ? 'status' : 'alert'} aria-live="polite" className={oauthNotice.ok ? 'success-box oauth-notice' : 'error-box oauth-notice'}>{oauthNotice.message}<button className="text-button" onClick={() => setOauthNotice(null)}>关闭</button></div>}
      {tab === 'home' && <Home demo={demo} capabilities={capabilities} spotify={spotify} youtube={youtube} onRefreshConnections={refreshConnections} onDemo={() => { setManualPersonal(null); setDataState('DEMO'); navigate('taste') }} onManual={(personal, state) => { setManualPersonal(personal); setDataState(state); navigate('taste'); window.scrollTo({ top: 0 }) }} onImportError={() => { setManualPersonal(null); setDataState('ERROR') }} onCompare={() => navigate('compare')} onTransfer={() => navigate('transfer')} onAgent={() => navigate('agent')} onVersions={() => navigate('versions')} onDiscover={() => activePersonal ? navigate('recommend') : document.getElementById('more-import')?.scrollIntoView({ behavior: 'smooth' })} onExport={openExport} />}
      {tab === 'taste' && activePersonal && <TastePage personal={activePersonal} canExplore={Boolean(activePersonal.recommendation_summary)} spotifyConnected={Boolean(spotify?.connected)} onExplore={() => navigate('recommend')} onExport={openExport} />}
      {tab === 'taste' && !activePersonal && <div className="page-width"><PageIntro eyebrow="Music Profile" title="先导入歌单，了解你的音乐偏好" copy="了解你的音乐风格、常听歌手和偏好。上传文件或粘贴歌曲列表，核对预览并确认后，就能在这里查看音乐画像。" badge="等待导入"/><button className="primary" onClick={() => navigate('home')}>返回首页导入歌单</button></div>}
      {tab === 'recommend' && activePersonal && <RecommendationPage personal={activePersonal} spotifyConnected={Boolean(spotify?.connected)} onExport={openExport} />}
      {tab === 'recommend' && !activePersonal && <div className="page-width"><PageIntro eyebrow="DISCOVER" title="先导入并确认一份歌单" copy="当前页面尚未绑定分析。返回首页上传文件或粘贴歌曲，确认分析后即可查看推荐。" badge="NO ANALYSIS"/><button className="primary" onClick={() => navigate('home')}>返回首页导入歌单</button></div>}
      {tab === 'compare' && <ComparePage demoReport={demo.comparison} capabilities={capabilities} onExport={openExport} onBinding={setComparisonBinding} />}
      {tab === 'versions' && <VersionsPage currentAnalysis={activePersonal} youtube={youtube} onAddPreview={(track) => openExport('youtube', [track], `${track.title} · 版本雷达`)} />}
      {tab === 'transfer' && <TransferPage spotify={spotify} youtube={youtube} capabilities={capabilities} currentAnalysis={activePersonal} />}
      {tab === 'agent' && <AgentPage currentAnalysis={activePersonal} comparisonBinding={comparisonBinding} />}
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

function LinkImportStatus({ result }: { result: PlaylistLinkInspection }) {
  const hasTracks = result.capability === 'TRACK_IMPORT_AVAILABLE' && result.preview_tracks.length > 0
  const chinaPlatform = ['netease', 'qq_music', 'kugou', 'qishui'].includes(result.platform ?? '')
  const declaredCount = result.declared_count ?? result.track_count
  const visibleCount = result.visible_count ?? result.preview_tracks.length
  const importedCount = result.imported_count ?? result.preview_tracks.length
  const skippedCount = result.skipped_count ?? result.import_rows?.filter(row => row.import_status === 'SKIPPED_DETAIL_UNAVAILABLE').length ?? 0
  const unexposedCount = result.unexposed_count ?? Math.max(0, (declaredCount ?? visibleCount) - visibleCount)
  const neteaseReason = visibleCount > 20
      ? '当前公开页面解析限制，仅导入前20首歌曲用于分析。'
      : skippedCount > 0
        ? '部分公开歌曲缺少可验证元数据。'
        : '公开页面已提供可验证歌曲。'
  const accessibility = result.publicly_accessible === true ? '页面可访问（不代表歌曲已读取）'
    : result.publicly_accessible === false ? '页面不可访问'
      : result.access_status === 'check_failed' ? '检查失败，无法确认可访问性'
        : '未检查；链接识别或官方 API 读取不代表页面检查'
  return <div className="import-stages" aria-label="公开链接读取状态">
    <p><b>1 · URL Recognition / 链接识别</b><span>{result.recognized ? `${result.platform === 'netease' ? 'NetEase playlist detected · ' : ''}已识别公开歌单链接 · ${result.platform_label}` : '未识别为支持的公开歌单链接'}</span></p>
    <p><b>2 · Accessibility Check / 可访问性检查</b><span>{accessibility}</span></p>
    {result.platform === 'netease' && (result.playlist_name != null || declaredCount != null) && <p><b>公开页面元数据</b><span>{result.playlist_name ?? '歌单名称未知'} · 页面声明 {declaredCount ?? '未知'} 首 · 实际可见 {visibleCount} 首 · 导入失败 {skippedCount} 首 · 未公开 {unexposedCount} 首</span></p>}
    <p><b>3 · Track Import / 歌曲读取</b><span>{result.platform === 'netease' ? (hasTracks ? <>网易云歌单解析成功<br/>{unexposedCount > 0 ? <>公开页面仅提供部分歌曲，<br/>已导入 {importedCount} / {declaredCount ?? visibleCount} 首歌曲</> : <>已导入：{importedCount} / {declaredCount ?? visibleCount} 首歌曲<br/>{neteaseReason}</>}</> : <>检测到网易云歌单，<br/>但当前无法获取公开歌曲列表。<br/>请使用 TXT/CSV 导入。</>) : chinaPlatform && hasTracks ? `Imported ${importedCount}/${declaredCount ?? visibleCount} tracks（仅公开 HTML / JSON-LD）` : hasTracks ? `官方 API 已返回 ${result.track_count ?? result.preview_tracks.length} 首歌曲，请核对预览。` : chinaPlatform ? 'Playlist recognized but tracks unavailable. · ACCESSIBILITY_CHECK_ONLY' : '尚未获得可导入歌曲，请按下方提示完成配置、授权或重试。'}</span></p>
    {chinaPlatform && !hasTracks && <p className="import-next-step"><b>下一步：提供歌曲列表</b><span>上传 CSV / TXT / JSON / M3U，或直接粘贴：歌手 - 歌名。</span></p>}
  </div>
}

function Home({ demo, capabilities, spotify, youtube, onRefreshConnections, onDemo, onManual, onImportError, onCompare, onTransfer, onAgent, onVersions, onDiscover, onExport }: {
  demo: DemoPayload
  capabilities: PlatformCapability[]
  spotify: SpotifyConnectionStatus | null
  youtube: YouTubeConnectionStatus | null
  onRefreshConnections: () => Promise<void>
  onDemo: () => void
  onManual: (personal: PersonalAnalysis, state: 'REAL_FILE' | 'REAL_TEXT' | 'REAL_PUBLIC_LINK' | 'REAL_ACCOUNT') => void
  onImportError: () => void
  onCompare: () => void
  onTransfer: () => void
  onAgent: () => void
  onVersions: () => void
  onDiscover: () => void
  onExport: (platform: string, tracks: Track[], label: string) => void
}) {
  const [text, setText] = useState('BIBI - Kazino\nDEAN - instagram\nMariya Takeuchi - Plastic Love')
  const [name, setName] = useState('我的歌单')
  const [error, setError] = useState('')
  const [busy, setBusy] = useState(false)
  const [link, setLink] = useState('')
  const [linkBusy, setLinkBusy] = useState(false)
  const linkRequest = useRef(0)
  const [linkResult, setLinkResult] = useState<PlaylistLinkInspection | null>(null)
  const [moreOpen, setMoreOpen] = useState(true)
  const [spotifyPickerOpen, setSpotifyPickerOpen] = useState(false)
  const [youtubePickerOpen, setYoutubePickerOpen] = useState(false)
  const [configPlatform, setConfigPlatform] = useState<'spotify' | 'youtube' | 'apple' | null>(null)
  const [importPreview, setImportPreview] = useState<ImportPreview | null>(null)
  const [pendingImport, setPendingImport] = useState<ImportPreviewRequest | null>(null)
  const [importError, setImportError] = useState('')
  const [importProgress, setImportProgress] = useState<TaskProgressState | null>(null)
  const previewRef = useRef<HTMLElement | null>(null)

  useEffect(() => {
    if (!importPreview) return
    window.requestAnimationFrame(() => previewRef.current?.scrollIntoView({ behavior: 'smooth', block: 'start' }))
  }, [importPreview])

  const prepareImport = async (request: ImportPreviewRequest) => {
    const total = estimateImportRows(request.content, request.format)
    setMoreOpen(true); setBusy(true); setImportError(''); setImportPreview(null); setPendingImport(request); setImportProgress({ stage: 'parsing', detail: `${total} 行已提交` })
    try { setImportPreview(await api.previewImport(request)); setImportProgress({ stage: 'completed', detail: '解析完成，请核对预览' }) }
    catch (reason) { const message = reason instanceof Error ? reason.message : '无法识别内容'; onImportError(); setImportError(message); setImportProgress({ stage: 'failed', error: message }) }
    finally { setBusy(false) }
  }

  const confirmImport = async () => {
    if (!importPreview) return
    setBusy(true); setImportError(''); setImportProgress({ stage: 'resolving_metadata', detail: `${importPreview.parsed_count} 首歌曲` })
    try {
      const analysis = await api.analyzeImport(importPreview.id, stage => setImportProgress({ stage, detail: `${importPreview.parsed_count} 首歌曲` }))
      setImportProgress({ stage: 'completed', detail: '分析与推荐已完成' }); onManual(analysis, importPreview.data_state)
    } catch (reason) { const message = reason instanceof Error ? reason.message : 'Metadata 分析失败'; onImportError(); setImportError(message); setImportProgress({ stage: 'failed', error: message }) }
    finally { setBusy(false) }
  }

  const inspectLink = async () => {
    const requestId = ++linkRequest.current
    setLinkBusy(true); setError(''); setLinkResult(null); setImportPreview(null); setPendingImport(null); setImportProgress({ stage: 'parsing', detail: '正在检查公开页面与可验证曲目' })
    try {
      const result = await api.inspectPlaylistLink(link)
      if (requestId !== linkRequest.current) return
      setLinkResult(result); setImportProgress({ stage: 'completed', detail: result.imported_count > 0 ? `Imported ${result.imported_count}/${result.declared_count ?? result.visible_count} tracks` : 'Playlist recognized but tracks unavailable.' })
      if (['netease', 'qq_music', 'kugou'].includes(result.platform ?? '') && result.can_analyze && result.import_preview) {
        setImportPreview(result.import_preview); setMoreOpen(true); setImportError('')
      }
    }
    catch (reason) { if (requestId !== linkRequest.current) return; const message = reason instanceof TypeError ? '公开歌单链接检查未能连接后端，请确认服务已启动或使用本地文件导入。' : reason instanceof Error ? `公开歌单链接检查失败：${reason.message}` : '链接检查失败，请使用本地文件导入。'; setError(message); setImportProgress({ stage: 'failed', error: message }) }
    finally { if (requestId === linkRequest.current) setLinkBusy(false) }
  }

  const confirmLink = () => {
    if (!linkResult || linkResult.capability !== 'TRACK_IMPORT_AVAILABLE' || linkResult.preview_tracks.length === 0) return
    onExport('youtube', linkResult.preview_tracks, linkResult.playlist_name ?? '官方歌单导入')
  }

  const loadFile = async (file?: File) => {
    if (!file) return
    setBusy(true); setImportError(''); setImportProgress({ stage: 'uploading', detail: file.name }); setMoreOpen(true)
    try {
      const raw = await readPlaylistFile(file)
      setName(file.name.replace(/\.[^.]+$/, ''))
      const format = file.name.split('.').pop()?.toLowerCase() ?? ''
      await prepareImport({ name: file.name.replace(/\.[^.]+$/, ''), file_name: file.name, format, content: raw, data_state: 'REAL_FILE' })
    } catch (reason) { const message = reason instanceof Error ? reason.message : '无法识别内容'; onImportError(); setImportError(message); setImportProgress({ stage: 'failed', error: message }); setBusy(false) }
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
    <section className="welcome-section" aria-labelledby="welcome-title">
      <span className="eyebrow">Welcome to MelodyPath 🎵</span><h2 id="welcome-title">从你的歌单出发，发现下一首喜欢的音乐</h2>
      <p>了解自己的听歌偏好，找到值得尝试的新风格，也看清每一次推荐和任务是怎样完成的。</p>
      <ol className="welcome-steps"><li><b>1 · 导入你的歌单</b><span>上传文件，或粘贴歌曲列表</span></li><li><b>2 · 了解你的音乐画像</b><span>看看常听歌手和音乐风格</span></li><li><b>3 · 发现新的音乐方向</b><span>查看推荐，再探索 Agent 执行过程</span></li></ol>
      <button className="primary" onClick={() => { setMoreOpen(true); requestAnimationFrame(() => document.getElementById('more-import')?.scrollIntoView({ behavior: 'smooth' })) }}>第一步：导入我的歌单</button>
      <details className="welcome-more"><summary>更多探索功能</summary><div className="task-launcher-grid"><button onClick={() => document.getElementById('more-import')?.scrollIntoView({ behavior: 'smooth' })}><b>Analyze my playlist</b><span>导入 → 预览 → 分析</span></button><button onClick={onDiscover}><b>Discover new music</b><span>找到熟悉风格之外的新歌</span></button><button onClick={onCompare}><b>Compare with a friend</b><span>比较两份歌单的相似点和连接</span></button><button onClick={onVersions}><b>Find another version</b><span>发现 Live / Remix / Acoustic 等不同版本</span></button><button onClick={onTransfer}><b>Copy a playlist</b><span>Spotify → YouTube · 原歌单不变</span></button><button onClick={onAgent}><b>View Agent workflow</b><span>查看 AI 如何规划任务并调用工具</span></button></div></details>
    </section>
    <section className="hero">
      <div className="hero-copy"><span className="eyebrow">LOCAL-FIRST MUSIC ANALYSIS</span><h1>不登录账号，<br/><em>也能先分析歌单。</em></h1><p>直接粘贴“歌手 - 歌名”清单或上传文件。Rust 后端会在本机完成统计，联网时仅查询公开音乐目录补全 Genre、年代与能量；随后生成真正基于这份歌单的探索路线和推荐。</p><div className="hero-actions"><button className="primary big" onClick={() => document.getElementById('more-import')?.scrollIntoView({ behavior: 'smooth' })}>立即本地分析 <span>→</span></button><button className="secondary big" onClick={() => document.getElementById('connection-panel')?.scrollIntoView({ behavior: 'smooth' })}>连接音乐平台</button><button className="secondary big" onClick={onCompare}>与朋友比较</button></div><div className="trust-row"><span>✓ 无需账号密码</span><span>✓ Rust 本机分析</span><span>✓ 元数据失败也可降级运行</span></div></div>
      <div className="hero-map" aria-label="音乐探索路线示意"><div className="orbit orbit-one"/><div className="orbit orbit-two"/><GenreNode x="12%" y="62%" name="Korean R&B" tone="coral"/><GenreNode x="40%" y="35%" name="Alt. R&B" tone="gold"/><GenreNode x="70%" y="18%" name="Neo Soul" tone="mint"/><GenreNode x="73%" y="72%" name="Dream Pop" tone="blue"/><svg viewBox="0 0 500 400"><path d="M85 265 C155 245 150 170 224 163 S315 88 382 90"/><path className="dashed" d="M224 163 C285 188 318 286 390 280"/></svg><div className="map-caption"><strong>Genre 不是标签墙</strong><span>它是一张可以解释的路线图</span></div></div>
    </section>

    <section className="connection-section" id="connection-panel">
      <div className="import-guide"><strong>✓ 支持 Spotify / YouTube 官方连接</strong><p>Spotify / YouTube：Connect 官方账号 → 在官方页面授权 → 选择歌单 → 读取歌曲并核对预览。</p><p>其他平台：上传文件或复制歌曲列表 → Rust 解析 → 核对预览 → 确认分析。Apple 公开目录也可由部署者配置官方 API 后读取。</p><small>平台 API 曲目用于确认后的传输；体验分析和 Agent 请使用自备文件或文本。</small></div>
      <div className="section-heading wide"><span className="eyebrow">OFFICIAL CONNECTIONS FIRST</span><h2>连接你的音乐平台</h2><p>MelodyPath 不要求用户提供账号密码或 Cookie。Spotify / YouTube 使用官方 OAuth；其他平台按下方真实能力导入。公网 Demo：普通用户无需开发者凭据。自行部署：若需 Spotify / YouTube / Apple Music / Last.fm 的真实平台能力，由部署者配置对应开发者凭据。</p></div>
      <div className="platform-group"><div className="platform-group-title"><strong>可操作平台</strong><span>能力状态由 Rust 后端实时返回</span></div><div className="platform-grid">
        {capabilities.filter((item) => ['spotify', 'youtube_music', 'apple_music', 'netease', 'qq_music', 'kugou', 'qishui'].includes(item.platform)).map((capability) => {
          const isSpotify = capability.platform === 'spotify'
          const isYoutube = capability.platform === 'youtube_music'
          const connected = isSpotify ? spotify?.connected : isYoutube ? youtube?.connected : false
          const displayName = isSpotify ? spotify?.display_name : isYoutube ? youtube?.channel_title ?? youtube?.display_name : undefined
          return <article className={`platform-card status-${connected ? 'connected' : capability.capability_status}`} key={capability.platform}><div className="platform-card-head"><span className={`platform-mark mark-${capability.platform}`}>{capability.display_name.slice(0, 1)}</span><div><h3>{capability.display_name}</h3><span className="capability-label">{connected ? `已连接 · ${displayName}` : isSpotify || isYoutube ? '官方 API · 已真人验收' : capability.platform === 'apple_music' ? (capability.configured ? '官方 API · 已配置' : '官方 API · 需部署者配置') : '文件 / 文本导入'}</span></div></div><p>{capability.platform === 'apple_music' ? (capability.configured ? '公开目录歌单读取代码已实现，私人资料库未支持。' : '当前演示环境尚未配置 Apple Music Developer Token。部署者配置后可通过官方 API 读取公开目录歌单。目前仍可使用文件或文本导入。私人资料库未支持。') : humanPlatformText(capability.description)}</p><div className="capability-facts"><span>真人验收：{isSpotify || isYoutube ? '已真人验收（账号与曲目读取）' : '尚未真人验收'}</span><span>开发者配置：{capability.auth_supported || capability.platform === 'apple_music' ? '由部署者配置，普通用户无需申请' : '文件/文本不需要'}</span><span>公开 URL：{humanCapability(capability.public_playlist_links)}</span><span>歌曲导入：{['netease', 'qq_music', 'kugou'].includes(capability.platform) ? '公开页面有可验证曲目时显示 Imported X/Y，否则仅能力检测' : capability.public_link_import_supported ? (capability.configured ? '官方 API（受权限与地区限制）' : '需部署者配置') : '不可通过 URL 读取'}</span>{capability.auth_supported && <span>账号歌单：连接账号后可读取</span>}<span>账号授权：{capability.auth_supported ? '官方 OAuth' : '未接入'}</span><span>文件/文本：{capability.file_import_supported ? '支持文件 / 文本导入' : '暂不支持'}</span><span>读取：{humanCapability(capability.playlist_read)}</span><span>写入：{humanCapability(capability.playlist_write)}</span></div>{capability.policy_notice && <small>{capability.policy_notice}</small>}<div className="platform-actions">
            {isSpotify && spotify?.connected ? <><button className="primary" onClick={() => setSpotifyPickerOpen(true)}>选择我的歌单</button><a className="text-button" href="/api/spotify/authorize">连接 Spotify</a><button className="text-button" onClick={() => void disconnectSpotify()}>解除连接</button></> : isYoutube && youtube?.connected ? <><button className="primary" onClick={() => setYoutubePickerOpen(true)}>选择我的播放列表</button><button className="text-button" onClick={() => void disconnectYoutube()}>解除连接</button></> : capability.auth_supported && capability.configured ? <a className="primary" href={isYoutube ? '/api/youtube/authorize' : '/api/spotify/authorize'}>连接 {capability.display_name}</a> : capability.auth_supported ? <button className="secondary" onClick={() => setConfigPlatform(isSpotify ? 'spotify' : isYoutube ? 'youtube' : 'apple')}>查看部署者配置说明</button> : capability.file_import_supported ? <button className="secondary" onClick={() => document.getElementById('more-import')?.scrollIntoView({ behavior: 'smooth' })}>导入文件或文本</button> : <button className="secondary" disabled>{capability.status_label}</button>}
            {capability.public_link_import_supported && <button className="text-button" onClick={() => document.getElementById('public-link-panel')?.scrollIntoView({ behavior: 'smooth' })}>公开歌单 URL</button>}{capability.official_docs_url && <a className="docs-link" href={capability.official_docs_url} target="_blank" rel="noreferrer">官方说明 ↗</a>}
          </div></article>
        })}
      </div></div>
      <details className="more-platforms"><summary>更多平台（尚未实现或等待资格）</summary><div className="platform-grid">{capabilities.filter((item) => !['spotify', 'youtube_music', 'apple_music', 'netease', 'qq_music', 'kugou', 'qishui'].includes(item.platform)).map((item) => <article className="platform-card" key={item.platform}><h3>{item.display_name}</h3><p>{item.description}</p><span className="capability-label">{humanCapability(item.status)}</span></article>)}</div></details>
      {spotify && <div className={`spotify-policy ${spotify.connected ? 'connected' : ''}`}><strong>{spotify.connected ? `Spotify 已连接：${spotify.display_name}` : 'Spotify 授权状态'}</strong><span>{spotify.message}</span><small>{spotify.policy_notice}</small></div>}
      {youtube && <div className={`spotify-policy ${youtube.connected ? 'connected' : ''}`} role="status"><strong>{youtube.connected ? 'YouTube Connected · 已连接' : 'YouTube 未连接'}</strong><span>{youtube.message}</span>{!youtube.connected && youtube.configured && <a href="/api/youtube/authorize">重新进行 YouTube 只读授权</a>}</div>}
    </section>

    <section className="link-section" id="public-link-panel"><div className="section-heading"><span className="eyebrow">PUBLIC PLAYLIST LINK</span><span className="experimental-status">按平台显示实际能力</span><h2>或者粘贴公开歌单链接</h2><p>Spotify / YouTube 授权后通过官方 API 读取；Apple Music 公开目录需部署者配置，尚未真人验收。网易云按公开页面可获取范围尝试导入，最多 20 首 / 20 秒；其他中国平台保留链接检测与文件/文本导入。不需要账号密码或 Cookie。</p></div><div className="input-card link-card"><label className="field"><span>Spotify、YouTube、Apple Music 或中国平台歌单链接</span><div className="link-input-row"><input id="playlist-link" type="url" placeholder="https://music.163.com/playlist?id=…" value={link} onChange={(event) => { setLink(event.target.value); setLinkResult(null); linkRequest.current++; setLinkBusy(false); if (importPreview?.data_state === 'REAL_PUBLIC_LINK') setImportPreview(null) }} onKeyDown={(event) => { if (event.key === 'Enter') void inspectLink() }}/><button className="primary" disabled={busy || linkBusy || !link.trim()} onClick={() => void inspectLink()}>{linkBusy ? '正在检查…' : '检查链接读取能力'}</button></div></label>{linkResult && <div className={`link-result ${linkResult.capability === 'TRACK_IMPORT_AVAILABLE' && linkResult.preview_tracks.length > 0 ? 'ready' : ''}`}><LinkImportStatus result={linkResult}/><p>{humanPlatformText(linkResult.message)}</p><small>当前能力：{humanCapability(linkResult.capability)} · 歌单 ID：{linkResult.playlist_id_valid ? '格式有效' : '未验证'}</small>{linkResult.capability === 'AUTH_REQUIRED' && (() => {
      const provider = linkResult.platform === 'spotify' ? 'spotify' : 'youtube'
      const status = provider === 'spotify' ? spotify : youtube
      return status?.configured ? <a className="primary" href={`/api/${provider}/authorize`}>Connect {provider === 'spotify' ? 'Spotify' : 'YouTube'}</a> : <button className="secondary" onClick={() => setConfigPlatform(provider)}>查看部署者配置说明</button>
    })()}{linkResult.import_rows && linkResult.import_rows.length > 0 && <details><summary>逐项导入报告 · {linkResult.import_rows.length} 个源条目 / {linkResult.import_rows.filter(row => row.import_status !== 'IMPORTED').length} 个跳过</summary><div className="spotify-track-preview">{linkResult.import_rows.map((row, index) => <div key={index}><strong>{row.track_title ?? '元数据不可用'}</strong><small>{(row.artist ?? []).join(', ') || '艺人缺失'} · {row.duration_ms == null ? '时长未知' : String(row.duration_ms) + ' ms'} · {humanCapability(row.availability)} · {humanCapability(row.import_status)}</small></div>)}</div></details>}{!linkResult.can_analyze && linkResult.capability === 'TRACK_IMPORT_AVAILABLE' && linkResult.preview_tracks.length > 0 && <><div className="link-preview"><span>Import Preview · 共 {linkResult.track_count} 首，展示前 {Math.min(10, linkResult.preview_tracks.length)} 首</span>{linkResult.preview_tracks.slice(0, 10).map((track) => <div key={track.id}><strong>{track.title}</strong><small>{track.artists.join(', ')}</small></div>)}</div><button className="primary" disabled={linkResult.capability !== 'TRACK_IMPORT_AVAILABLE'} onClick={confirmLink}>确认并进入 Copy Playlist 预览</button></>}<small>{linkResult.platform === 'apple_music' && linkResult.capability === 'CONFIG_REQUIRED' ? '普通用户无需申请开发者凭据，可直接使用文件或文本导入。' : humanPlatformText(linkResult.next_step)}</small>{!linkResult.can_analyze && <button className="secondary" onClick={() => { setMoreOpen(true); document.getElementById('more-import')?.scrollIntoView({ behavior: 'smooth' }) }}>导入文件或文本</button>}</div>}{error && <p className="error-box">{error}</p>}<p className="fine-print">网易云仅使用公开 HTML 和官方歌曲页元数据；曲目完整性检查通过后显示实际导入预览。Spotify / YouTube / Apple 官方 API 曲目继续用于确认后的传输。</p></div></section>

    <section className="more-import-section" id="more-import"><details open={moreOpen} onToggle={(event) => setMoreOpen(event.currentTarget.open)}><summary><span><strong>上传你的歌单，开始探索你的音乐偏好</strong><small>先由 Rust 可靠解析并预览，确认后才会分析</small></span><b>{moreOpen ? '−' : '+'}</b></summary><div className="fallback-grid"><div className="input-card"><span className="eyebrow">REAL PLAYLIST IMPORT</span><h3>Import · 文件或批量文本</h3><p>✓ CSV / TXT / JSON 文件（也支持 TSV、M3U/M3U8）<br/>✓ 直接粘贴：歌手 - 歌名</p><p className="import-example">例如：<br/>周杰伦 - 晴天<br/>DEAN - instagram</p><p className="fallback-copy">支持自行整理的 CSV、TSV、JSON、TXT、M3U/M3U8；不代表各平台都有官方导出格式。Apple Music Mac 可用“文件 → 资料库 → 导出播放列表 → 文本文件”，或复制歌曲列。中国平台可手动整理“歌手 - 歌名”；不使用需要密码或 Cookie 的导出工具。暂不接收 XML / HTML。</p><label className="field"><span>歌单名称</span><input value={name} onChange={(event) => setName(event.target.value)} /></label><label className="field"><span>批量文本</span><textarea rows={7} value={text} onChange={(event) => setText(event.target.value)} /></label><div className="input-actions"><label className="secondary upload-button">选择真实文件<input type="file" accept=".txt,.csv,.tsv,.json,.m3u,.m3u8" onChange={(event) => { const input = event.currentTarget; const file = input.files?.[0]; input.value = ''; void loadFile(file) }}/></label><button className="primary" onClick={() => void prepareImport({ name, format: 'txt', content: text, data_state: 'REAL_TEXT' })} disabled={busy}>{busy ? '正在解析…' : '解析并预览文本'}</button></div><p className="fine-print">没有导出文件？可以直接粘贴：歌手 - 歌名。支持中日韩英文混合、常见横线与 TAB；系统先自动判断歌手/歌名，置信度不足时再请你确认。</p></div><div className="demo-fallback"><span className="demo-badge">DEMO MODE</span><h3>明确体验示例</h3><p>{demo.disclosure}</p><button className="secondary" onClick={onDemo}>体验 Demo</button><div><strong>只有点击本按钮才显示 Demo</strong><span>真实导入失败不会进入这里。</span></div></div></div>{importProgress && <TaskProgress state={importProgress}/>} {importError && <ImportErrorFeedback error={importError}/>} {importPreview && <div ref={(node) => { previewRef.current = node }}><ImportPreviewPanel key={importPreview.id} preview={importPreview} busy={busy} onOrder={(order) => pendingImport && void prepareImport({ ...pendingImport, text_order: order })} onConfirm={() => void confirmImport()} /></div>}</details></section>
    {spotifyPickerOpen && <SpotifyPlaylistPicker onClose={() => setSpotifyPickerOpen(false)} onAnalysis={(personal) => onManual(personal, 'REAL_ACCOUNT')} />}
    {youtubePickerOpen && <YouTubePlaylistPicker onClose={() => setYoutubePickerOpen(false)} onExport={onExport} />}
    {configPlatform && <ConfigurationWizard platform={configPlatform} onClose={() => setConfigPlatform(null)} onChecked={onRefreshConnections} />}
  </>
}

function estimateImportRows(content: string, format: string) {
  const rows = content.split(/\r?\n/).filter((line) => line.trim()).length
  return Math.max(1, ['csv', 'tsv'].includes(format.toLowerCase()) ? rows - 1 : rows)
}

function importErrorCategory(error: string) {
  if (/编码|TextDecoder|UTF-?\d+/i.test(error)) return '编码问题'
  if (/不支持的导入格式|文件格式|扩展名/i.test(error)) return '文件格式错误'
  if (/未找到歌曲列|未找到歌手列|缺少.*(?:歌曲|歌手)|歌曲字段/i.test(error)) return '缺少歌曲字段'
  return '无法识别内容'
}

function ImportErrorFeedback({ error }: { error: string }) {
  return <div className="import-error-feedback" role="alert"><strong>⚠ {importErrorCategory(error)}</strong><p>{error}</p><span>支持格式示例：</span><pre>CSV:{'\n'}artist,title{'\n\n'}TXT:{'\n'}artist - title</pre></div>
}

function metadataPreviewState(status: string) {
  if (status === 'complete') return { label: 'Matched', detail: 'Metadata 已就绪', tone: 'matched' }
  if (status === 'partial') return { label: 'Waiting', detail: 'Metadata 待匹配', tone: 'waiting' }
  return { label: 'Waiting', detail: 'Metadata 待匹配', tone: 'waiting' }
}

function ImportPreviewPanel({ preview, busy, onOrder, onConfirm }: { preview: ImportPreview; busy: boolean; onOrder: (order: 'artist_title' | 'title_artist') => void; onConfirm: () => void }) {
  const needsConfirmation = preview.parser_status === 'need_confirmation'
  return <section className="import-preview panel"><div className={needsConfirmation ? 'import-success need-confirmation' : 'import-success'} role="status" aria-live="polite"><strong>{needsConfirmation ? 'Need confirmation' : '✅ 歌单解析成功'}</strong><span>已读取：<b>{preview.parsed_count}</b> 首歌曲</span><span>{preview.parser_status === 'llm_fallback' ? 'LLM 仅完成结构拆分，真实性待 MetadataResolver 验证' : needsConfirmation ? '请补充明确分隔符或编辑文本后重试' : '请查看下方 Preview ↓'}</span></div><div className="panel-title"><div><span className="eyebrow">IMPORT PREVIEW · {preview.data_state}</span><h3>{preview.file_name ?? preview.source_label}</h3></div><strong>{preview.parser_status === 'llm_fallback' ? 'LLM STRUCTURED · NOT VERIFIED' : 'is_demo=false'}</strong></div><div className="import-stats"><span><b>{preview.total_rows}</b>总行数</span><span><b>{preview.parsed_count}</b>成功解析</span><span><b>{preview.warning_count}</b>警告</span><span><b>{preview.invalid_count}</b>无法解析</span></div><p>检测字段：{preview.detected_fields.join(' · ') || '文本列'}</p>{preview.questions.map((question) => <p className="warning-box" key={question}>{question}</p>)}{preview.requires_column_confirmation && preview.preview_tracks.length > 0 && <div className="order-confirm"><strong>请选择文本列含义</strong><button className="secondary" onClick={() => onOrder('artist_title')}>左侧歌手，右侧歌名</button><button className="secondary" onClick={() => onOrder('title_artist')}>左侧歌名，右侧歌手</button></div>}<div className="import-track-list"><div className="import-track-head"><span>#</span><span>歌名</span><span>歌手</span><span>专辑</span><span>解析</span><span>Metadata</span></div>{preview.preview_tracks.map((track, index) => { const metadata = metadataPreviewState(track.metadata_status); return <div className="import-track-row" key={`${track.original_row}-${index}`}><span>{index + 1}</span><strong>{track.title}</strong><span>{track.artists.join(' / ')}</span><span>{track.album ?? '—'}</span><em className="parse-success">✓ Success<small>{preview.parser_status === 'llm_fallback' ? '结构化' : '已解析'}</small></em><em className={`metadata-state ${metadata.tone}`}>{metadata.label}<small>{metadata.detail}</small></em></div> })}</div><footer className="preview-actions"><span>预览前 {Math.min(20, preview.parsed_count)} 首；确认后仅通过 MetadataResolver 的曲目进入 Taste Profile。</span><button className="primary" disabled={busy || preview.requires_column_confirmation || preview.parsed_count === 0} onClick={onConfirm}>{busy ? '正在分析…' : '确认并分析真实数据'}</button></footer></section>
}

function humanPlatformText(value: string = '') {
  if (value.includes('WAITING_FOR_APPLE_DEVELOPER_CREDENTIALS')) return '当前演示环境尚未配置 Apple Music Developer Token。部署者配置后可通过官方 API 读取公开目录歌单。目前仍可使用文件或文本导入。'
  return value.replace(/CONFIG_REQUIRED/g, '需部署者配置').replace(/AUTH_REQUIRED/g, '连接账号后可用').replace(/NOT REAL VERIFIED/g, '尚未真人验收').replace(/REAL VERIFIED/g, '已真人验收')
}
function humanCapability(value: string) {
  const labels: Record<string, string> = {
    ACCESSIBILITY_CHECK_ONLY: '可识别链接并检查公开可访问性', URL_RECOGNITION_ONLY: '可识别公开链接',
    FILE_IMPORT_AVAILABLE: '支持文件 / 文本导入', TRACK_IMPORT_AVAILABLE: '支持读取真实歌单曲目',
    CONFIG_REQUIRED: '需部署者配置', AUTH_REQUIRED: '连接账号后可用', UNSUPPORTED: '暂不支持',
    PUBLIC_METADATA_AVAILABLE: '已读取歌单信息，暂无可导入曲目', not_implemented: '暂不支持',
    UNKNOWN: '可播放状态未知', UNAVAILABLE: '暂不可用', PLAYABLE_WITH_SUBSCRIPTION: '订阅后可播放',
    IMPORTED: '已导入', SKIPPED_DUPLICATE: '重复曲目，已跳过', SKIPPED_MISSING_ARTIST: '缺少艺人，已跳过',
    SKIPPED_REQUEST_LIMIT: '超过详情请求上限，未导入', SKIPPED_TIME_LIMIT: '超过本批时间上限，未导入', SKIPPED_DETAIL_UNAVAILABLE: '详情不可用或缺少艺人，未导入',
    SKIPPED_UNAVAILABLE_METADATA: '曲目信息缺失，已跳过', SKIPPED_UNSUPPORTED_TYPE: '非歌曲条目，已跳过',
    public_catalog_only: '仅公开目录，私人资料库未接入', official_api: '官方 API', official_api_transfer_only: '官方 API（仅传输）', needs_configuration: '需要配置',
    requires_official_credentials: '需正式资格', requires_platform_approval: '需平台审核', planned: '计划中',
    not_verified: '未验证', not_verified_for_public_web: '未核实本项目适用的通用接口', official_sdk_only: '仅官方 SDK',
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
    } catch (reason) { setError(reason instanceof TypeError ? '配置检查无法连接后端，请确认服务已启动后重试。' : reason instanceof Error ? reason.message : '配置检查失败') }
    finally { setBusy(false) }
  }
  useEffect(() => { void load() }, [platform]) // eslint-disable-line react-hooks/exhaustive-deps
  return <div className="modal-backdrop"><section className="export-modal config-wizard" role="dialog" aria-modal="true"><header className="modal-header"><div><span className="eyebrow">LOCAL DEVELOPER SETUP</span><h2>{status?.display_name ?? platform} 配置向导</h2></div><button className="icon-button" onClick={onClose}>×</button></header>{busy && <p>正在检查后端环境变量…</p>}{error && <p className="error-box">{error}</p>}{status && <><div className={`configuration-verdict ${status.configured ? 'ready' : ''}`}><strong>{status.configured ? (platform === 'apple' ? '目录配置格式就绪 · 尚未真人验收' : '配置已就绪，仍需官方授权') : '需部署者配置'}</strong><span>{humanPlatformText(status.message)}</span></div>{status.redirect_uri && <label className="field"><span>Developer Console 中必须填写的精确 Redirect URI</span><code className="redirect-code">{status.redirect_uri}</code></label>}<div className="config-columns"><div><h3>缺少的环境变量</h3>{status.missing_environment_variables.length ? <ul>{status.missing_environment_variables.map((name) => <li><code>{name}</code></li>)}</ul> : <p>无</p>}</div><div><h3>已检测到</h3>{status.present_environment_variables.length ? <ul>{status.present_environment_variables.map((name) => <li><code>{name}</code></li>)}</ul> : <p>无</p>}</div></div><ol className="setup-steps">{status.setup_steps.map((step) => <li key={step}>{step}</li>)}</ol><p className="fine-print">此页面只显示变量名和状态，永远不会显示 Client Secret、私钥或 token。修改后端环境变量后需要重启 Rust 后端。</p><div className="modal-actions">{status.dashboard_url && <a className="secondary" href={status.dashboard_url} target="_blank" rel="noreferrer">打开官方控制台 ↗</a>}<button className="primary" disabled={busy} onClick={() => void load(true)}>检查配置</button></div>{platform === 'apple' && <p className="policy-box">Apple 公开目录读取使用后端 Developer Token；私人资料库授权和 Apple 写入未接入。公开目录读取成功也不代表账号登录成功。</p>}</>}</section></div>
}

function YouTubePlaylistPicker({ onClose, onExport }: { onClose: () => void; onExport: (platform: string, tracks: Track[], label: string) => void }) {
  const [playlists, setPlaylists] = useState<YouTubePlaylistSummary[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [result, setResult] = useState<YouTubeImportResult | null>(null)
  const [busy, setBusy] = useState(true)
  const [error, setError] = useState('')
  const [progress, setProgress] = useState<TaskProgressState | null>({ stage: 'uploading', detail: '正在读取可选 YouTube 播放列表' })
  useEffect(() => { api.youtubePlaylists().then((items) => { setPlaylists(items); setProgress(null) }).catch((reason: unknown) => { const message = reason instanceof Error ? reason.message : '读取 YouTube 播放列表失败'; setError(message); setProgress({ stage: 'failed', error: message }) }).finally(() => setBusy(false)) }, [])
  const toggle = (id: string) => setSelected((current) => { const next = new Set(current); next.has(id) ? next.delete(id) : next.add(id); return next })
  const importSelected = async () => { setBusy(true); setError(''); setProgress({ stage: 'uploading', detail: `正在读取 ${selected.size} 个 YouTube 播放列表` }); try { const imported = await api.importYouTubePlaylists([...selected]); setResult(imported); setProgress({ stage: 'completed', detail: `Imported ${imported.track_count}/${imported.track_count} tracks` }) } catch (reason) { const message = reason instanceof Error ? reason.message : '读取失败'; setError(message); setProgress({ stage: 'failed', error: message }) } finally { setBusy(false) } }
  return <div className="modal-backdrop"><section className="export-modal spotify-picker" role="dialog" aria-modal="true"><header className="modal-header"><div><span className="eyebrow">YOUTUBE DATA API · OFFICIAL</span><h2>{result ? '确认导入的曲目' : '选择你拥有的播放列表'}</h2></div><button className="icon-button" onClick={onClose}>×</button></header><div className="spotify-restriction"><strong>合规边界</strong><span>官方 API 数据用于你主动请求的预览、传输和写回，不发送给 LLM，也不计算独立衍生画像。</span></div>{progress && <TaskProgress state={progress}/>} {error && <p className="error-box">{error}</p>}{!busy && !result && <><div className="spotify-playlist-list">{playlists.map((playlist) => <label className={selected.has(playlist.id) ? 'spotify-playlist selected' : 'spotify-playlist'} key={playlist.id}><input type="checkbox" checked={selected.has(playlist.id)} onChange={() => toggle(playlist.id)}/>{playlist.image_url ? <img src={playlist.image_url} alt=""/> : <span className="playlist-placeholder">▶</span>}<span><strong>{playlist.name}</strong><small>{playlist.item_count} 项</small></span><a href={playlist.youtube_url} target="_blank" rel="noreferrer">YouTube ↗</a></label>)}</div><div className="modal-actions"><button className="secondary" onClick={onClose}>取消</button><button className="primary" disabled={!selected.size} onClick={() => void importSelected()}>读取所选播放列表</button></div></>}{result && <><div className="import-summary"><strong>{result.track_count}</strong><span>首条目已清理标题噪声并转换为统一 Track</span></div><p className="policy-box">{result.policy_notice}</p><div className="spotify-track-preview">{result.tracks.slice(0, 10).map((track, index) => <div key={track.id}><span>{index + 1}</span><div><strong>{track.title}</strong><small>{track.artists.join(', ')}</small></div>{track.platform_url && <a href={track.platform_url} target="_blank" rel="noreferrer">YouTube ↗</a>}</div>)}</div><div className="modal-actions"><button className="secondary" onClick={() => { setResult(null); setProgress(null) }}>返回</button><button className="primary" onClick={() => { onExport('youtube', result.tracks, result.playlists.map((p) => p.name).join(' + ')); onClose() }}>预览并创建新的 YouTube 播放列表</button></div></>}</section></div>
}

function SpotifyPlaylistPicker({ onClose, onAnalysis }: { onClose: () => void; onAnalysis: (analysis: PersonalAnalysis) => void }) {
  const [playlists, setPlaylists] = useState<SpotifyPlaylistSummary[]>([])
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [result, setResult] = useState<SpotifyAnalysisPreview | null>(null)
  const [busy, setBusy] = useState(true)
  const [error, setError] = useState('')
  const [progress, setProgress] = useState<TaskProgressState | null>(null)
  const confirming = useRef(false)

  useEffect(() => {
    api.spotifyPlaylists().then(setPlaylists).catch((reason: unknown) => setError(reason instanceof Error ? reason.message : '读取 Spotify 歌单失败')).finally(() => setBusy(false))
  }, [])

  const toggle = (id: string) => setSelected((current) => {
    const next = new Set(current)
    if (next.has(id)) next.delete(id); else next.add(id)
    return next
  })
  const importSelected = async () => {
    setBusy(true); setError(''); setResult(null); setProgress({ stage: 'uploading', detail: `正在读取 ${selected.size} 个 Spotify 歌单` })
    try { const imported = await api.previewSpotifyImport([...selected]); setResult(imported); setProgress({ stage: 'completed', detail: `Imported ${imported.track_count}/${imported.track_count} tracks` }) }
    catch (reason) { const message = reason instanceof Error ? reason.message : 'Spotify 歌单读取失败'; setError(message); setProgress({ stage: 'failed', error: message }) }
    finally { setBusy(false) }
  }

  const confirm = async () => {
    if (confirming.current || busy || !result) return
    if (!result.import_id) {
      setError('缺少 import_id，请返回重选以生成新的分析预览；确认后端已更新。')
      return
    }
    confirming.current = true
    setBusy(true); setError(''); setProgress({ stage: 'resolving_metadata', detail: `${result.track_count} 首歌曲` })
    try {
      const analysis = await api.analyzeImport(result.import_id, stage => setProgress({ stage, detail: `${result.track_count} 首歌曲` }))
      if (!analysis.analysis_id || !analysis.playlist || analysis.playlist.is_demo) throw new Error('分析返回了无效结果，请重试。')
      setProgress({ stage: 'completed', detail: '分析与推荐已完成' })
      onAnalysis(analysis)
    } catch (reason) {
      const message = reason instanceof Error ? `分析失败：${reason.message}` : '分析失败，请检查后端后重试。'; setError(message); setProgress({ stage: 'failed', error: message })
    } finally { confirming.current = false; setBusy(false) }
  }

  return <div className="modal-backdrop"><section className="export-modal spotify-picker spotify-picker-fixed" role="dialog" aria-modal="true" aria-label="选择 Spotify 歌单">
    <header className="modal-header"><div><span className="eyebrow">SPOTIFY · OFFICIAL API</span><h2>{result ? 'Import Preview · 确认导入的曲目' : '选择可访问的歌单'}</h2></div><button className="icon-button" aria-label="关闭歌单选择" disabled={busy} onClick={onClose}>×</button></header>
    <div className="picker-body">
      <p className="spotify-restriction">确认后分析所选歌单并生成推荐；Spotify 来源标识会保留，数据不会发送给 LLM。</p>
      {progress && <TaskProgress state={progress} />}
      {error && <p className="error-box" role="alert">{error}</p>}
      {!result && <div className="spotify-playlist-list" aria-label="可访问的 Spotify 歌单">{playlists.map((playlist) => <label className={selected.has(playlist.id) ? 'spotify-playlist selected' : 'spotify-playlist'} key={playlist.id}>
        <input type="checkbox" disabled={busy} checked={selected.has(playlist.id)} onChange={() => toggle(playlist.id)}/>{playlist.image_url ? <img src={playlist.image_url} alt=""/> : <span className="playlist-placeholder">♫</span>}
        <span><strong>{playlist.name}</strong><small>{playlist.owner_name} · {playlist.track_count} 首{playlist.collaborative ? ' · 协作歌单' : ''}</small></span>{playlist.spotify_url && <a href={playlist.spotify_url} target="_blank" rel="noreferrer" onClick={(event) => event.stopPropagation()}>Spotify ↗</a>}
      </label>)}{!busy && playlists.length === 0 && <p className="empty-row">当前没有 API 可访问的歌单，请检查账号权限。</p>}</div>}
      {result && <div className="picker-preview-body"><div className="import-summary"><strong>{result.track_count}</strong><span>首真实 API 曲目</span></div><div className="spotify-track-preview">{result.tracks.slice(0, 10).map((track) => <div key={track.id}><strong>{track.title}</strong><small>{track.artists.join(', ')}</small>{track.platform_url && <a href={track.platform_url} target="_blank" rel="noreferrer">Spotify ↗</a>}</div>)}</div><p>{result.attribution}</p></div>}
    </div>
    <footer className="modal-actions picker-footer"><span aria-live="polite">已选择 {selected.size} 个歌单</span><button className="secondary" disabled={busy} onClick={onClose}>取消</button>{result ? <><button className="secondary" disabled={busy} onClick={() => { setResult(null); setError(''); setProgress(null) }}>返回重选</button><button className="primary" disabled={busy || result.tracks.length === 0} onClick={() => void confirm()}>{busy ? '分析中…' : 'Confirm Import · 确认并分析'}</button></> : <button className="primary" disabled={selected.size === 0 || busy} onClick={() => void importSelected()}>确认选择 / Continue</button>}</footer>
  </section></div>

}

function GenreNode({ x, y, name, tone }: { x: string; y: string; name: string; tone: string }) { return <div className={`genre-node ${tone}`} style={{ left: x, top: y }}><span/><strong>{name}</strong></div> }

function TastePage({ personal, canExplore, spotifyConnected, onExplore, onExport }: { personal: PersonalAnalysis; canExplore: boolean; spotifyConnected: boolean; onExplore: () => void; onExport: (platform: string, tracks: Track[], label: string) => void }) {
  const { report, playlist, import_summary: summary, unmatched_tracks: unmatched } = personal
  const profile = personal.taste_profile
  const maxGenre = Math.max(...report.genre_distribution.map((item) => item[1]), 1)
  return <div className="page-width report-page">
    <div className="section-heading"><span className="eyebrow">Music Profile</span><h2>了解你的音乐风格、常听歌手和偏好</h2></div>
    <PageIntro eyebrow={summary?.data_state === 'REAL_FILE' ? 'REAL FILE ANALYSIS' : summary?.data_state === 'REAL_PUBLIC_LINK' ? 'REAL PUBLIC LINK ANALYSIS' : summary?.data_state === 'REAL_TEXT' ? 'REAL TEXT ANALYSIS' : 'PERSONAL TASTE MAP'} title={report.playlist_name} copy={report.summary} badge={report.is_demo ? 'DEMO DATA' : `${report.source_label} · is_demo=false`} />
    {summary && <section className="real-analysis-summary panel"><div className="panel-title"><div><span className="eyebrow">VERIFIED REAL INPUT</span><h3>真实数据分析覆盖</h3></div><strong>{Math.round(summary.genre_coverage * 100)}% Genre 覆盖率</strong></div><div className="import-stats analysis"><span><b>{summary.input_count}</b>输入总数</span><span><b>{summary.parsed_count}</b>成功解析</span><span><b>{summary.analyzed_count}</b>实际参与分析</span><span><b>{summary.complete_metadata_count}</b>完整元数据</span><span><b>{summary.partial_metadata_count}</b>部分元数据</span><span><b>{summary.unmatched_count}</b>未匹配</span></div><p>数据来源：{summary.source_label} · Genre：{summary.genre_matched_count}/{summary.analyzed_count} · Energy：{summary.energy_matched_count}/{summary.analyzed_count}</p></section>}
    {profile && summary?.data_state !== 'REAL_ACCOUNT' && <section className="panel taste-profile"><div className="panel-title"><div><span className="eyebrow">TASTE PROFILE · RESOLVED METADATA ONLY</span><h3>你的音乐画像</h3></div><strong>{profile.resolved_track_count} 首已解析歌曲</strong></div>{profile.resolved_track_count > 0 ? <div className="taste-profile-grid"><RankingList title="Top Artists" items={profile.top_artists}/><RankingList title="Top Albums" items={profile.top_albums}/><RankingList title="Language Distribution" items={profile.language_distribution} total={profile.resolved_track_count}/><RankingList title="Genres" items={profile.genres}/></div> : <div className="zone-empty">暂无通过 MetadataResolver 验证的歌曲，未生成画像排名。</div>}</section>}
    <div className="metric-grid">{report.metrics.map((metric) => <MetricCard key={metric.label} {...metric} />)}</div>
    {!report.is_demo && <div className="basic-stat-grid"><span><b>{report.album_distribution.length}</b>专辑数</span><span><b>{report.duplicate_track_count}</b>重复歌曲</span><span><b>{report.collaboration_track_count}</b>合作歌曲</span><span><b>{report.artist_distribution.length}</b>歌手数</span></div>}
    <div className="report-grid"><section className="panel chart-panel"><div className="panel-title"><div><span className="eyebrow">GENRE SIGNAL</span><h3>你的声音地形</h3></div><span>共 {report.track_count} 首</span></div><div className="bar-chart">{report.genre_distribution.slice(0, 7).map(([genre, count]) => <div className="bar-row" key={genre}><span>{genre}</span><div><i style={{ width: `${count / maxGenre * 100}%` }}/></div><b>{count}</b></div>)}</div></section><section className="panel zones-panel"><div className="panel-title"><div><span className="eyebrow">TASTE ZONES</span><h3>核心、相邻与空白</h3></div></div><TasteZone label="核心区" values={report.core_preferences} tone="core"/><TasteZone label="相邻区" values={report.adjacent_preferences} tone="adjacent"/><TasteZone label="待探索" values={report.unexplored_preferences} tone="blank"/></section></div>
    <section className="panel source-tracks"><div className="panel-title"><div><span className="eyebrow">SOURCE TRACKS</span><h3>全部真实分析歌曲</h3></div><span>{playlist.tracks.length} 首 · 元数据置信度 {Math.round(report.confidence * 100)}%</span></div><div className="compact-track-grid">{playlist.tracks.map((track) => <TrackLine track={track} resolution={personal.metadata_resolutions?.find((item) => item.track_id === track.id)} key={track.id}/>)}</div></section>
    {spotifyConnected && playlist.tracks.length > 0 && <div className="export-toolbar"><div><span className="eyebrow">SPOTIFY EXPORT</span><strong>标准 Track 将先匹配 Spotify，确认后再新建私有歌单</strong></div><button className="spotify-button" onClick={() => onExport('spotify', playlist.tracks, playlist.name)}>Create Spotify Playlist</button></div>}
    {summary && unmatched.length > 0 && <section className="panel unmatched-section"><div className="panel-title"><div><span className="eyebrow">UNMATCHED TRACKS</span><h3>元数据暂未匹配</h3></div><strong>{unmatched.length} 首仍参与基础分析</strong></div>{unmatched.map((track, index) => <div className="unmatched-row" key={`${track.original_row}-${index}`}><span>{index + 1}</span><strong>{track.title}</strong><span>{track.artists.join(' / ')}</span><em>metadata_status=missing</em><small>{track.warnings.join('；')}</small></div>)}</section>}
    <div className="limitation-note"><strong>数据说明</strong>{report.limitations.map((item) => <span key={item}>{item}</span>)}</div>
    {canExplore && <div className="page-cta"><div><span className="eyebrow">NEXT: DISCOVERY ROUTE</span><h2>从熟悉出发，但不在熟悉处打转。</h2></div><button className="primary big" onClick={onExplore}>查看基于当前歌单的推荐 →</button></div>}
  </div>
}

function RankingList({ title, items, total }: { title: string; items: RankingItem[]; total?: number }) {
  const groups = items.reduce<Record<string, RankingItem[]>>((result, item) => { (result[item.rank] ??= []).push(item); return result }, {})
  return <div className="ranking-list"><h4>{title}</h4>{Object.values(groups).map((group) => <div className="ranking-group" key={`${title}-${group[0].rank}`}><span>Rank {group[0].rank}{group[0].tied ? ' · Tie' : ''}</span><div>{group.map((item) => <strong key={item.name}>{item.name}</strong>)}</div><small>{total ? `${Math.round(group[0].count / total * 100)}% · ${group[0].count} tracks` : `${group[0].count} tracks`}</small></div>)}{items.length === 0 && <div className="zone-empty">No resolved metadata</div>}</div>
}

function MetricCard({ label, display, explanation, value }: { label: string; display: string; explanation: string; value: number }) { return <article className="metric-card"><div className="ring" style={{ '--value': `${value * 360}deg` } as React.CSSProperties}><span>{display}</span></div><div><strong>{label}</strong><p>{explanation}</p></div></article> }
function TasteZone({ label, values, tone }: { label: string; values: string[]; tone: string }) { return <div className={`taste-zone ${tone}`}><strong>{label}</strong><div>{values.map((value) => <span key={value}>{value}</span>)}</div></div> }
function TrackLine({ track, resolution }: { track: Track; resolution?: PersonalAnalysis['metadata_resolutions'][number] }) { const matched = resolution && resolution.status !== 'UNMATCHED'; return <div className="track-line"><span className="album-placeholder">♪</span><span><strong>{track.title}</strong><small>{track.artists.join(', ')} · {track.album ?? '专辑未知'} · {track.release_year ?? '年份未知'}</small>{resolution && <small>{matched ? 'Matched' : 'Not Found · 未找到外部 metadata'}{resolution.source ? ` · Source: ${resolution.source}` : ''}{matched ? ` · ${Math.round(resolution.match_confidence * 100)}%` : ''}</small>}</span><em>{track.genres[0] ?? '元数据暂未匹配'}</em></div> }

function RecommendationPage({ personal, spotifyConnected, onExport }: { personal: PersonalAnalysis; spotifyConnected: boolean; onExport: (platform: string, tracks: Track[], label: string) => void }) {
  const zones = ['舒适区', '拓展区', '惊喜区']
  const routeLabel = personal.route.map((step) => step.genre).join(' → ') || '音乐探索路线'
  const summary = personal.recommendation_summary
  const stats = summary.query_stats
  const pools: Record<string, Recommendation[]> = {
    '舒适区': summary.comfort_pool?.length ? summary.comfort_pool : personal.recommendations.filter((item) => item.zone === '舒适区'),
    '拓展区': summary.expansion_pool?.length ? summary.expansion_pool : personal.recommendations.filter((item) => item.zone === '拓展区'),
    '惊喜区': summary.surprise_pool?.length ? summary.surprise_pool : personal.recommendations.filter((item) => item.zone === '惊喜区'),
  }
  const [offsets, setOffsets] = useState<Record<string, number>>({ '舒适区': 0, '拓展区': 0, '惊喜区': 0 })
  const [expanded, setExpanded] = useState<Record<string, boolean>>({})
  const [exhausted, setExhausted] = useState<Record<string, boolean>>({})
  const lastFmConfigured = summary.status !== 'not_configured'
  useEffect(() => { setOffsets({ '舒适区': 0, '拓展区': 0, '惊喜区': 0 }); setExpanded({}); setExhausted({}) }, [personal.analysis_id])
  const visible = (zone: string) => expanded[zone] ? pools[zone] : pools[zone].slice(offsets[zone] ?? 0, (offsets[zone] ?? 0) + 4)
  const nextBatch = (zone: string) => {
    const next = (offsets[zone] ?? 0) + 4
    if (next >= pools[zone].length) { setExhausted({ ...exhausted, [zone]: true }); return }
    setOffsets({ ...offsets, [zone]: next }); setExhausted({ ...exhausted, [zone]: next + 4 >= pools[zone].length })
  }
  const tracks = zones.flatMap((zone) => visible(zone)).map((item) => item.track)
  return <div className="page-width">
    <PageIntro eyebrow="EXPLAINABLE RECOMMENDATION" title="一条听得懂的探索路线" copy="真实模式以 Last.fm 听众相似关系、相似艺术家和关联标签生成候选，再由 Rust 本地评分；Genre 缺失不会淘汰强相似候选。" badge={`${tracks.length} TRACKS · ${personal.report.is_demo ? 'DEMO DATA' : 'REAL DATA · is_demo=false'}`} />
    {!personal.report.is_demo && <section className={`lastfm-readiness ${lastFmConfigured ? 'ready' : 'not-configured'}`} role="status"><strong>{lastFmConfigured ? '✓ Last.fm Recommendation Ready' : '⚠ Last.fm Recommendation Service Not Configured'}</strong>{lastFmConfigured ? <span>外部音乐推荐服务已配置；歌曲导入和音乐画像仍由各自的数据流程完成。</span> : <><span>当前环境未配置 LASTFM_API_KEY。</span><ul><li>不影响歌曲导入</li><li>不影响音乐画像分析</li><li>仅影响外部音乐推荐功能</li></ul><p>解决方式：管理员配置 Last.fm API Key。</p></>}</section>}
    <section className={`recommendation-status panel status-${summary.status}`}><div><span className="eyebrow">RECOMMENDATION SOURCE</span><h3>{summary.source_label}</h3><p>{summary.message}</p></div><div className="recommendation-coverage"><span><b>{personal.report.genre_matched_count}/{personal.report.track_count}</b>Genre 覆盖 · {Math.round(personal.report.genre_coverage * 100)}%</span><span><b>{personal.report.energy_matched_count}/{personal.report.track_count}</b>Energy 覆盖 · {Math.round(personal.report.energy_coverage * 100)}%</span><span><b>{summary.candidate_count}</b>真实候选</span><span><b>{personal.report.is_demo ? 'true' : 'false'}</b>is_demo</span></div><small>当前数据来源：{personal.report.source_label}</small></section>
    {!personal.report.is_demo && <section className="panel recommendation-evidence"><div><span className="eyebrow">SELECTED SEEDS</span><h3>实际采用的种子歌曲 · {summary.seeds.length} 首</h3><div className="seed-list">{summary.seeds.map((seed) => <span key={`${seed.title}-${seed.artists.join('-')}`}><b>{seed.title}</b><small>{seed.artists.join(', ')}</small></span>)}</div></div><div><span className="eyebrow">LAST.FM QUERY REPORT</span><div className="query-stats"><span><b>{stats.successful_seed_count}</b>成功种子</span><span><b>{stats.failed_seed_count}</b>失败种子</span><span><b>{stats.raw_track_similar_count}</b>track.getSimilar</span><span><b>{stats.raw_artist_similar_count}</b>artist.getSimilar</span><span><b>{stats.raw_artist_top_tracks_count}</b>artist.getTopTracks</span><span><b>{stats.raw_tag_top_tracks_count}</b>tag.getTopTracks</span><span><b>{stats.raw_candidate_count}</b>Last.fm 原始候选</span><span><b>{stats.after_version_filter_count}</b>版本过滤后</span><span><b>{stats.after_deduplication_count}</b>规范化去重后</span><span><b>{stats.after_source_exclusion_count}</b>排除原歌单后</span><span><b>{stats.after_artist_cap_count}</b>艺术家上限后</span><span><b>{stats.comfort_candidate_count}/{stats.expansion_candidate_count}/{stats.surprise_candidate_count}</b>候选池：舒适 / 拓展 / 惊喜</span><span><b>{stats.tag_layer1_candidate_count} / {stats.tag_layer1_rejected_count}</b>Tag 第一层获取 / 淘汰</span><span><b>{stats.tag_layer2_candidate_count} / {stats.tag_layer2_rejected_count}</b>Tag 第二层获取 / 淘汰</span><span><b>{stats.genre_bridge_candidate_count}/{stats.second_hop_artist_candidate_count}</b>Genre 桥梁 / 第二跳艺人</span><span><b>{stats.tag_similar_success_count}/{stats.tag_similar_failure_count}</b>tag.getSimilar 成功 / 失败</span><span><b>{stats.request_budget_used_count}/48 · retry {stats.retry_count}</b>请求预算 / 重试</span><span><b>{stats.request_budget_exhausted_count}</b>请求预算耗尽</span></div><p className="candidate-seed">核心标签：{stats.core_tags?.join(' · ') || '未获得'}<br/>第一层：{stats.layer1_tags?.join(' · ') || '空'}<br/>第二层：{stats.layer2_tags?.join(' · ') || '空'}</p></div></section>}
    {tracks.length > 0 && <ExportToolbar tracks={tracks} label={routeLabel} spotifyConnected={spotifyConnected} onExport={onExport}/>}
    {personal.route.length > 0 ? <section className="route-panel"><div className="route-line"/>{personal.route.map((step, index) => <div className="route-step" key={`${step.genre}-${index}`}><span className="step-number">0{index + 1}</span><div><strong>{step.genre}</strong><p>{step.explanation}</p><small>{step.tracks.map((track) => track.title).join(' · ')}</small></div>{index < personal.route.length - 1 && <b>→</b>}</div>)}</section> : <div className="zone-empty route-empty">暂无可验证的 Genre 路线；{summary.message}</div>}
    {zones.map((zone) => { const items = visible(zone); const pool = pools[zone]; const zoneSummary = summary.zones.find((item) => item.zone === zone); return <section className={`recommend-zone zone-${zone}`} key={zone}><div className="zone-heading"><span>{zone === '舒适区' ? '01' : zone === '拓展区' ? '02' : '03'}</span><div><h2>{zone === '舒适区' ? 'Comfort Zone' : zone === '拓展区' ? 'Expansion Zone' : 'Surprise Zone'} · {zone} <small>当前 {items.length} / 候选池 {pool.length} 首</small></h2><p>{zone === '舒适区' ? '从你熟悉的音乐类型出发，找到相近风格的新歌' : zone === '拓展区' ? '探索与你兴趣相关的新音乐方向' : '发现与你平时不同的音乐可能性'}</p></div></div>{items.length > 0 ? <><div className="recommend-grid">{items.map((item) => <RecommendationCard item={item} onAdd={(track) => onExport('youtube', [track], `${track.title} · MelodyPath 推荐`)} key={item.track.id}/>)}</div><div className="recommend-actions"><button className="secondary" disabled={expanded[zone] || exhausted[zone] || pool.length <= 4} onClick={() => nextBatch(zone)}>换一批</button><button className="text-button" disabled={pool.length <= 4} onClick={() => setExpanded({ ...expanded, [zone]: !expanded[zone] })}>{expanded[zone] ? '收起' : '查看更多'}</button>{(exhausted[zone] || pool.length <= 4) && <span className="candidate-seed">已看完本次候选</span>}</div></> : <div className="zone-empty"><strong>暂无足够的真实候选</strong><span>{zoneSummary?.message ?? summary.message}</span></div>}</section> })}
  </div>
}

function RecommendationCard({ item, onAdd }: { item: Recommendation; onAdd: (track: Track) => void }) {
  const [versionsOpen, setVersionsOpen] = useState(false)
  return <article className="recommend-card"><div className="recommend-top"><span className="album-art">{item.track.title.slice(0, 1)}</span><div><h3>{item.track.title}</h3><p>{item.track.artists.join(', ')}</p></div>{item.track.platform_url && <a href={item.track.platform_url} target="_blank" rel="noreferrer" aria-label="Last.fm 曲目页面">↗</a>}</div><div className="tag-row">{item.tags.length > 0 ? item.tags.map((tag) => <span key={tag}>{tag}</span>) : <span>标签未返回</span>}</div><p className="candidate-source">{item.candidate_source} · {item.source_endpoint} · UI 置信度 {Math.round(item.match_confidence * 100)}% · 系统综合分 {Math.round(item.match_score * 100)}% · 放宽级别 {item.relaxation_level}</p>{(item.seed_track || item.seed_artist) && <p className="candidate-seed">关联种子：{item.seed_track ? `《${item.seed_track}》` : item.seed_artist}{item.lastfm_similarity != null ? ` · Last.fm 原始相似度 ${Math.round(item.lastfm_similarity * 100)}%` : ''}</p>}<p className="reason">{item.reason}</p><div className="explain-pair"><div><small>连接依据</small><span>{item.connection}</span></div><div><small>拓展方向</small><span>{item.expansion}</span></div></div><div className="score-row"><Score label="匹配" value={item.match_score}/><Score label="新颖" value={item.novelty_score}/></div><div className="recommend-actions"><button className="secondary" onClick={() => onAdd(item.track)}>Add to playlist</button><button className="text-button version-explore" onClick={() => setVersionsOpen((value) => !value)}>{versionsOpen ? '收起其他版本' : 'Explore other versions'}</button></div>{versionsOpen && <AlternateVersionsPanel track={item.track} onAdd={onAdd}/>}</article>
}

function AlternateVersionsPanel({ track, onAdd }: { track: Track; onAdd: (track: Track) => void }) {
  const choices: { label: string; value: VersionType }[] = [{ label: 'Live', value: 'live' }, { label: 'Concert', value: 'concert' }, { label: 'Remix', value: 'remix' }, { label: 'Acoustic', value: 'acoustic' }, { label: 'Unplugged', value: 'unplugged' }, { label: 'Remaster', value: 'remastered' }]
  const [selected, setSelected] = useState<VersionType[]>(['live', 'remix', 'acoustic'])
  const [result, setResult] = useState<AlternateVersionSearchResult | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const toggle = (value: VersionType) => setSelected((current) => current.includes(value) ? current.filter((item) => item !== value) : [...current, value])
  const search = async (useMock = false) => { setBusy(true); setError(''); try { setResult(await api.searchAlternateVersions(track, selected, useMock)) } catch (reason) { setError(reason instanceof Error ? reason.message : '版本搜索失败') } finally { setBusy(false) } }
  const asTrack = (candidate: AlternateVersionSearchResult['candidates'][number]): Track => ({ id: candidate.source_url, title: candidate.title, normalized_title: candidate.title.toLowerCase(), artists: candidate.artists, genres: [], duration_ms: candidate.duration_ms, platform: candidate.platform, platform_url: candidate.source_url, external_ids: {}, version_type: candidate.version_type, mood_tags: [], metadata_confidence: candidate.match_confidence })
  return <div className="alternate-panel"><strong>{track.title} · 版本探索</strong><div className="version-options">{choices.map((choice) => <button className={selected.includes(choice.value) ? 'active' : ''} onClick={() => toggle(choice.value)} key={choice.value}>{choice.label}</button>)}</div><div className="alternate-actions"><button className="secondary" disabled={busy || selected.length === 0} onClick={() => void search(false)}>{busy ? '搜索中…' : '使用 YouTube 官方搜索'}</button><button className="text-button" disabled={busy} onClick={() => void search(true)}>用明确 Mock 验证流程</button></div>{error && <p className="error-box">{error}</p>}{result && <><p className={`alternate-status ${result.is_mock ? 'mock' : ''}`}><b>{result.provider} · {result.status}</b>{result.message}</p><div className="alternate-results">{result.candidates.map((candidate) => <div className="alternate-result-row" key={`${candidate.source_url}-${candidate.version_type}`}><a href={candidate.source_url} target="_blank" rel="noreferrer"><span>{candidate.version_type.replaceAll('_', ' ').toUpperCase()}</span><b>{candidate.title}</b><small>{candidate.artists.join(', ')} · {candidate.official_status} · {Math.round(candidate.match_confidence * 100)}%</small><em>{candidate.reason}</em></a><button className="secondary" onClick={() => onAdd(asTrack(candidate))}>Add to playlist</button></div>)}</div>{result.candidates.length === 0 && <div className="zone-empty">Original unavailable or no verified alternate on this provider.</div>}</>}</div>
}
function Score({ label, value }: { label: string; value: number }) { return <div><span>{label}</span><i><b style={{ width: `${value * 100}%` }}/></i><strong>{Math.round(value * 100)}</strong></div> }

function ComparePage({ demoReport, capabilities, onExport, onBinding }: { demoReport: ComparisonReport; capabilities: PlatformCapability[]; onExport: (platform: string, tracks: Track[], label: string) => void; onBinding: (binding: { analysis_a_id: string; analysis_b_id: string } | null) => void }) {
  const [previewA, setPreviewA] = useState<ImportPreview | null>(null)
  const [previewB, setPreviewB] = useState<ImportPreview | null>(null)
  const [textA, setTextA] = useState('')
  const [textB, setTextB] = useState('')
  const [report, setReport] = useState<ComparisonReport | null>(null)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const [sourceA, setSourceA] = useState('file')
  const [sourceB, setSourceB] = useState('file')
  const [progress, setProgress] = useState<TaskProgressState | null>(null)
  const prepare = async (side: 'a' | 'b', request: ImportPreviewRequest) => {
    setBusy(true); setError(''); setReport(null); setProgress({ stage: 'parsing', detail: `Friend ${side.toUpperCase()} · 正在解析歌曲` })
    try {
      const preview = await api.previewImport(request)
      if (side === 'a') setPreviewA(preview); else setPreviewB(preview); setProgress({ stage: 'completed', detail: `Friend ${side.toUpperCase()} · ${preview.parsed_count} 首解析完成` })
    } catch (reason) { const message = reason instanceof Error ? reason.message : '歌单解析失败'; setError(message); setProgress({ stage: 'failed', error: message }) }
    finally { setBusy(false) }
  }
  const loadFile = async (side: 'a' | 'b', file?: File) => {
    if (!file) return
    setBusy(true); setError(''); setProgress({ stage: 'uploading', detail: `Friend ${side.toUpperCase()} · ${file.name}` })
    try {
      const content = await readPlaylistFile(file)
      const format = file.name.split('.').pop()?.toLowerCase() ?? ''
      await prepare(side, { name: file.name.replace(/\.[^.]+$/, ''), file_name: file.name, format, content, data_state: 'REAL_FILE' })
    } catch (reason) { const message = reason instanceof Error ? reason.message : '文件读取失败'; setError(message); setProgress({ stage: 'failed', error: message }); setBusy(false) }
  }
  const compare = async () => {
    if (!previewA || !previewB) return
    setBusy(true); setError(''); setProgress({ stage: 'resolving_metadata', detail: 'Friend A · 正在补全元数据' })
    try {
      const analysisA = await api.analyzeImport(previewA.id, stage => setProgress({ stage, detail: 'Friend A' }))
      setProgress({ stage: 'resolving_metadata', detail: 'Friend B · 正在补全元数据' })
      const analysisB = await api.analyzeImport(previewB.id, stage => setProgress({ stage, detail: 'Friend B' }))
      setProgress({ stage: 'analyzing_taste', detail: '正在寻找两位用户之间的音乐连接点' })
      setReport(await api.compareAnalyses(analysisA, analysisB))
      onBinding({ analysis_a_id: analysisA.analysis_id, analysis_b_id: analysisB.analysis_id })
      setProgress({ stage: 'completed', detail: 'Friend Bridge 已完成' })
    } catch (reason) { const message = reason instanceof Error ? reason.message : '比较失败'; setError(message); setProgress({ stage: 'failed', error: message }) }
    finally { setBusy(false) }
  }
  const inputCard = (side: 'a' | 'b', preview: ImportPreview | null, text: string, setText: (value: string) => void) => {
    const source = side === 'a' ? sourceA : sourceB
    const setSource = side === 'a' ? setSourceA : setSourceB
    const unavailable = source === 'spotify' || source === 'youtube'
    return <section className="panel compare-input"><span className="eyebrow">FRIEND {side.toUpperCase()}</span><h3>{side === 'a' ? '第一份歌单' : '第二份歌单'}</h3><label className="field"><span>选择来源</span><select value={source} onChange={(event) => setSource(event.target.value)}><option value="file">Local file</option><option value="pasted">Pasted tracks</option><option value="netease">NetEase import</option><option value="qq_music">QQ Music import</option><option value="kugou">Kugou import</option><option value="spotify" disabled={!capabilities.find((item) => item.platform === 'spotify')?.compare_supported}>Spotify account</option><option value="youtube" disabled={!capabilities.find((item) => item.platform === 'youtube_music')?.compare_supported}>YouTube account</option></select></label>{unavailable ? <p className="privacy-note">该账号平台的数据政策不允许用于当前跨平台衍生比较；请上传自己导出的歌单文件。</p> : <><p>{['netease', 'qq_music', 'kugou'].includes(source) ? '该平台目前无法通过已验证的官方 API 直接读取，请上传导出的歌单或粘贴歌曲清单。' : '数据只用于本次临时比较。'}</p>{source !== 'pasted' && <label className="secondary upload-button">选择文件<input type="file" accept=".txt,.csv,.tsv,.json,.m3u,.m3u8" onChange={(event) => void loadFile(side, event.target.files?.[0])}/></label>}<textarea rows={4} value={text} placeholder="歌手 - 歌名" onChange={(event) => setText(event.target.value)}/><button className="secondary" disabled={!text.trim() || busy} onClick={() => void prepare(side, { name: `Friend ${side.toUpperCase()}`, format: 'txt', content: text, data_state: 'REAL_TEXT' })}>解析文本</button></>}{preview && <div className="compare-preview"><strong>{preview.source_label}</strong><span>{preview.parsed_count}/{preview.total_rows} 首解析成功 · {preview.warning_count} 个警告</span>{preview.preview_tracks.slice(0, 3).map((track) => <small key={`${track.title}-${track.artists.join()}`}>{track.title} — {track.artists.join(', ')}</small>)}</div>}</section>
  }
  if (!report) return <div className="page-width"><PageIntro eyebrow="Compare · TEMPORARY FRIEND COMPARE" title="两份真实歌单，一次私密比较" copy="Friend A 与 Friend B 默认都从 Local file 开始，也可改用 TXT/CSV/JSON/M3U 文本导入。比较只寻找双方的音乐连接点，不要求连接第三方账号。" badge="/compare · LOCAL FILE DEFAULT"/><div className="compare-input-grid">{inputCard('a', previewA, textA, setTextA)}{inputCard('b', previewB, textB, setTextB)}</div>{progress && <TaskProgress state={progress}/>} {error && <p className="error-box">{error}</p>}<div className="compare-actions"><button className="secondary" onClick={() => { setReport(demoReport); onBinding(null) }}>查看明确标注的 Demo</button><button className="primary big" disabled={!previewA || !previewB || busy} onClick={() => void compare()}>{busy ? '正在分析两份歌单…' : '确认并开始临时比较'}</button></div><p className="privacy-note">不会建立公开社交账号；不会默认保存好友歌单。默认输入始终是 Local file；在线平台连接不是当前 Friend Bridge 的前置条件。</p></div>
  return <ComparisonResults report={report} onReset={() => { setReport(null); onBinding(null) }} onExport={onExport}/>
}

function ComparisonResults({ report, onReset, onExport }: { report: ComparisonReport; onReset: () => void; onExport: (platform: string, tracks: Track[], label: string) => void }) {
  const tracks = report.bridge_playlist.map((item) => item.track)
  const compatibility = report.metrics.find((metric) => metric.label === 'Overall Compatibility')?.value ?? report.metrics[0]?.value ?? 0
  return <div className="page-width"><PageIntro eyebrow="CROSS-PLAYLIST FRIEND MATCH" title="两种品味，一座声音桥梁" copy={report.summary} badge={`A × B · ${report.is_demo ? 'DEMO' : 'REAL · is_demo=false'}`}/><button className="secondary compare-reset" onClick={onReset}>重新比较</button><div className="people-card"><Person label={report.user_a} initials="A" values={report.user_a_signatures} tone="coral"/><div className="compatibility"><span>综合兼容度</span><strong>{Math.round(compatibility * 100)}</strong><small>%</small><i>{report.shared_track_count} 共同曲目 · {report.shared_artists.length} 共同艺人</i></div><Person label={report.user_b} initials="B" values={report.user_b_signatures} tone="blue"/></div><div className="comparison-facts"><span><b>{report.track_count_a}</b>A 曲目</span><span><b>{report.track_count_b}</b>B 曲目</span><span><b>{report.shared_genres.length}</b>共同 Genre</span><span><b>{report.saved_locally ? '是' : '否'}</b>保存好友数据</span><span><b>{report.data_source}</b>数据来源</span></div><div className="metric-grid comparison-metrics">{report.metrics.map((metric) => <MetricCard key={metric.label} {...metric}/>)}</div><section className="panel bridge-section"><div className="panel-title"><div><span className="eyebrow">MUTUAL DISCOVERY</span><h3>Safe for Both / Bridge / Adventure Together</h3></div><span>排除双方源歌单 · 确定性评分</span></div>{tracks.length > 0 && <ExportToolbar tracks={tracks} label="A 与 B 的共同推荐" onExport={onExport}/>}<div className="bridge-list">{report.bridge_playlist.map((item, index) => <BridgeRow item={item} index={index} onAdd={(track) => onExport('youtube', [track], `${track.title} · 共同推荐`)} key={item.track.id}/>)}</div>{tracks.length === 0 && <div className="zone-empty"><strong>暂无同时满足双方关联的真实候选</strong><span>没有使用 Demo 或固定曲目补齐；可在配置真实 Last.fm 后重新分析。</span></div>}</section></div>
}
function Person({ label, initials, values, tone }: { label: string; initials: string; values: string[]; tone: string }) { return <div className={`person ${tone}`}><span>{initials}</span><div><strong>{label}</strong><p>{values.join(' · ')}</p></div></div> }
function BridgeRow({ item, index, onAdd }: { item: BridgeTrack; index: number; onAdd: (track: Track) => void }) { return <div className="bridge-row"><span className="bridge-index">{String(index + 1).padStart(2, '0')}</span><span className="album-placeholder">♪</span><div className="bridge-track"><strong>{item.track.title}</strong><small>{item.track.artists.join(', ')} · {item.track.genres.join(' / ') || 'Genre 未匹配'}</small><em>A：{item.reason_for_a} · B：{item.reason_for_b}</em><em>{item.candidate_source} · {item.shared_basis.join(' / ') || '跨画像关联'}</em></div><span className="phase-chip">{item.phase}</span><p>{item.reason}<button className="text-button" onClick={() => onAdd(item.track)}>Add to playlist</button></p><b>{Math.round(item.bridge_score * 100)}</b></div> }

function ExportToolbar({ tracks, label, spotifyConnected = true, onExport }: { tracks: Track[]; label: string; spotifyConnected?: boolean; onExport: (platform: string, tracks: Track[], label: string) => void }) { return <div className="export-toolbar"><div><span className="eyebrow">SAVE YOUR PATH</span><strong>选择歌曲，预览匹配，再创建新歌单</strong></div><div>{spotifyConnected && <button className="spotify-button" onClick={() => onExport('spotify', tracks, label)}>Create Spotify Playlist</button>}<button onClick={() => onExport('apple_music', tracks, label)}>♪ 保存到 Apple Music</button><button onClick={() => onExport('youtube', tracks, label)}>▶ 保存到 YouTube</button><button className="export-button" onClick={() => onExport('file', tracks, label)}>⇩ 导出歌曲清单</button><button className="demo-flow-button" onClick={() => onExport('demo', tracks, label)}>演示写入流程</button></div></div> }

function VersionsPage({ currentAnalysis, youtube, onAddPreview }: { currentAnalysis: PersonalAnalysis | null; youtube: YouTubeConnectionStatus | null; onAddPreview: (track: Track) => void }) {
  const [tracks, setTracks] = useState<Track[]>([])
  const [source, setSource] = useState('尚未选择歌单')
  const [playlists, setPlaylists] = useState<YouTubePlaylistSummary[]>([])
  const [results, setResults] = useState<Awaited<ReturnType<typeof api.discoverVersions>>[]>([])
  const [busy, setBusy] = useState(false)
  const [progress, setProgress] = useState(0)
  const [error, setError] = useState('')
  const [releaseResult, setReleaseResult] = useState<ReleaseRadarResult | null>(null)
  const [releaseBusy, setReleaseBusy] = useState(false)
  const [sourceProgress, setSourceProgress] = useState<TaskProgressState | null>(null)
  const [scanned, setScanned] = useState(false)
  const cancelled = useRef(false)
  const [preferences, setPreferences] = useState<string[]>([])
  const [tastePreferences, setTastePreferences] = useState<string[]>([])
  const profilePreferences = (analysis: PersonalAnalysis) => [...(analysis.taste_profile?.genres ?? []).slice(0, 3).map(g => g.name.toLowerCase()), ...(analysis.taste_profile?.language_distribution ?? []).slice(0, 2).map(l => ({ English: 'en', Chinese: 'zh', Japanese: 'ja', Korean: 'ko' }[l.name] ?? l.name.toLowerCase()))]
  const useCurrent = () => { if (currentAnalysis) { setTracks(currentAnalysis.playlist.tracks); setTastePreferences(profilePreferences(currentAnalysis)); setSource(`当前分析 · ${currentAnalysis.playlist.name}`); setResults([]); setReleaseResult(null); setProgress(0); setScanned(false) } }
  const loadFile = async (file?: File) => {
    if (!file) return
    setBusy(true); setError(''); setSourceProgress({ stage: 'uploading', detail: file.name })
    try { const content = await readPlaylistFile(file); const format = file.name.split('.').pop()?.toLowerCase() ?? ''; setSourceProgress({ stage: 'parsing', detail: file.name }); const preview = await api.previewImport({ name: file.name.replace(/\.[^.]+$/, ''), file_name: file.name, format, content, data_state: 'REAL_FILE' }); const analysis = await api.analyzeImport(preview.id, stage => setSourceProgress({ stage, detail: file.name })); setTracks(analysis.playlist.tracks); setTastePreferences(profilePreferences(analysis)); setSource(`真实文件 · ${file.name}`); setResults([]); setReleaseResult(null); setProgress(0); setScanned(false); setSourceProgress({ stage: 'completed', detail: `${analysis.playlist.tracks.length} 首歌曲` }) }
    catch (reason) { const message = reason instanceof Error ? reason.message : '文件解析失败'; setError(message); setSourceProgress({ stage: 'failed', error: message }) } finally { setBusy(false) }
  }
  const loadYoutube = async () => { setBusy(true); setError(''); setSourceProgress({ stage: 'uploading', detail: '正在读取 YouTube 播放列表' }); try { setPlaylists(await api.youtubePlaylists()); setSourceProgress(null) } catch (reason) { const message = reason instanceof Error ? reason.message : '无法读取 YouTube 播放列表'; setError(message); setSourceProgress({ stage: 'failed', error: message }) } finally { setBusy(false) } }
  const chooseYoutube = async (playlist: YouTubePlaylistSummary) => { setBusy(true); setError(''); setSourceProgress({ stage: 'parsing', detail: playlist.name }); try { const imported = await api.importYouTubePlaylists([playlist.id]); setTracks(imported.tracks); setTastePreferences([]); setSource(`YouTube 官方 OAuth · ${playlist.name}`); setResults([]); setReleaseResult(null); setProgress(0); setScanned(false); setSourceProgress({ stage: 'completed', detail: `Imported ${imported.track_count}/${imported.track_count} tracks` }) } catch (reason) { const message = reason instanceof Error ? reason.message : 'YouTube 歌单读取失败'; setError(message); setSourceProgress({ stage: 'failed', error: message }) } finally { setBusy(false) } }
  const scanReleases = async () => { setReleaseBusy(true); setError(''); setReleaseResult(null); try { setReleaseResult(await api.scanReleaseRadar(tracks)) } catch (reason) { const message = reason instanceof Error ? reason.message : '版本雷达更新查询失败'; setError(message); setReleaseResult({ provider: 'MusicBrainz public metadata', status: 'error', message, new_releases: [], upcoming_albums: [], artist_updates: [] }) } finally { setReleaseBusy(false) } }
  const scan = async () => {
    const limited = tracks.slice(0, 40)
    cancelled.current = false; setBusy(true); setError(''); setResults([]); setProgress(0); setScanned(false)
    try {
      for (let index = 0; index < limited.length; index += 4) {
        if (cancelled.current) break
        const batch = limited.slice(index, index + 4)
        const found = await Promise.all(batch.map((track) => api.discoverVersions(track, preferences.length ? preferences : [...tastePreferences, ...tracks.filter(t => t.platform !== 'spotify').flatMap(t => [t.version_type, ...(t.language ? [t.language] : [])])])))
        if (cancelled.current) break
        setResults((current) => [...current, ...found]); setProgress(Math.min(1, (index + batch.length) / limited.length))
      }
      setScanned(true)
    } catch (reason) { setError(reason instanceof Error ? reason.message : '版本扫描失败'); setScanned(true) } finally { setBusy(false) }
  }
  const cancel = () => { cancelled.current = true; setBusy(false) }
  const candidateTrack = (candidate: Awaited<ReturnType<typeof api.discoverVersions>>['candidates'][number]): Track => ({ id: candidate.url, title: candidate.title, normalized_title: candidate.title.toLowerCase(), artists: [candidate.artist], genres: [],  platform: candidate.platform, platform_url: candidate.url, external_ids: {}, version_type: candidate.version_type === 'language_cover' ? 'cover' : candidate.version_type === 'extended' ? 'other' : candidate.version_type as VersionType, mood_tags: [], metadata_confidence: candidate.confidence })
  const alternateCount = results.reduce((sum, result) => sum + result.candidates.length, 0)
  return <div className="page-width"><PageIntro eyebrow="VERSION RADAR" title="发行动态与不同录音版本" copy="从真实公开元数据查看 New releases、Upcoming albums、Artist updates；跨 Spotify、YouTube、MusicBrainz 理解歌曲、分类版本，并结合偏好排序。无数据、加载和错误状态都会明确显示。" badge="/versions · REAL DATA STATES"/><section className="panel transfer-source"><div className="panel-title"><div><span className="eyebrow">PLAYLIST SOURCE</span><h3>{source}</h3></div><span>{tracks.length} 首</span></div><div className="transfer-source-actions">{currentAnalysis && !currentAnalysis.report.is_demo && <button className="secondary" onClick={useCurrent}>当前已分析歌单</button>}<label className="secondary upload-button">本地文件<input type="file" accept=".txt,.csv,.tsv,.json,.m3u,.m3u8" onChange={(event) => void loadFile(event.target.files?.[0])}/></label>{youtube?.connected ? <button className="secondary" onClick={() => void loadYoutube()}>YouTube 账号歌单</button> : <button className="secondary" disabled>YouTube OAuth 未连接</button>}</div>{playlists.length > 0 && <div className="transfer-playlist-picker">{playlists.map((playlist) => <button onClick={() => void chooseYoutube(playlist)} key={playlist.id}><b>{playlist.name}</b><span>{playlist.item_count} 首</span></button>)}</div>}{sourceProgress && <TaskProgress state={sourceProgress}/>}<p className="privacy-note">发行动态来自 MusicBrainz 公开元数据，最多查询 5 位歌手；日期缺失的条目不会伪造成更新。不同版本扫描最多 40 首且不会修改源歌单。</p><div className="version-options" aria-label="Version preferences">{['original', 'live', 'acoustic', 'cover', 'language_cover', 'remix', 'instrumental', 'extended', 'ko', 'zh', 'ja'].map(type => <button key={type} className={preferences.includes(type) ? 'active' : ''} onClick={() => setPreferences(current => current.includes(type) ? current.filter(t => t !== type) : [...current, type])}>{type.replaceAll('_', ' ')}</button>)}</div><p>默认参考当前歌单的版本与语言偏好；也可手动选择兴趣。Match 为确定性匹配分数，不是正确率。</p><div className="transfer-preview-actions"><button className="primary" disabled={releaseBusy || tracks.length === 0} onClick={() => void scanReleases()}>{releaseBusy ? 'Loading release updates...' : '检查发行动态'}</button><button className="secondary" disabled={busy || tracks.length === 0} onClick={() => void scan()}>{busy ? `扫描中 ${Math.round(progress * 100)}%` : '发现跨平台版本'}</button>{busy && <button className="danger-button" onClick={cancel}>取消扫描</button>}</div>{error && <p className="error-box">{error}</p>}</section><section className={`panel release-radar status-${releaseResult?.status ?? 'idle'}`}><div className="panel-title"><div><span className="eyebrow">RELEASE RADAR</span><h3>{releaseBusy ? 'Loading...' : releaseResult?.provider ?? 'MusicBrainz public metadata'}</h3></div><span>{releaseResult?.status ?? 'idle'}</span></div>{releaseResult?.status === 'error' && <p className="error-box">{releaseResult.message}</p>}{releaseResult && releaseResult.status !== 'error' && <p>{releaseResult.message}</p>}<div className="release-radar-grid"><ReleaseColumn title="New releases" items={releaseResult?.new_releases ?? []} loading={releaseBusy}/><ReleaseColumn title="Upcoming albums" items={releaseResult?.upcoming_albums ?? []} loading={releaseBusy}/><ReleaseColumn title="Artist updates" items={releaseResult?.artist_updates ?? []} loading={releaseBusy}/></div></section><section className="panel version-radar-results"><div className="panel-title"><div><span className="eyebrow">ALTERNATE VERSIONS</span><h3>Alternative Versions</h3></div><span>{alternateCount} 个版本</span></div>{results.map((result) => <article className="alternate-panel" key={result.source_track.id}><strong>{result.source_track.title} — {result.source_track.artists.join(', ')}</strong><small>{result.provider_status.join(' · ')} · {result.status}</small><div className="alternate-results">{result.candidates.map((candidate) => <div className="alternate-result-row" key={`${candidate.url}-${candidate.version_type}`}><a href={candidate.url} target="_blank" rel="noreferrer"><span>{candidate.version_type.toUpperCase()}</span><b>{candidate.title}</b><small>{candidate.platform} · {candidate.language ?? '语言未知'} · Match: {Math.round(candidate.confidence * 100)}%</small><em>{candidate.reason}</em></a><button className="secondary" onClick={() => onAddPreview(candidateTrack(candidate))}>选择并进入 Add to playlist 预览</button></div>)}</div>{result.candidates.length === 0 && <div className="zone-empty">No update available</div>}</article>)}{scanned && alternateCount === 0 && <div className="zone-empty">No update available</div>}{!scanned && <div className="zone-empty">选择来源后开始扫描；页面不会以空白表示状态。</div>}</section></div>
}

function ReleaseColumn({ title, items, loading }: { title: string; items: ReleaseUpdate[]; loading: boolean }) {
  return <div><h4>{title}</h4>{loading ? <p>Loading...</p> : items.length > 0 ? items.map((item) => <a href={item.source_url} target="_blank" rel="noreferrer" key={`${item.source_url}-${title}`}><strong>{item.title}</strong><span>{item.artist} · {item.release_type} · {item.release_date}</span></a>) : <p>No update available</p>}</div>
}

function TransferPage({ spotify, youtube, capabilities, currentAnalysis }: { spotify: SpotifyConnectionStatus | null; youtube: YouTubeConnectionStatus | null; capabilities: PlatformCapability[]; currentAnalysis: PersonalAnalysis | null }) {
  const spotifyWrite = useSpotifyWrite()
  const [sourceName, setSourceName] = useState('待复制歌单')
  const [tracks, setTracks] = useState<Track[]>([])
  const [sourceLabel, setSourceLabel] = useState('尚未选择来源')
  const [spotifyPlaylists, setSpotifyPlaylists] = useState<SpotifyPlaylistSummary[]>([])
  const [youtubePlaylists, setYoutubePlaylists] = useState<YouTubePlaylistSummary[]>([])
  const [sourcePlatform, setSourcePlatform] = useState<'spotify' | 'youtube' | 'import'>('import')
  const [destinationPlatform, setDestinationPlatform] = useState<'spotify' | 'youtube'>('youtube')
  const [preview, setPreview] = useState<TransferPreview | null>(null)
  const [result, setResult] = useState<TransferResult | null>(null)
  const [transferRun, setTransferRun] = useState<TransferRun | null>(null)
  const [allowAlternate, setAllowAlternate] = useState(false)
  const [selections, setSelections] = useState<Record<string, string>>({})
  const [confirmed, setConfirmed] = useState(false)
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const transferEvents = useRef<EventSource | null>(null)
  useEffect(() => () => transferEvents.current?.close(), [])
  const loadSpotifyPlaylists = async () => { setBusy(true); setError(''); try { setSpotifyPlaylists(await api.spotifyPlaylists()) } catch (reason) { setError(reason instanceof Error ? reason.message : '无法读取 Spotify 歌单') } finally { setBusy(false) } }
  const chooseSpotify = async (playlist: SpotifyPlaylistSummary) => { setBusy(true); setError(''); try { const imported = await api.importSpotifyPlaylists([playlist.id]); setTracks(imported.tracks); setSourcePlatform('spotify'); setDestinationPlatform('youtube'); setSourceName(playlist.name); setSourceLabel(`Spotify 官方 API · ${imported.track_count} 首`); setPreview(null); setResult(null) } catch (reason) { setError(reason instanceof Error ? reason.message : '读取 Spotify 歌单失败') } finally { setBusy(false) } }
  const loadYoutubePlaylists = async () => { setBusy(true); setError(''); try { setYoutubePlaylists(await api.youtubePlaylists()) } catch (reason) { setError(reason instanceof Error ? reason.message : '无法读取 YouTube 播放列表') } finally { setBusy(false) } }
  const chooseYoutube = async (playlist: YouTubePlaylistSummary) => { setBusy(true); setError(''); try { const imported = await api.importYouTubePlaylists([playlist.id]); setTracks(imported.tracks); setSourcePlatform('youtube'); setDestinationPlatform('spotify'); setSourceName(playlist.name); setSourceLabel(`YouTube 官方 API · ${imported.track_count} 首`); setPreview(null); setResult(null) } catch (reason) { setError(reason instanceof Error ? reason.message : '读取 YouTube 播放列表失败') } finally { setBusy(false) } }
  const loadFile = async (file?: File) => { if (!file) return; setBusy(true); setError(''); try { const content = await readPlaylistFile(file); const format = file.name.split('.').pop()?.toLowerCase() ?? ''; const parsed = await api.previewImport({ name: file.name.replace(/\.[^.]+$/, ''), file_name: file.name, format, content, data_state: 'REAL_FILE' }); const analysis = await api.analyzeImport(parsed.id); setTracks(analysis.playlist.tracks); setSourcePlatform('import'); setSourceName(analysis.playlist.name); setSourceLabel(`真实文件 · ${parsed.parsed_count}/${parsed.total_rows} 首解析成功`); setPreview(null); setResult(null) } catch (reason) { setError(reason instanceof Error ? reason.message : '文件读取失败') } finally { setBusy(false) } }
  const useCurrent = () => { if (!currentAnalysis) return; setTracks(currentAnalysis.playlist.tracks); setSourcePlatform('import'); setSourceName(currentAnalysis.playlist.name); setSourceLabel(`当前分析 · ${currentAnalysis.playlist.tracks.length} 首`); setPreview(null); setResult(null) }
  const prepare = async (useMock: boolean) => { setBusy(true); setError(''); setResult(null); setTransferRun(null); setSelections({}); setConfirmed(false); if (!useMock && sourcePlatform === destinationPlatform) { setError('来源与目标必须是不同平台'); setBusy(false); return } try { setPreview(await api.previewTransfer(sourceName, tracks, destinationPlatform, allowAlternate, useMock)) } catch (reason) { setError(reason instanceof Error ? reason.message : '生成复制预览失败') } finally { setBusy(false) } }
  const watchTransferRun = (id: string) => new Promise<void>((resolve, reject) => {
    transferEvents.current?.close()
    const events = new EventSource(`/api/transfers/runs/${id}/events`)
    transferEvents.current = events
    events.addEventListener('transfer', (event) => {
      const current = JSON.parse((event as MessageEvent).data) as TransferRun
      setTransferRun(current)
      if (current.result) setResult(current.result)
      if (['COMPLETED', 'FAILED', 'CANCELLED'].includes(current.status)) { events.close(); transferEvents.current = null; resolve() }
    })
    events.onerror = async () => {
      events.close(); transferEvents.current = null
      try {
        const current = await api.transferRun(id)
        setTransferRun(current)
        if (current.result) setResult(current.result)
        if (['COMPLETED', 'FAILED', 'CANCELLED'].includes(current.status)) resolve()
        else reject(new Error('复制进度连接中断，请重试状态查询'))
      } catch (reason) { reject(reason) }
    }
  })
  const execute = async () => { if (!preview || !confirmed) return; setBusy(true); setError(''); try { if (!preview.is_mock && destinationPlatform === 'spotify' && !await spotifyWrite.check(execute)) return; const created = await api.createTransferRun(preview.preview_id, selections); setTransferRun(created); await watchTransferRun(created.id) } catch (reason) { if (destinationPlatform === 'spotify') spotifyWrite.failed(reason, execute); setError(reason instanceof Error ? reason.message : '复制执行失败') } finally { setBusy(false) } }
  const cancelTransfer = async () => { if (!transferRun) return; try { setTransferRun(await api.cancelTransferRun(transferRun.id)) } catch (reason) { setError(reason instanceof Error ? reason.message : '取消复制失败') } }
  const resumeTransfer = async () => { if (!transferRun) return; setBusy(true); setError(''); try { const resumed = await api.resumeTransferRun(transferRun.id); setTransferRun(resumed); await watchTransferRun(resumed.id) } catch (reason) { setError(reason instanceof Error ? reason.message : '恢复复制失败') } finally { setBusy(false) } }
  const unresolved = preview?.matches.filter((item) => item.requires_confirmation && !selections[item.source_track.source_track_id]).length ?? 0
  const destinationCapability = capabilities.find((item) => item.platform === (destinationPlatform === 'youtube' ? 'youtube_music' : 'spotify'))
  if (capabilities.length > 0) return <div className="page-width">
    <PageIntro eyebrow="COPY PLAYLIST AGENT" title="选择来源与目标，预览后再复制" copy="Spotify 与 YouTube 只通过官方 OAuth 连接；文件导入同样转换为统一曲目。歧义未确认前不会创建目标播放列表，原歌单不会被修改或删除。" badge="/transfer · PRIVATE BY DEFAULT"/>
    {destinationPlatform === 'spotify' && spotifyWrite.prompt}<section className="transfer-connections"><ConnectionCard name="Spotify" status={spotify}/><ConnectionCard name="YouTube" status={youtube}/></section>{youtube?.configured && !youtube.write_authorized && <div className="policy-box"><p>读取和版本搜索只需只读权限。创建歌单需要 Google 较宽的写权限（平台没有仅限歌单写入的 scope）；MelodyPath 只执行经确认的新建私有歌单和添加视频。</p><a href="/api/youtube/authorize?write=true">仅当需要 Copy Playlist 时授权写入</a></div>}
    <section className="panel transfer-source">
      <div className="panel-title"><div><span className="eyebrow">CHOOSE SOURCE</span><h3>选择一个只读来源</h3></div><span>{sourceLabel}</span></div>
      <div className="transfer-source-actions">
        {spotify?.connected ? <button className="spotify-button" onClick={() => void loadSpotifyPlaylists()}>Spotify 歌单</button> : spotify?.configured ? <a className="button-link spotify-button" href="/api/spotify/authorize">连接 Spotify</a> : <button className="secondary" disabled>Spotify 未配置</button>}
        {youtube?.connected ? <button className="secondary" onClick={() => void loadYoutubePlaylists()}>YouTube 播放列表</button> : youtube?.configured ? <a className="button-link secondary" href="/api/youtube/authorize">连接 YouTube</a> : <button className="secondary" disabled>YouTube 未配置</button>}
        <label className="secondary upload-button">本地文件<input type="file" accept=".txt,.csv,.tsv,.json,.m3u,.m3u8" onChange={(event) => void loadFile(event.target.files?.[0])}/></label>
        {currentAnalysis && !currentAnalysis.report.is_demo && <button className="secondary" onClick={useCurrent}>已有 MelodyPath 分析</button>}
      </div>
      {spotifyPlaylists.length > 0 && <div className="transfer-playlist-picker">{spotifyPlaylists.map((playlist) => <button onClick={() => void chooseSpotify(playlist)} key={playlist.id}><b>{playlist.name}</b><span>{playlist.track_count} 首 · {playlist.owner_name}</span></button>)}</div>}
      {youtubePlaylists.length > 0 && <div className="transfer-playlist-picker">{youtubePlaylists.map((playlist) => <button onClick={() => void chooseYoutube(playlist)} key={playlist.id}><b>{playlist.name}</b><span>{playlist.item_count} 首 · YouTube</span></button>)}</div>}
      <div className="transfer-source-summary"><strong>{sourceName}</strong><span>{tracks.length} 首 · 源歌单永远不会被修改或删除</span></div>
      <label className="field"><span>Destination（只显示已有官方 Writer 的平台）</span><select value={destinationPlatform} onChange={(event) => { setDestinationPlatform(event.target.value as 'spotify' | 'youtube'); setPreview(null); setResult(null) }}><option value="youtube" disabled={!capabilities.find((item) => item.platform === 'youtube_music')?.playlist_write_supported}>YouTube · 新建私有播放列表</option><option value="spotify" disabled={!capabilities.find((item) => item.platform === 'spotify')?.playlist_write_supported}>Spotify · 新建私有播放列表</option></select></label>
      <p className="privacy-note">{destinationCapability?.reason}</p>
      <label className="confirm-check"><input type="checkbox" checked={allowAlternate} onChange={(event) => setAllowAlternate(event.target.checked)}/><span><b>原版不可用时允许 Live / Remix / Acoustic</b><small>默认关闭；开启后会显示版本类型与匹配理由，并要求确认。</small></span></label>
      <button className="primary big full" disabled={tracks.length === 0 || busy || !destinationCapability?.configured} onClick={() => void prepare(false)}>{busy ? '正在搜索候选…' : `匹配到 ${destinationPlatform === 'youtube' ? 'YouTube' : 'Spotify'} 并生成预览`}</button>
      {!destinationCapability?.configured && <p className="error-box">目标平台属于 IMPLEMENTED BUT UNCONFIGURED；需要完成官方 OAuth 开发者配置，当前不会用 Mock 冒充连接。</p>}
    </section>
    {error && <p className="error-box">{error}</p>}
    {preview && <section className="panel transfer-preview"><div className="panel-title"><div><span className="eyebrow">TRANSFER PREVIEW</span><h3>{preview.provider} · {preview.status}</h3></div><span>{preview.is_mock ? 'MOCK ONLY' : 'REAL PROVIDER'}</span></div><p>{preview.message}</p><div className="result-stats"><div><strong>{preview.source_count}</strong><span>源歌曲</span></div><div><strong>{preview.high_confidence_count}</strong><span>高置信</span></div><div><strong>{preview.ambiguous_count}</strong><span>歧义</span></div><div><strong>{preview.unmatched_count}</strong><span>未匹配</span></div></div><div className="transfer-match-list">{preview.matches.map((match) => <div className={`transfer-match ${match.status.toLowerCase()}`} key={match.source_track.source_track_id}><div><b>{match.source_track.title}</b><small>{match.source_track.artists.join(', ')} · {match.source_track.version_type}</small></div><span>{match.status} · {Math.round(match.score * 100)}%</span><p>{match.reason}</p>{match.requires_confirmation && <select value={selections[match.source_track.source_track_id] ?? ''} onChange={(event) => setSelections({ ...selections, [match.source_track.source_track_id]: event.target.value })}><option value="">选择候选或跳过</option>{match.candidates.map((candidate) => <option value={candidate.target_id} key={candidate.target_id}>{candidate.title} — {candidate.artists.join(', ')} · {candidate.version_type} · {Math.round(candidate.score * 100)}%</option>)}<option value="SKIP">跳过</option></select>}<details><summary>查看候选与确定性评分依据</summary>{match.candidates.map((candidate) => <div className="transfer-candidate" key={candidate.target_id}><b>{candidate.title}</b><span>{candidate.reason}</span></div>)}</details></div>)}</div>{preview.matches.length > 0 && <><label className="confirm-check"><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)}/><span><b>我已检查结果，确认创建新的私有播放列表</b><small>所有歧义必须改选或跳过；源歌单只读。</small></span></label><button className="primary big full" disabled={!confirmed || unresolved > 0 || busy} onClick={() => void execute()}>{unresolved > 0 ? `还有 ${unresolved} 首待确认` : '确认并执行迁移'}</button></>}</section>}
    {transferRun && <section className="panel transfer-result"><div className="panel-title"><div><span className="eyebrow">COPY SESSION</span><h3>{transferRun.status}</h3></div><span>{transferRun.processed_count} / {transferRun.source_count}</span></div><p>实时进度 {Math.round(transferRun.progress * 100)}% · 会话 {transferRun.id.slice(0, 8)}</p>{['QUEUED', 'RUNNING', 'CANCELLING'].includes(transferRun.status) && <button className="danger-button" disabled={transferRun.status === 'CANCELLING'} onClick={() => void cancelTransfer()}>中断复制</button>}{['FAILED', 'CANCELLED'].includes(transferRun.status) && <button className="secondary" onClick={() => void resumeTransfer()}>从已完成歌曲继续</button>}{transferRun.error && <p className="error-box">{transferRun.error}</p>}</section>}
    {result && <section className="panel transfer-result"><span className="eyebrow">EXECUTION RESULT</span><h3>{result.status} · {result.is_mock ? 'MOCK' : preview?.destination_platform.toUpperCase()}</h3><div className="result-stats"><div><strong>{result.written_count}</strong><span>成功</span></div><div><strong>{result.failed_count}</strong><span>失败</span></div><div><strong>{result.skipped_count}</strong><span>跳过</span></div><div><strong>{result.unmatched_count}</strong><span>未匹配</span></div></div>{result.playlist_url && <a href={result.playlist_url} target="_blank" rel="noreferrer">打开新播放列表</a>}<div className="report-links">{result.report_csv_url && <a href={result.report_csv_url}>下载 CSV 报告</a>}{result.report_json_url && <a href={result.report_json_url}>下载 JSON 报告</a>}</div></section>}
  </div>
  return <div className="page-width"><PageIntro eyebrow="PLAYLIST TRANSFER AGENT" title="Spotify → YouTube，先匹配再确认" copy="源歌单只读；确定性程序搜索和评分每首候选。未确认歧义或版本回退前不会创建目标歌单，目标默认且当前仅支持私有。" badge="/transfer · MVP"/><section className="transfer-connections"><ConnectionCard name="Spotify source" status={spotify}/><ConnectionCard name="YouTube destination" status={youtube}/></section><section className="panel transfer-source"><div className="panel-title"><div><span className="eyebrow">SOURCE PLAYLIST</span><h3>选择一个只读来源</h3></div><span>{sourceLabel}</span></div><div className="transfer-source-actions">{spotify?.connected ? <button className="spotify-button" onClick={() => void loadSpotifyPlaylists()}>读取我的 Spotify 歌单</button> : <a className="button-link spotify-button" href="/api/spotify/authorize">通过官方 OAuth 连接 Spotify</a>}<label className="secondary upload-button">上传导出的歌单<input type="file" accept=".txt,.csv,.tsv,.json,.m3u,.m3u8" onChange={(event) => void loadFile(event.target.files?.[0])}/></label>{currentAnalysis && !currentAnalysis.report.is_demo && <button className="secondary" onClick={useCurrent}>使用当前已分析歌单</button>}</div>{spotifyPlaylists.length > 0 && <div className="transfer-playlist-picker">{spotifyPlaylists.map((playlist) => <button onClick={() => void chooseSpotify(playlist)} key={playlist.id}><b>{playlist.name}</b><span>{playlist.track_count} 首 · {playlist.owner_name}{playlist.collaborative ? ' · 协作' : ''}</span></button>)}</div>}<div className="transfer-source-summary"><strong>{sourceName}</strong><span>{tracks.length} 首 · 源歌单不会被修改或删除</span></div><label className="confirm-check"><input type="checkbox" checked={allowAlternate} onChange={(event) => setAllowAlternate(event.target.checked)}/><span><b>原版不可用时允许其他版本</b><small>默认关闭。开启后 Live/Remix/Acoustic 等仍会标成歧义并要求逐首确认。</small></span></label><div className="transfer-preview-actions"><button className="primary" disabled={tracks.length === 0 || busy} onClick={() => void prepare(false)}>使用真实 YouTube Provider 生成预览</button><button className="demo-flow-button" disabled={tracks.length === 0 || busy} onClick={() => void prepare(true)}>使用明确 Mock Connector 验证流程</button></div></section>{error && <p className="error-box">{error}</p>}{preview && <section className="panel transfer-preview"><div className="panel-title"><div><span className="eyebrow">TRANSFER PREVIEW</span><h3>{preview.provider} · {preview.status}</h3></div><span>{preview.is_mock ? 'MOCK CONNECTOR' : 'REAL PROVIDER'}</span></div><p>{preview.message}</p><div className="result-stats"><div><strong>{preview.source_count}</strong><span>源歌曲</span></div><div><strong>{preview.high_confidence_count}</strong><span>高置信</span></div><div><strong>{preview.ambiguous_count}</strong><span>歧义</span></div><div><strong>{preview.unmatched_count}</strong><span>未匹配</span></div></div><div className="transfer-match-list">{preview.matches.map((match) => <div className={`transfer-match ${match.status.toLowerCase()}`} key={match.source_track.source_track_id}><div><b>{match.source_track.title}</b><small>{match.source_track.artists.join(', ')} · {match.source_track.version_type}</small></div><span>{match.status} · {Math.round(match.score * 100)}%</span><p>{match.reason}</p>{match.requires_confirmation && <select value={selections[match.source_track.source_track_id] ?? ''} onChange={(event) => setSelections({ ...selections, [match.source_track.source_track_id]: event.target.value })}><option value="">请选择候选或跳过</option>{match.candidates.map((candidate) => <option value={candidate.target_id} key={candidate.target_id}>{candidate.title} — {candidate.artists.join(', ')} · {candidate.version_type} · {Math.round(candidate.score * 100)}%</option>)}<option value="SKIP">跳过这首</option></select>}<details><summary>查看最多 5 个候选及评分依据</summary>{match.candidates.map((candidate) => <div className="transfer-candidate" key={candidate.target_id}><b>{candidate.title}</b><span>{candidate.reason}</span></div>)}</details></div>)}</div>{preview.matches.length > 0 && <><label className="confirm-check"><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)}/><span><b>我已检查匹配，确认创建新的私有 YouTube 播放列表</b><small>不会修改 Spotify 源歌单；歧义或版本回退必须先改选或跳过。</small></span></label><button className="primary big full" disabled={!confirmed || unresolved > 0 || busy} onClick={() => void execute()}>{unresolved > 0 ? `还有 ${unresolved} 首待确认` : '确认并执行迁移'}</button></>}</section>}{result && <section className="panel transfer-result"><span className="eyebrow">EXECUTION RESULT</span><h3>{result.status} · {result.is_mock ? 'MOCK CONNECTOR' : 'REAL YOUTUBE'}</h3><div className="result-stats"><div><strong>{result.written_count}</strong><span>写入成功</span></div><div><strong>{result.failed_count}</strong><span>写入失败</span></div><div><strong>{result.skipped_count}</strong><span>跳过</span></div><div><strong>{result.unmatched_count}</strong><span>未匹配</span></div></div><p>进度 {Math.round(result.progress * 100)}% · 源歌单修改：{result.source_was_modified ? '是（异常）' : '否'}</p>{result.playlist_url && (result.is_mock ? <code>{result.playlist_url}</code> : <a href={result.playlist_url} target="_blank" rel="noreferrer">打开新 YouTube 播放列表</a>)}<div className="report-links">{result.report_csv_url && <a href={result.report_csv_url}>下载 CSV 报告</a>}{result.report_json_url && <a href={result.report_json_url}>下载 JSON 报告</a>}</div></section>}</div>
}

function ConnectionCard({ name, status }: { name: string; status: SpotifyConnectionStatus | YouTubeConnectionStatus | null }) { return <div className={`connection-card ${status?.connected ? 'connected' : ''}`}><span>{status?.connected ? '已连接' : status?.configured ? '等待授权' : 'CONFIG_REQUIRED · 未配置'}</span><strong>{name}</strong><small>{status?.message ?? '正在检查状态'}</small></div> }

function AgentPage({ currentAnalysis, comparisonBinding }: { currentAnalysis: PersonalAnalysis | null; comparisonBinding: { analysis_a_id: string; analysis_b_id: string } | null }) {
  const [task, setTask] = useState<AgentTask | null>(null)
  const [scenario, setScenario] = useState<AgentTask['scenario']>('personal_exploration')
  const [goal, setGoal] = useState('分析当前真实歌单，并根据候选结果逐步构建可解释探索路线')
  const [useDemo, setUseDemo] = useState(false)
  const [error, setError] = useState('')
  useEffect(() => {
    if (!task || ['COMPLETED', 'FAILED', 'CANCELLED', 'BLOCKED_EXTERNAL_AUTH'].includes(task.status)) return
    const source = new EventSource(`/api/tasks/${task.id}/events`)
    source.addEventListener('task', (event) => setTask(JSON.parse((event as MessageEvent).data) as AgentTask))
    source.onerror = () => source.close()
    return () => source.close()
  }, [task?.id, task?.status])
  const start = async () => {
    setError('')
    try {
      setTask(await api.createTask(scenario, goal, {
        analysis_id: scenario === 'personal_exploration' && !useDemo ? currentAnalysis?.analysis_id : undefined,
        analysis_a_id: scenario === 'friend_bridge' && !useDemo ? comparisonBinding?.analysis_a_id : undefined,
        analysis_b_id: scenario === 'friend_bridge' && !useDemo ? comparisonBinding?.analysis_b_id : undefined,
        use_demo: useDemo,
      }))
    } catch (reason) { setError(reason instanceof Error ? reason.message : '无法创建任务') }
  }
  const cancel = async () => { if (task) setTask(await api.cancelTask(task.id)) }
  const resume = async () => { if (task) { setError(''); try { setTask(await api.resumeTask(task.id)) } catch (reason) { setError(reason instanceof Error ? reason.message : '无法恢复任务') } } }
  const plan = task ? parseAgentPlan(task.plan_json) : null
  const decisions = task ? parseJsonArray<AgentDecision>(task.decisions_json) : []
  const toolCalls = task ? parseJsonArray<{ call_id: string; tool: string; step_id: string; attempt: number }>(task.tool_calls_json) : []
  const toolResults = task ? parseJsonArray<{ call_id: string; summary: string; success: boolean; output?: { summary?: string; recommendation_message?: string } }>(task.tool_results_json) : []
  const explanationCall = [...toolCalls].reverse().find(call => call.tool === 'prepare_explanation')
  const explanation = explanationCall ? toolResults.find(result => result.call_id === explanationCall.call_id && result.success)?.output : undefined
  const active = task && ['PLANNING', 'RUNNING'].includes(task.status)
  const bindingReady = useDemo || (scenario === 'personal_exploration' ? Boolean(currentAnalysis && !currentAnalysis.report.is_demo) : Boolean(comparisonBinding))
  return <div className="page-width"><PageIntro eyebrow="LLM-ASSISTED RUST AGENT LOOP" title="从用户请求到工具结果，看清 Agent 的每一步" copy="这里展示 AI 如何理解任务、选择工具并生成结果。先导入并分析歌单，再选择下面的任务开始。未配置 AI 模型时，由确定性程序规划并执行，页面会明确标注；音乐分析与工具执行始终由 Rust 完成。" badge="LIVE SSE · CHECKPOINT"/><ol className="agent-workflow" aria-label="Agent 展示流程">{['User Request · 用户请求', 'Agent Decision · 结构化决策', 'Tool Call · Rust 工具调用', 'Tool Result · 执行结果', 'Final Explanation · 最终说明'].map((label, index) => <li key={label}><span>{index + 1}</span>{label}{index < 4 && <b aria-hidden="true">↓</b>}</li>)}</ol><div className="agent-layout"><section className="panel agent-control"><h3>选择音乐场景</h3><label className={scenario === 'personal_exploration' ? 'scenario active' : 'scenario'}><input type="radio" checked={scenario === 'personal_exploration'} onChange={() => setScenario('personal_exploration')}/><span>01</span><div><strong>个人 Genre 探索</strong><p>根据已导入歌单，寻找新的音乐方向</p></div></label><label className={scenario === 'friend_bridge' ? 'scenario active' : 'scenario'}><input type="radio" checked={scenario === 'friend_bridge'} onChange={() => setScenario('friend_bridge')}/><span>02</span><div><strong>两份歌单比较</strong><p>先在 Compare 比较，再探索双方共同喜欢的音乐</p></div></label><label className="field"><span>User Goal</span><textarea rows={3} value={goal} onChange={(event) => setGoal(event.target.value)}/></label><div className="key-status"><div><strong>{scenario === 'personal_exploration' ? currentAnalysis ? `${currentAnalysis.playlist.name} · ${currentAnalysis.report.track_count} 首` : '尚未分析真实歌单' : comparisonBinding ? '已绑定最近一次真实比较' : '尚未完成真实 /compare'}</strong><p>将使用你刚才确认的歌单分析结果。请先导入并完成分析。</p></div></div><label className="confirm-check"><input type="checkbox" checked={useDemo} onChange={(event) => setUseDemo(event.target.checked)}/><span><b>明确使用 Demo</b><small>仅勾选后 Agent 才允许读取 demo payload。</small></span></label><button className="primary big full" disabled={Boolean(active) || !bindingReady} onClick={start}>启动 Agent 工具循环</button>{!bindingReady && <p className="privacy-note">请先完成对应的真实分析或真实比较。</p>}{error && <p className="error-box">{error}</p>}</section><section className="panel agent-terminal"><div className="terminal-head"><span><i/> agent.runtime</span>{task && <code>{task.id.slice(0, 8)}</code>}</div>{!task ? <div className="terminal-empty"><Logo/><p>绑定数据后启动，可核验的决策与工具轨迹将在这里出现。</p></div> : <><div className="progress-orbit"><div className="progress-number">{Math.round(task.progress * 100)}<small>%</small></div><svg viewBox="0 0 120 120"><circle cx="60" cy="60" r="52"/><circle className="progress-value" cx="60" cy="60" r="52" style={{ strokeDashoffset: 327 - 327 * task.progress }}/></svg></div><div className="agent-message"><span>STEP {String(task.current_step).padStart(2, '0')}</span><h3>{task.message}</h3><p><b>User Goal：</b>{task.goal}</p><p><b>Normalized Intent：</b>{task.normalized_intent_json}</p><p><b>Decision mode：</b>{task.decision_mode === 'LLM' ? 'LLM' : 'DETERMINISTIC_FALLBACK · 确定性执行'} · <b>Data state：</b>{task.data_state}</p></div>{plan && <div className="agent-plan">{plan.steps.map((step) => <div className={`agent-plan-step ${step.status.toLowerCase()}`} key={step.step_id}><i>{step.status === 'COMPLETED' ? '✓' : step.status === 'RUNNING' ? '→' : step.status === 'FAILED' ? '!' : '○'}</i><span><b>{step.label}</b><small>{step.tool}{step.attempts > 0 ? ` · ${step.attempts} 次` : ''}</small></span></div>)}</div>}<div className="agent-trace"><p>以下分组展示本次请求、最近 8 条决策/调用/结果及最终状态；工具结果会反馈到下一轮决策。</p><span className="eyebrow">01 · USER REQUEST / 用户请求</span><p><b>GOAL</b><small>{task.goal}</small></p><span className="eyebrow">02 · AGENT DECISION / 结构化决策</span>{decisions.slice(-8).map((decision, index) => <p key={`${decision.next_tool}-${index}`}><b>{decision.action.toUpperCase()}</b> · {decision.next_tool ?? 'finish'}<small>{decision.reason}</small></p>)}<span className="eyebrow">03 · TOOL CALL / 工具调用</span>{toolCalls.slice(-8).map((call, index) => <p key={`${call.step_id}-${call.attempt}-${index}`}><b>{call.tool}</b><small>{call.step_id} · attempt {call.attempt}</small></p>)}<span className="eyebrow">04 · TOOL RESULT / 执行结果</span>{toolResults.slice(-8).map((result, index) => <p key={`${result.summary}-${index}`}><b>{result.success ? 'RESULT' : 'ERROR'}</b><small>{result.summary}</small></p>)}<span className="eyebrow">05 · FINAL EXPLANATION / 最终说明</span><p><b>{task.status === 'COMPLETED' ? 'FINAL' : '尚未完成 · 当前状态'}</b><small>{task.message}</small>{task.status === 'COMPLETED' && explanation && <><small><b>Rust 工具分析摘要：</b>{explanation.summary}</small>{explanation.recommendation_message && <small>{explanation.recommendation_message}</small>}</>}</p></div><div className="usage-grid"><span><small>状态</small><strong>{task.status}</strong></span><span><small>重试</small><strong>{task.retries}</strong></span><span><small>工具调用</small><strong>{safeArrayLength(task.tool_calls_json)}</strong></span><span><small>Token Usage</small><strong>{task.input_tokens + task.output_tokens}</strong></span><span><small>Estimated Cost</small><strong>${task.estimated_cost_usd.toFixed(4)}</strong></span></div>{active && <button className="danger-button" onClick={cancel}>中断任务</button>}{['FAILED', 'CANCELLED'].includes(task.status) && <button className="secondary" onClick={resume}>从检查点恢复</button>}{task.error && <p className="error-box">{task.error}</p>}{task.status === 'COMPLETED' && <p className="success-box">本次结果（数据来源见 Data state）、结构化决策、工具调用和检查点均已持久化，可在“历史”中核验。</p>}</>}</section></div></div>
}

function parseAgentPlan(value: string): AgentPlan | null { try { return JSON.parse(value) as AgentPlan } catch { return null } }
function safeArrayLength(value: string): number { try { const parsed = JSON.parse(value) as unknown; return Array.isArray(parsed) ? parsed.length : 0 } catch { return 0 } }
function parseJsonArray<T>(value: string): T[] { try { const parsed = JSON.parse(value) as unknown; return Array.isArray(parsed) ? parsed as T[] : [] } catch { return [] } }

function HistoryPage() {
  const [tasks, setTasks] = useState<AgentTask[]>([])
  const [selected, setSelected] = useState<AgentTask | null>(null)
  const [error, setError] = useState('')
  const load = () => api.tasks().then(setTasks).catch((reason: unknown) => setError(reason instanceof Error ? reason.message : '读取失败'))
  useEffect(() => { load() }, [])
  const totals = useMemo(() => tasks.reduce((sum, task) => ({ tokens: sum.tokens + task.input_tokens + task.output_tokens, cost: sum.cost + task.estimated_cost_usd }), { tokens: 0, cost: 0 }), [tasks])
  return <div className="page-width"><PageIntro eyebrow="PERSISTED HISTORY" title="任务不会随页面消失" copy="目标、计划、工具调用、结果、重试与检查点保存在 SQLite。服务重启会明确中断运行中任务，并允许从已完成步骤继续。" badge="SQLITE"/><div className="history-summary"><span><strong>{tasks.length}</strong>历史任务</span><span><strong>{totals.tokens}</strong>累计 Tokens</span><span><strong>${totals.cost.toFixed(4)}</strong>估算费用</span><button className="secondary" onClick={load}>刷新</button></div>{error && <p className="error-box">{error}</p>}<section className="panel history-table"><div className="table-row table-head"><span>场景 / 目标</span><span>状态</span><span>步骤</span><span>工具调用</span><span>重试</span><span/></div>{tasks.map((task) => <button className="table-row" onClick={() => setSelected(task)} key={task.id}><span><strong>{task.scenario === 'personal_exploration' ? '个人 Genre 探索' : '好友歌单比较'}</strong><small>{task.goal}</small></span><span><i className={`task-status ${task.status.toLowerCase()}`}/>{task.status}</span><span>{task.current_step} / {task.max_steps}</span><span>{safeArrayLength(task.tool_calls_json)}</span><span>{task.retries}</span><span>查看 →</span></button>)}{tasks.length === 0 && <div className="empty-row">还没有历史任务。到“Agent 运行”启动一个场景。</div>}</section>{selected && <section className="panel history-detail"><button className="icon-button" onClick={() => setSelected(null)}>×</button><span className="eyebrow">SAVED AGENT RUN</span><h3>{selected.goal}</h3><p>{selected.message}</p>{selected.error && <p className="error-box">{selected.error}</p>}<pre>{selected.result_json ? JSON.stringify(JSON.parse(selected.result_json), null, 2).slice(0, 6000) : selected.plan_json}</pre></section>}</div>
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
