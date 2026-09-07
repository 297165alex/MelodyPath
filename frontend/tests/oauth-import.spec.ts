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
        await expect(page.locator('.link-result')).toContainText('AUTH_REQUIRED')
        await expect(page.locator('.link-preview')).toHaveCount(0)
        await expect(page.locator('.link-result a')).toHaveAttribute('href', `/api/${platform}/authorize`)
      }
      expect(requests.some(path => /analyze|execute/.test(path))).toBe(false)
      await expect(page.locator('.experimental-status')).toContainText('Experimental')
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
