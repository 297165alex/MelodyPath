import { useEffect, useState } from 'react'

export default function DemoNotice() {
  const [date, setDate] = useState<string>()
  useEffect(() => {
    const controller = new AbortController()
    fetch('/api/public-config', { signal: controller.signal })
      .then(async response => {
        if (!response.ok) return
        const config = await response.json() as { public_demo_expires_at?: unknown }
        if (typeof config.public_demo_expires_at === 'string' && /^\d{4}-\d{2}-\d{2}$/.test(config.public_demo_expires_at)) {
          setDate(config.public_demo_expires_at)
        }
      })
      .catch(() => { /* Optional public notice; never infer deployment dates on failure. */ })
    return () => controller.abort()
  }, [])
  if (!date) return null
  return <aside aria-label="课程 Demo 说明" className="public-demo-notice">
    <strong>MelodyPath · Temporary course demo / 课程临时演示</strong>
    <p>本公网版本预计开放至 <time dateTime={date}>{date}</time>，到期后服务可能关闭，源码继续保留在 GitHub / Git.Tsinghua。</p>
    <p>开放期间普通用户无需安装 Rust、Node 或配置 Developer Credentials；Spotify / YouTube 只需在官方 OAuth 页面授权自己的账号。</p>
    <p>后续可 clone 源码自行部署。Self-host 如需 Spotify / YouTube / Last.fm 的真实平台能力，必须配置自己的 Developer Credentials；clone 不包含、也不能共享本 Demo 部署者的 Secret。</p>
  </aside>
}
