import { test, expect, type Page } from '@playwright/test'

// Isolated synthetic fixtures. These tests do not claim real OAuth or platform acceptance.
async function setup(page: Page, youtubeConnected = false, youtubeError = false) {
  await page.route('**/api/**', async route => {
    const path = new URL(route.request().url()).pathname
    if (path === '/api/youtube/me' && youtubeError) return route.fulfill({ status: 503, json: { error: 'Synthetic unavailable' } })
    const connected = path.includes('spotify') || youtubeConnected
    const status = { configured: true, connected, write_authorized: false, display_name: 'Synthetic', message: connected ? 'Synthetic identity verified' : 'Session expired', policy_notice: 'Synthetic fixture only' }
    const data: Record<string, unknown> = {
      '/api/demo': { disclosure: 'Explicit synthetic fixture' },
      '/api/writers/status': [],
      '/api/platforms/capabilities': [{ platform: 'spotify', display_name: 'Spotify', configured: true, auth_supported: true, restrictions: [], status_label: 'Synthetic' }],
      '/api/spotify/me': status,
      '/api/youtube/me': status,
      '/api/spotify/playlists': Array.from({ length: 100 }, (_, i) => ({ id: `synthetic-${i}`, name: `Synthetic playlist ${i}`, owner_name: 'Synthetic', track_count: 2, collaborative: false })),
    }
    await route.fulfill({ json: data[path] ?? [] })
  })
}

test('callback query cannot establish a connection', async ({ page }) => {
  await setup(page)
  await page.goto('/?oauth=connected&provider=youtube')
  await expect(page.locator('.oauth-notice')).toContainText('YouTube Error')
  await expect(page.getByText('YouTube Connected', { exact: false })).toHaveCount(0)
})

test('backend-verified callback shows connected', async ({ page }) => {
  await setup(page, true)
  await page.goto('/?oauth=connected&provider=youtube')
  await expect(page.locator('.oauth-notice')).toContainText('YouTube Connected')
})

test('session endpoint failure remains visible', async ({ page }) => {
  await setup(page, false, true)
  await page.goto('/?oauth=connected&provider=youtube')
  await expect(page.locator('.oauth-notice')).toContainText('无法核验后端会话')
})

test('callback errors are explicit', async ({ page }) => {
  await setup(page)
  await page.goto('/?oauth=error&provider=youtube&reason=state_mismatch')
  await expect(page.locator('.oauth-notice')).toContainText('安全校验失败')
})

for (const viewport of [{ width: 1280, height: 720 }, { width: 390, height: 600 }]) {
  test(`Spotify picker footer remains in viewport ${viewport.width}`, async ({ page }) => {
    await page.setViewportSize(viewport)
    await setup(page)
    await page.goto('/')
    await page.getByRole('button', { name: '选择我的歌单', exact: true }).click()
    const dialog = page.getByRole('dialog', { name: '选择 Spotify 歌单' })
    const confirm = dialog.getByRole('button', { name: '确认选择 / Continue', exact: true })
    await expect(confirm).toBeDisabled()
    await expect(confirm).toBeInViewport()
    await dialog.getByRole('checkbox').first().check()
    await expect(confirm).toBeEnabled()
    await expect(dialog.locator('.picker-footer')).toContainText('已选择 1 个歌单')
    const list = dialog.locator('.spotify-playlist-list')
    expect(await list.evaluate(e => e.scrollHeight > e.clientHeight)).toBe(true)
    await list.evaluate(e => { e.scrollTop = e.scrollHeight })
    await expect(confirm).toBeInViewport()
    await expect(dialog.getByRole('button', { name: '取消', exact: true })).toBeInViewport()
  })
}

test('public link transport failure has an actionable message', async ({ page }) => {
  await setup(page)
  await page.route('**/api/playlists/inspect-link', route => route.abort())
  await page.goto('/')
  await page.locator('#playlist-link').fill('https://open.spotify.com/playlist/1234567890123456789012')
  await page.getByRole('button', { name: '检查并导入预览' }).click()
  await expect(page.locator('.error-box').last()).toContainText('请确认服务已启动')
  await expect(page.getByText('Failed to fetch', { exact: true })).toHaveCount(0)
})

for (const platform of ['spotify', 'youtube']) {
  for (const authorized of [false, true]) {
    test(`${platform} public link ${authorized ? 'preview requires confirmation' : 'requires OAuth without tracks'}`, async ({ page }) => {
      await setup(page)
      const requests: string[] = []
      page.on('request', request => requests.push(new URL(request.url()).pathname))
      await page.route('**/api/playlists/inspect-link', route => route.fulfill({ json: {
        recognized: true, platform, platform_label: platform, playlist_id: 'synthetic', playlist_id_valid: true,
        capability: authorized ? 'TRACK_IMPORT_AVAILABLE' : 'AUTH_REQUIRED', can_analyze: false,
        playlist_name: 'Synthetic Public List', track_count: authorized ? 1 : 0,
        preview_tracks: authorized ? [{ id: 'synthetic', title: 'Synthetic Song', artists: ['Synthetic Artist'], platform, external_ids: {}, genres: [], version_type: 'ORIGINAL' }] : [],
        message: authorized ? 'Official API contract fixture' : 'AUTH_REQUIRED：OAuth session 已过期，请重新连接',
      } }))
      await page.goto('/')
      await page.locator('#playlist-link').fill('https://open.spotify.com/playlist/1234567890123456789012')
      await page.getByRole('button', { name: '检查并导入预览' }).click()
      if (authorized) {
        await expect(page.locator('.link-preview')).toContainText('Synthetic Song')
        await expect(page.getByRole('dialog')).toHaveCount(0)
        await page.getByRole('button', { name: '确认并进入 Copy Playlist 预览' }).click()
        await expect(page.getByRole('dialog', { name: '保存到音乐平台' })).toBeVisible()
      } else {
        await expect(page.locator('.link-result')).toContainText('连接账号后可用')
        await expect(page.locator('.link-preview')).toHaveCount(0)
        await expect(page.locator('.link-result a')).toHaveAttribute('href', `/api/${platform}/authorize`)
      }
      expect(requests.some(path => /analyze|execute/.test(path))).toBe(false)
      await expect(page.locator('.experimental-status')).toContainText('按平台显示实际能力')
    })
  }
}


test('YouTube token network stage is visible instead of generic OAuth failure', async ({ page }) => {
  await setup(page)
  await page.goto('/?oauth=error&provider=youtube&reason=youtube_token_timeout')
  await expect(page.locator('.oauth-notice')).toContainText('Google 授权码交换超时')
  await expect(page.locator('.oauth-notice')).toContainText('代理')
  await expect(page.getByText('YouTube Connected', { exact: false })).toHaveCount(0)
})

test('Apple configuration required never offers Google OAuth or fabricated tracks', async ({ page }) => {
  await setup(page)
  await page.route('**/api/playlists/inspect-link', route => route.fulfill({ json: {
    recognized: true, platform: 'apple_music', platform_label: 'Apple Music', playlist_id: 'pl.synthetic', playlist_id_valid: true,
    capability: 'CONFIG_REQUIRED', preview_tracks: [], import_rows: [], can_analyze: false,
    message: 'WAITING_FOR_APPLE_DEVELOPER_CREDENTIALS', next_step: '文件或文本仍可用',
  } }))
  await page.goto('/')
  await page.locator('#playlist-link').fill('https://music.apple.com/us/playlist/pl.synthetic')
  await page.getByRole('button', { name: '检查并导入预览' }).click()
  await expect(page.locator('.link-result')).toContainText('当前演示环境尚未配置 Apple Music Developer Token')
  await expect(page.locator('.link-result').getByRole('button', { name: '导入文件或文本', exact: true })).toBeVisible()
  await expect(page.locator('.link-result a[href*="authorize"]')).toHaveCount(0)
  await expect(page.locator('.link-preview')).toHaveCount(0)
})

test('Apple catalog preview shows skipped rows and requires existing Copy confirmation', async ({ page }) => {
  await setup(page)
  const requests: string[] = []
  page.on('request', request => requests.push(new URL(request.url()).pathname))
  await page.route('**/api/playlists/inspect-link', route => route.fulfill({ json: {
    recognized: true, platform: 'apple_music', platform_label: 'Apple Music', playlist_id: 'pl.synthetic', playlist_id_valid: true,
    capability: 'TRACK_IMPORT_AVAILABLE', can_analyze: false, playlist_name: 'Synthetic Apple List', track_count: 1,
    preview_tracks: [{ id: 'synthetic', title: '中文 日本語 한국어', artists: ['Synthetic Artist'], platform: 'apple_music', external_ids: {}, genres: [], version_type: 'ORIGINAL' }],
    import_rows: [
      { track_title: '中文 日本語 한국어', artist: ['Synthetic Artist'], duration_ms: null, availability: 'UNKNOWN', import_status: 'IMPORTED' },
      { track_title: '中文 日本語 한국어', artist: ['Synthetic Artist'], availability: 'UNKNOWN', import_status: 'SKIPPED_DUPLICATE' },
      { track_title: null, artist: null, availability: 'UNAVAILABLE', import_status: 'SKIPPED_UNAVAILABLE_METADATA' },
    ], message: 'Synthetic contract only, not real verification',
  } }))
  await page.goto('/')
  await page.locator('#playlist-link').fill('https://music.apple.com/us/playlist/pl.synthetic')
  await page.getByRole('button', { name: '检查并导入预览' }).click()
  await expect(page.locator('.link-preview')).toContainText('中文 日本語 한국어')
  await page.locator('.link-result summary').click()
  await expect(page.locator('.link-result')).toContainText('重复曲目，已跳过')
  await expect(page.locator('.link-result')).toContainText('时长未知')
  await expect(page.getByRole('dialog')).toHaveCount(0)
  await page.getByRole('button', { name: '确认并进入 Copy Playlist 预览' }).click()
  await expect(page.getByRole('dialog', { name: '保存到音乐平台' })).toBeVisible()
  expect(requests.some(path => /analyze|execute/.test(path))).toBe(false)
})

test('UTF-16 desktop export is decoded without corrupting Unicode before shared preview', async ({ page }) => {
  await setup(page)
  let received = ''
  await page.route('**/api/imports/preview', route => {
    received = route.request().postDataJSON().content
    return route.fulfill({ status: 400, json: { error: 'Synthetic preview stop' } })
  })
  await page.goto('/')
  const content = 'Name\tArtist\tTime\n中文 日本語 한국어\tSynthetic\t3:05'
  await page.locator('#more-import input[type=file]').setInputFiles({ name: 'synthetic.txt', mimeType: 'text/plain', buffer: Buffer.concat([Buffer.from([0xff, 0xfe]), Buffer.from(content, 'utf16le')]) })
  await expect.poll(() => received).toBe(content)
})

test('all seven cards disclose credentials and separate real acceptance from file support', async ({ page }) => {
  await setup(page)
  await page.route('**/api/platforms/capabilities', route => route.fulfill({ json:
    ['spotify', 'youtube_music', 'apple_music', 'netease', 'qq_music', 'kugou', 'qishui'].map(platform => ({
      platform, display_name: platform, configured: false, auth_supported: ['spotify', 'youtube_music'].includes(platform),
      file_import_supported: true, public_link_import_supported: ['spotify', 'youtube_music', 'apple_music'].includes(platform),
      public_playlist_links: platform === 'apple_music' ? 'CONFIG_REQUIRED' : ['netease', 'qq_music'].includes(platform) ? 'ACCESSIBILITY_CHECK_ONLY' : 'URL_RECOGNITION_ONLY',
      status_label: platform === 'apple_music' ? 'OFFICIAL API · CONFIG REQUIRED' : 'FILE IMPORT',
    }))
  }))
  await page.goto('/')
  const cards = page.locator('#connection-panel .platform-group .platform-card')
  await expect(cards).toHaveCount(7)
  await expect(page.locator('#connection-panel')).not.toContainText(/WAITING_FOR_APPLE_DEVELOPER_CREDENTIALS|CONFIG_REQUIRED|AUTH_REQUIRED|URL_RECOGNITION_ONLY|ACCESSIBILITY_CHECK_ONLY|not_implemented/)
  const apple = cards.filter({ has: page.getByRole('heading', { name: 'apple_music', exact: true }) })
  await expect(apple.locator('.capability-label')).toHaveText('官方 API · 需部署者配置')
  await expect(apple).toContainText('私人资料库未支持')
  await expect(apple.getByRole('button', { name: '导入文件或文本', exact: true })).toBeVisible()
  await expect(apple.getByRole('button', { name: 'Apple 目录配置' })).toHaveCount(0)
  for (const name of ['apple_music', 'netease', 'qq_music', 'kugou', 'qishui']) {
    const card = cards.filter({ has: page.getByRole('heading', { name, exact: true }) })
    await expect(card).toContainText('尚未真人验收')
    await expect(card).toContainText('账号授权：未接入')
    await expect(card).toContainText('文件/文本：支持文件 / 文本导入')
    await expect(card.locator('a[href*="authorize"]')).toHaveCount(0)
  }
  await expect(page.locator('#connection-panel')).toContainText('自行部署')
  await expect(page.locator('#connection-panel')).toContainText('不要求用户提供账号密码或 Cookie')
  await expect(page.locator('#public-link-panel')).toContainText('中国音乐平台：支持公开链接检测与文件/文本导入，不需要账号密码')
})
