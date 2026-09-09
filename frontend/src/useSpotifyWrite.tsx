import { useEffect, useRef, useState } from 'react'
import { api } from './api'

// Keep the draft in React memory while official OAuth runs in a separate window.
export function useSpotifyWrite() {
  const [message, setMessage] = useState('')
  const [waiting, setWaiting] = useState(false)
  const pending = useRef<(() => Promise<void>) | null>(null)
  const alive = useRef(true)
  const authWindow = useRef<Window | null>(null)
  const callbackState = useRef('')
  const timer = useRef<ReturnType<typeof setInterval> | undefined>(undefined)
  useEffect(() => { alive.current = true; return () => { alive.current = false; pending.current = null; clearInterval(timer.current) } }, [])
  useEffect(() => {
    const receive = (event: MessageEvent) => {
      if (event.origin === window.location.origin && event.source === authWindow.current && event.data?.type === 'melody-spotify-oauth') callbackState.current = event.data.status
    }
    window.addEventListener('message', receive)
    return () => window.removeEventListener('message', receive)
  }, [])
  const check = async (resume: () => Promise<void>) => {
    const status = await api.spotifyMe()
    if (status.connected && status.write_authorized) { setMessage(''); return true }
    pending.current = resume
    setMessage(!status.configured ? 'Spotify OAuth 未配置，请由部署者完成配置后重试。' : status.connected ? '需要 Spotify 写入权限' : 'Spotify 登录已过期或尚未连接，请重新授权。')
    return false
  }
  const failed = (reason: unknown, resume: () => Promise<void>) => {
    if (reason instanceof Error && /AUTH|401|403|expired|授权/i.test(reason.message)) {
      pending.current = resume
      setMessage('Spotify 授权已失效或缺少写权限，请重新授权。原任务已保留。')
    }
  }
  const authorize = () => {
    const popup = window.open('/api/spotify/authorize?write=true', 'melody-spotify-write', 'width=620,height=760')
    if (!popup) { setMessage('授权窗口被阻止，请允许弹窗后重试。'); return }
    authWindow.current = popup; callbackState.current = ''
    setWaiting(true)
    clearInterval(timer.current)
    const deadline = Date.now() + 300_000
    let checking = false
    timer.current = setInterval(async () => {
      if (checking) return
      checking = true
      try {
        let callback = callbackState.current
        try {
          const url = new URL(popup.location.href)
          if (url.origin === window.location.origin && url.searchParams.get('provider') === 'spotify') callback = url.searchParams.get('oauth') ?? ''
        } catch { /* Official OAuth pages are cross-origin; never inspect their contents. */ }
        if (popup.closed || Date.now() > deadline || callback === 'error') {
          clearInterval(timer.current); setWaiting(false); setMessage('授权未完成或未授予写权限。原任务已保留，请重试。'); return
        }
        if (callback !== 'connected') return
        const status = await api.spotifyMe()
        if (!alive.current) return
        if (status.connected && status.write_authorized) {
          clearInterval(timer.current); popup.close(); setWaiting(false); setMessage('')
          const resume = pending.current; pending.current = null
          await resume?.()
        } else {
          clearInterval(timer.current); setWaiting(false); setMessage('授权未完成或未授予写权限。原任务已保留，请重试。')
        }
      } catch { clearInterval(timer.current); setWaiting(false); setMessage('无法确认授权状态，请重试。') }
      finally { checking = false }
    }, 1500)
  }
  return { check, failed, waiting, prompt: message && <div className="policy-box" role="status"><p>{message}</p><button className="secondary" disabled={waiting} onClick={authorize}>{waiting ? '等待 Spotify 官方授权…' : '重新授权 Spotify'}</button></div> }
}
