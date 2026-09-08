import { useMemo, useState } from 'react'
import { api } from './api'
import type { ExportPreview, ExportResult, Track, WriterStatus } from './types'

interface Props {
  initialPlatform: string
  tracks: Track[]
  sourceLabel: string
  statuses: WriterStatus[]
  onClose: () => void
}

const platformNames: Record<string, string> = {
  spotify: 'Spotify', apple_music: 'Apple Music', youtube: 'YouTube', file: '歌曲清单', demo: 'Demo 虚拟平台',
}

export default function ExportModal({ initialPlatform, tracks, sourceLabel, statuses, onClose }: Props) {
  const [platform, setPlatform] = useState(initialPlatform)
  const [selected, setSelected] = useState(() => new Set(tracks.map((track) => track.id)))
  const [playlistName, setPlaylistName] = useState(initialPlatform === 'spotify' ? 'MelodyPath Generated Playlist' : `${sourceLabel} · MelodyPath`)
  const [format, setFormat] = useState('csv')
  const [preview, setPreview] = useState<ExportPreview | null>(null)
  const [result, setResult] = useState<ExportResult | null>(null)
  const [confirmed, setConfirmed] = useState(false)
  const [selections, setSelections] = useState<Record<string, string>>({})
  const [busy, setBusy] = useState(false)
  const [error, setError] = useState('')
  const status = statuses.find((item) => item.platform === platform)
  const chosenTracks = useMemo(() => tracks.filter((track) => selected.has(track.id)), [selected, tracks])

  const toggle = (id: string) => {
    setSelected((current) => {
      const next = new Set(current)
      if (next.has(id)) next.delete(id); else next.add(id)
      return next
    })
  }

  const makePreview = async () => {
    setBusy(true); setError('')
    try { setPreview(await api.previewExport(platform, playlistName, chosenTracks, format)) }
    catch (reason) { setError(reason instanceof Error ? reason.message : '无法生成预览') }
    finally { setBusy(false) }
  }

  const execute = async () => {
    if (!preview || !confirmed) return
    setBusy(true); setError('')
    try { setResult(await api.executeExport(preview.preview_id, selections)) }
    catch (reason) { setError(reason instanceof Error ? reason.message : '写入失败') }
    finally { setBusy(false) }
  }

  return <div className="modal-backdrop" role="presentation" onMouseDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="export-modal" role="dialog" aria-modal="true" aria-label="保存到音乐平台">
      <header className="modal-header">
        <div><span className="eyebrow">PLAYLIST WRITER</span><h2>保存探索成果</h2></div>
        <button className="icon-button" onClick={onClose} aria-label="关闭">×</button>
      </header>

      {!preview && !result && <>
        <div className="safety-note"><strong>授权边界</strong><span>只创建新歌单。预览与确认之前不会修改任何账号；授权只通过平台 OAuth。</span></div>
        <div className="platform-pills">
          {['spotify', 'apple_music', 'youtube', 'file', 'demo'].map((key) => <button key={key} className={platform === key ? 'active' : ''} onClick={() => { setPlatform(key); setError('') }}>{platformNames[key]}</button>)}
        </div>
        {status && <div className={`status-strip ${status.availability}`}>
          <span className={`status-dot ${status.authorized ? 'online' : ''}`} />
          <span>{status.message}</span>
          {['spotify', 'youtube'].includes(platform) && status.availability === 'available' && !status.authorized && <a className="mini-button" href={`/api/${platform}/authorize?write=true`}>单独授权歌单写入</a>}
        </div>}
        {platform === 'youtube' && <p className="policy-box">Google 没有仅限歌单写入的 scope；仅需读取时无需升级。写入授权文案也涵盖视频、评论等操作，但本应用只允许用户确认后新建私有歌单及添加视频。</p>}
        <label className="field"><span>新歌单名称</span><input value={playlistName} maxLength={100} onChange={(event) => setPlaylistName(event.target.value)} /></label>
        {platform === 'file' && <label className="field"><span>导出格式</span><select value={format} onChange={(event) => setFormat(event.target.value)}><option value="csv">CSV（推荐）</option><option value="json">JSON</option><option value="m3u">M3U8</option></select></label>}
        <div className="selection-heading"><div><strong>歌曲预览</strong><span>{chosenTracks.length} / {tracks.length} 首已选择</span></div><button className="text-button" onClick={() => setSelected(selected.size === tracks.length ? new Set() : new Set(tracks.map((t) => t.id)))}>{selected.size === tracks.length ? '取消全选' : '选择全部'}</button></div>
        <div className="track-picker">
          {tracks.map((track, index) => <label className="pick-row" key={track.id}>
            <input type="checkbox" checked={selected.has(track.id)} onChange={() => toggle(track.id)} />
            <span className="track-index">{String(index + 1).padStart(2, '0')}</span>
            <span className="track-main"><strong>{track.title}</strong><small>{track.artists.join(', ')} · {track.genres.slice(0, 2).join(' / ')}</small></span>
            <span className="version-chip">{track.version_type}</span>
          </label>)}
        </div>
        {error && <p className="error-box">{error}</p>}
        <footer className="modal-actions"><button className="secondary" onClick={onClose}>取消</button><button className="primary" disabled={busy || !playlistName.trim() || chosenTracks.length === 0 || status?.availability !== 'available'} onClick={makePreview}>{busy ? '正在匹配…' : '下一步：匹配并预览'}</button></footer>
      </>}

      {preview && !result && <>
        {preview.is_demo && <div className="demo-warning">DEMO · {preview.disclosure}</div>}
        <div className="result-stats compact"><Stat value={preview.requested_count} label="请求"/><Stat value={preview.auto_matched_count} label="自动匹配"/><Stat value={preview.needs_confirmation_count} label="待确认"/><Stat value={preview.unmatched_count} label="未匹配"/></div>
        <div className="match-list">
          {preview.matches.map((match) => <div className={`match-row ${match.status}`} key={match.source_track.id}>
            <div className="match-title"><strong>{match.source_track.title}</strong><span>{match.source_track.artists.join(', ')}</span></div>
            <div className="confidence"><b>{Math.round(match.confidence * 100)}%</b><span>{match.match_reason}</span></div>
            <span className="match-status">{match.status === 'matched' ? '已匹配' : match.status === 'needs_confirmation' ? '请选择版本' : '无法匹配'} · Source: {match.target_platform === 'spotify' ? 'Spotify' : platformNames[match.target_platform] ?? match.target_platform}</span>
            {match.status === 'needs_confirmation' && <select value={selections[match.source_track.id] ?? ''} onChange={(event) => setSelections({ ...selections, [match.source_track.id]: event.target.value })}>
              <option value="">暂不写入，保留待确认</option>
              {match.candidates.filter((candidate) => candidate.available_in_market).map((candidate) => <option value={candidate.target_track_id} key={candidate.target_track_id}>{candidate.title} — {candidate.artists.join(', ')} · {Math.round(candidate.confidence * 100)}% · {candidate.version_type}</option>)}
            </select>}
          </div>)}
        </div>
        <label className="confirm-check"><input type="checkbox" checked={confirmed} onChange={(event) => setConfirmed(event.target.checked)} /><span>我已检查以上歌曲与版本，并明确同意创建<strong>新的</strong>“{preview.playlist_name}”歌单。未确认或无法匹配的歌曲不会写入。</span></label>
        {error && <p className="error-box">{error}</p>}
        <footer className="modal-actions"><button className="secondary" onClick={() => { setPreview(null); setConfirmed(false) }}>返回选择</button><button className="primary" disabled={!confirmed || busy} onClick={execute}>{busy ? '正在创建…' : platform === 'file' ? '确认并生成文件' : '确认并创建新歌单'}</button></footer>
      </>}

      {result && <>
        <div className={`completion-mark ${result.is_demo ? 'demo' : ''}`}>{result.is_demo ? 'DEMO' : '✓'}</div>
        <div className="completion-copy"><span className="eyebrow">EXPORT REPORT</span><h2>{result.is_demo ? '虚拟写入流程已完成' : '歌单处理完成'}</h2><p>{result.disclosure ?? `“${result.playlist_name}”已按确认结果处理。`}</p></div>
        <div className="result-stats"><Stat value={result.requested_count} label="请求"/><Stat value={result.added_count} label="成功"/><Stat value={result.failed_count} label="失败"/><Stat value={result.needs_confirmation_count} label="仍待确认"/></div>
        {result.successful_tracks.length > 0 && <ResultList title="成功" items={result.successful_tracks.map((item) => `${item.source_track.title} — ${item.source_track.artists.join(', ')}`)} />}
        {result.failed_tracks.length > 0 && <ResultList title="失败" items={result.failed_tracks.map((item) => `${item.track.title}：${item.reason}`)} />}
        {result.ambiguous_tracks.length > 0 && <ResultList title="待确认" items={result.ambiguous_tracks.map((item) => `${item.source_track.title}：${item.match_reason}`)} />}
        <footer className="modal-actions"><button className="secondary" onClick={onClose}>关闭</button>{result.playlist_url && (result.platform === 'file' ? <a className="primary button-link" href={result.playlist_url}>下载真实文件</a> : result.is_demo ? <span className="demo-url">{result.playlist_url}</span> : <a className="primary button-link" href={result.playlist_url} target="_blank" rel="noreferrer">打开真实歌单 ↗</a>)}</footer>
      </>}
    </section>
  </div>
}

function Stat({ value, label }: { value: number; label: string }) { return <div><strong>{value}</strong><span>{label}</span></div> }
function ResultList({ title, items }: { title: string; items: string[] }) { return <details className="result-list" open><summary>{title} · {items.length}</summary>{items.map((item) => <p key={item}>{item}</p>)}</details> }
