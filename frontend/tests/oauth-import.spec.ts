import { test, expect, type Page } from '@playwright/test'

for (const width of [390, 768]) {
  test(`responsive menu reaches every page and closes with Escape at ${width}px`, async ({ page }) => {
    await page.setViewportSize({ width, height: 740 })
    await setup(page)
    await page.route('**/api/settings', route => route.fulfill({ json: {
      endpoint: 'https://example.invalid', model: 'synthetic', temperature: 0,
      max_tokens: 100, max_agent_steps: 16, request_timeout_seconds: 10,
      retry_limit: 0, max_cost_usd: 0, input_price_per_million: 0,
      output_price_per_million: 0, api_key_available: false,
    } }))
    await page.goto('/')
    const toggle = page.locator('.mobile-menu-toggle')
    const nav = page.getByRole('navigation', { name: '主导航' })
    await expect(nav).toBeHidden()
    await page.screenshot({ path: `test-results/welcome-${width}.png` })
    await toggle.click()
    await expect(nav.getByRole('button')).toHaveCount(9)
    await page.keyboard.press('Escape')
    await expect(toggle).toBeFocused()
    await expect(toggle).toHaveAttribute('aria-expanded', 'false')
    for (const [entry, heading] of [
      ['Music Profile', '先导入歌单，了解你的音乐偏好'],
      ['Compare', '两份真实歌单，一次私密比较'],
      ['Version Radar', '批量寻找同一首歌的其他正式版本'],
      ['Agent ·', '从用户请求到工具结果，看清 Agent 的每一步'],
      ['History', '任务不会随页面消失'],
      ['Settings', '模型、预算与运行边界'],
      ['Discover', '先导入并确认一份歌单'],
      ['Copy Playlist', '选择来源与目标，预览后再复制'],
      ['Import', '从你的歌单出发，发现下一首喜欢的音乐'],
    ]) {
      await toggle.click()
      const button = nav.getByRole('button').filter({ hasText: entry })
      await button.scrollIntoViewIfNeeded()
      await button.click()
      await expect(nav).toBeHidden()
      await expect(page.getByRole('heading', { name: heading, exact: true })).toBeVisible()
    }
    expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true)
    await toggle.click()
    await page.screenshot({ path: `test-results/mobile-navigation-${width}.png` })
  })
}

test('welcome CTA opens import and desktop keeps its full navigation', async ({ page }) => {
  await page.setViewportSize({ width: 1440, height: 900 })
  await setup(page)
  await page.goto('/')
  await expect(page.locator('.mobile-menu-toggle')).toBeHidden()
  await expect(page.getByRole('navigation', { name: '主导航' }).getByRole('button')).toHaveCount(9)
  await expect(page.locator('.welcome-steps li')).toHaveCount(3)
  await page.screenshot({ path: 'test-results/welcome-desktop.png' })
  await expect(page.locator('.welcome-more')).not.toHaveAttribute('open', '')
  await page.locator('#more-import summary').click()
  await page.getByRole('button', { name: '第一步：导入我的歌单' }).click()
  await expect(page.locator('#more-import details')).toHaveAttribute('open', '')
  await expect(page.getByRole('button', { name: '解析并预览文本' })).toBeInViewport()
  await expect(page.locator('#more-import')).toContainText('周杰伦 - 晴天')
  await page.locator('.welcome-more summary').click()
  await page.getByRole('button', { name: 'Find another version' }).click()
  await expect(page).toHaveURL(/\/versions$/)
})

for (const [platform, capability, access, accessible] of [
  ['netease', 'ACCESSIBILITY_CHECK_ONLY', 'page_reachable', true],
  ['qq_music', 'ACCESSIBILITY_CHECK_ONLY', 'page_reachable', true],
  ['netease', 'ACCESSIBILITY_CHECK_ONLY', 'page_not_accessible', false],
  ['qq_music', 'ACCESSIBILITY_CHECK_ONLY', 'check_failed', null],
  ['kugou', 'URL_RECOGNITION_ONLY', 'not_checked', null],
  ['qishui', 'URL_RECOGNITION_ONLY', 'not_checked', null],
] as const) {
  test(`${platform} ${access} never implies track import`, async ({ page }) => {
    await setup(page)
    await page.route('**/api/playlists/inspect-link', route => route.fulfill({ json: {
      recognized: true, platform, platform_label: platform, capability,
      publicly_accessible: accessible, access_status: access, preview_tracks: [],
      track_count: null, can_analyze: false, message: 'Synthetic accessibility contract',
      next_step: 'Synthetic fixture only',
    } }))
    await page.goto('/')
    await page.locator('#playlist-link').fill('https://y.music.163.com/m/playlist?id=7736940069')
    await page.getByRole('button', { name: '检查链接读取能力' }).click()
    const result = page.locator('.link-result')
    await expect(result).toContainText('已识别公开歌单链接')
    await expect(result).toContainText(platform === 'netease' ? '检测到网易云歌单，但当前无法获取公开歌曲列表。请使用TXT/CSV备用导入。' : '当前无法通过官方接口读取完整歌曲列表')
    await expect(result).toContainText('CSV / TXT / JSON / M3U')
    await expect(result).toContainText('歌手 - 歌名')
    await expect(result).toContainText(accessible === true ? '页面可访问（不代表歌曲已读取）' : accessible === false ? '页面不可访问' : access === 'check_failed' ? '检查失败' : '未检查')
    await expect(result.locator('.link-preview')).toHaveCount(0)
    await expect(result.getByRole('button', { name: '确认并进入 Copy Playlist 预览' })).toHaveCount(0)
    await result.getByRole('button', { name: '导入文件或文本', exact: true }).click()
    await expect(page.locator('#more-import details')).toHaveAttribute('open', '')
    await expect(page.getByRole('button', { name: '解析并预览文本' })).toBeVisible()
  })
}

test('NetEase public metadata shows declared count without creating Import Preview', async ({ page }) => {
  await setup(page)
  // This contract test does not depend on the external Google Fonts stylesheet.
  await page.route('https://fonts.googleapis.com/**', route => route.fulfill({ contentType: 'text/css', body: '' }))
  let downstreamRequests = 0
  await page.route('**/api/import/**', route => { downstreamRequests++; return route.abort() })
  await page.route('**/api/playlists/inspect-link', route => route.fulfill({ json: {
    recognized: true, platform: 'netease', platform_label: '网易云音乐',
    capability: 'ACCESSIBILITY_CHECK_ONLY', publicly_accessible: true,
    access_status: 'page_reachable', playlist_id: '123456', playlist_id_valid: true,
    playlist_name: 'Synthetic 公开歌单', track_count: 15, preview_tracks: [], import_rows: [],
    structured_data_status: 'public_track_list_incomplete', can_analyze: false,
    message: '公开页面声明 15 首，JSON-LD 实际列出 10 项。公开歌曲列表不完整。未生成 Track。',
    next_step: '请使用文件或文本导入。',
  } }))
  await page.goto('/')
  await page.locator('#playlist-link').fill('https://y.music.163.com/m/playlist?id=123456')
  await page.getByRole('button', { name: '检查链接读取能力' }).click()
  const result = page.locator('.link-result')
  await expect(result).toContainText('Synthetic 公开歌单')
  await expect(result).toContainText('页面声明 15 首（不是已导入数量）')
  await expect(result).toContainText('JSON-LD 实际列出 10 项')
  await expect(result.locator('.link-preview')).toHaveCount(0)
  await expect(result.getByRole('button', { name: /确认/ })).toHaveCount(0)
  expect(downstreamRequests).toBe(0)
})

test('Agent guide is visible before execution and explicit Demo keeps its data label', async ({ page }) => {
  await setup(page)
  await page.route('**/api/tasks', route => route.fulfill({ json: {
    id: 'synthetic-agent', scenario: 'personal_exploration', goal: 'Synthetic request',
    status: 'COMPLETED', progress: 1, current_step: 1, message: 'Synthetic final explanation',
    decision_mode: 'DETERMINISTIC_FALLBACK', data_state: 'DEMO', retries: 0,
    input_tokens: 0, output_tokens: 0, estimated_cost_usd: 0,
    decisions_json: JSON.stringify([{ action: 'execute', next_tool: 'synthetic_tool', reason: 'Synthetic decision' }]),
    tool_calls_json: JSON.stringify([{ call_id: 'synthetic-call', tool: 'prepare_explanation', step_id: 'one', attempt: 1 }]),
    tool_results_json: JSON.stringify([{ call_id: 'synthetic-call', success: true, summary: 'Synthetic result', output: { summary: 'Synthetic analysis explanation' } }]),
  } }))
  await page.goto('/agent')
  await expect(page.getByRole('list', { name: 'Agent 展示流程' }).locator('li')).toHaveCount(5)
  await expect(page.getByRole('button', { name: '启动 Agent 工具循环' })).toBeDisabled()
  await page.getByRole('checkbox', { name: '明确使用 Demo' }).check()
  await page.getByRole('button', { name: '启动 Agent 工具循环' }).click()
  await expect(page.locator('.agent-message')).toContainText('DETERMINISTIC_FALLBACK')
  await expect(page.locator('.agent-message')).toContainText('DEMO')
  await expect(page.locator('.agent-trace')).toContainText('Synthetic decision')
  await expect(page.locator('.agent-trace')).toContainText('synthetic_tool')
  await expect(page.locator('.agent-trace')).toContainText('Synthetic result')
  await expect(page.locator('.agent-trace')).toContainText('Synthetic final explanation')
  await expect(page.locator('.agent-trace')).toContainText('Synthetic analysis explanation')
})

// Isolated synthetic fixtures. These tests do not claim real OAuth or platform acceptance.
async function setup(page: Page, youtubeConnected = false, youtubeError = false) {
  // Keep isolated UI contracts independent of external font availability.
  await page.route('https://fonts.googleapis.com/**', route => route.fulfill({ contentType: 'text/css', body: '' }))
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
  await page.getByRole('button', { name: '检查链接读取能力' }).click()
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
      await page.getByRole('button', { name: '检查链接读取能力' }).click()
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
  await page.getByRole('button', { name: '检查链接读取能力' }).click()
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
  await page.getByRole('button', { name: '检查链接读取能力' }).click()
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

test('first CSV upload renders Preview immediately and the same file can be selected again', async ({ page }) => {
  await setup(page)
  let previewCalls = 0
  await page.route('**/api/imports/preview', async route => {
    previewCalls += 1
    await new Promise(resolve => setTimeout(resolve, 250))
    await route.fulfill({ json: importPreviewFixture(`preview-${previewCalls}`) })
  })
  await page.goto('/')
  const input = page.locator('#more-import input[type=file]')
  const file = { name: 'same.csv', mimeType: 'text/csv', buffer: Buffer.from('artist,title\n周杰伦,晴天\nIU,Blueming') }
  await input.setInputFiles(file)
  await expect(page.locator('.import-operation')).toContainText('正在解析歌曲...')
  await expect(page.locator('.import-operation')).toContainText('已提交 2 / 2 行')
  const preview = page.locator('.import-preview')
  await expect(preview).toContainText('✅ 歌单解析成功')
  await expect(preview).toContainText('已读取：2 首歌曲')
  await expect(preview).toContainText('请查看下方 Preview ↓')
  await expect(preview).toContainText('晴天')
  await expect(preview).toContainText('✓ Success')
  await expect(preview).toContainText('Metadata 待匹配')
  await expect(preview.getByText('missing', { exact: true })).toHaveCount(0)
  await expect(preview).toBeInViewport()

  await input.setInputFiles(file)
  await expect.poll(() => previewCalls).toBe(2)
  await expect(preview).toContainText('✅ 歌单解析成功')
})

test('import failures explain missing fields and show supported examples', async ({ page }) => {
  await setup(page)
  await page.route('**/api/imports/preview', route => route.fulfill({ status: 400, json: { error: '未找到歌曲列，请提供 title 字段' } }))
  await page.goto('/')
  await page.locator('#more-import input[type=file]').setInputFiles({ name: 'bad.csv', mimeType: 'text/csv', buffer: Buffer.from('album,year\nSynthetic,2026') })
  const feedback = page.locator('.import-error-feedback')
  await expect(feedback).toContainText('缺少歌曲字段')
  await expect(feedback).toContainText('artist,title')
  await expect(feedback).toContainText('artist - title')
})

test('invalid file encoding is reported as an encoding problem', async ({ page }) => {
  await setup(page)
  await page.goto('/')
  await page.locator('#more-import input[type=file]').setInputFiles({ name: 'broken.csv', mimeType: 'text/csv', buffer: Buffer.from([0xc3, 0x28]) })
  await expect(page.locator('.import-error-feedback')).toContainText('编码问题')
})

test('real analysis clearly separates missing Last.fm configuration from import success', async ({ page }) => {
  await setup(page)
  await page.route('**/api/imports/preview', route => route.fulfill({ json: importPreviewFixture('lastfm-preview') }))
  await page.route('**/api/imports/*/analyze', route => route.fulfill({ json: analysisWithoutLastFmFixture() }))
  await page.goto('/')
  await page.locator('#more-import input[type=file]').setInputFiles({ name: 'songs.csv', mimeType: 'text/csv', buffer: Buffer.from('artist,title\n周杰伦,晴天\nIU,Blueming') })
  await page.getByRole('button', { name: '确认并分析真实数据' }).click()
  await page.getByRole('button', { name: '探索推荐', exact: true }).click()
  const status = page.locator('.lastfm-readiness')
  await expect(status).toContainText('⚠ Last.fm Recommendation Service Not Configured')
  await expect(status).toContainText('当前环境未配置 LASTFM_API_KEY')
  await expect(status).toContainText('不影响歌曲导入')
  await expect(status).toContainText('不影响音乐画像分析')
  await expect(status).toContainText('仅影响外部音乐推荐功能')
  await expect(status).toContainText('管理员配置 Last.fm API Key')
})

function importPreviewFixture(id: string) {
  return {
    id, name: 'Synthetic CSV', file_name: 'same.csv', data_state: 'REAL_FILE', source_label: '真实文件导入',
    total_rows: 2, parsed_count: 2, warning_count: 0, invalid_count: 0, detected_fields: ['artist', 'title'],
    preview_tracks: [
      { title: '晴天', artists: ['周杰伦'], genres: [], source: 'same.csv', original_row: '周杰伦,晴天', metadata_status: 'missing', metadata_confidence: 0, warnings: [] },
      { title: 'Blueming', artists: ['IU'], genres: [], source: 'same.csv', original_row: 'IU,Blueming', metadata_status: 'partial', metadata_confidence: 0.4, warnings: [] },
    ],
    requires_column_confirmation: false, text_order: 'artist_title', questions: [],
  }
}

test('NetEase partial import uses the existing preview and confirmed analysis flow', async ({ page }) => {
  await setup(page)
  let analyses = 0
  const preview = { ...importPreviewFixture('netease-preview'), file_name: undefined,
    name: 'Synthetic NetEase', source_label: '网易云公开歌单 · Imported 2 / 1196 tracks', data_state: 'REAL_PUBLIC_LINK',
    total_rows: 1196, invalid_count: 1194, questions: ['Imported 2 / 1196 tracks · 未导入 1194 首'],
  }
  await page.route('**/api/playlists/inspect-link', route => route.fulfill({ json: {
    recognized: true, platform: 'netease', platform_label: '网易云音乐', capability: 'TRACK_IMPORT_AVAILABLE',
    publicly_accessible: true, access_status: 'page_reachable', playlist_id_valid: true,
    playlist_name: 'Synthetic NetEase', track_count: 1196, preview_tracks: [{ title: '晴天', artists: ['周杰伦'] }, { title: 'Blueming', artists: ['IU'] }],
    can_analyze: true, message: '网易云歌单解析成功。已导入2/1196首歌曲；未导入1194首。 当前公开页面解析限制，仅导入前20首歌曲用于分析。', next_step: '核对预览后分析', import_preview: preview,
    import_rows: [{ track_title: 'Unavailable', source_platform: 'netease', import_status: 'SKIPPED_DETAIL_UNAVAILABLE', availability: 'UNKNOWN' }],
  } }))
  await page.route('**/api/imports/netease-preview/analyze', route => {
    analyses++
    return route.fulfill({ json: analysisWithoutLastFmFixture() })
  })
  await page.goto('/')
  await page.locator('#playlist-link').fill('https://y.music.163.com/m/playlist?id=123')
  await page.getByRole('button', { name: '检查链接读取能力' }).click()
  await expect(page.locator('.link-result')).toContainText('网易云歌单解析成功')
  await expect(page.locator('.link-result')).toContainText('已导入2/1196首歌曲')
  await expect(page.locator('.link-result')).toContainText('当前公开页面解析限制，仅导入前20首歌曲用于分析')
  await expect(page.locator('.import-preview')).toContainText('Imported 2 / 1196 tracks')
  await expect(page.locator('.import-preview')).toContainText('REAL_PUBLIC_LINK')
  await page.locator('.link-result summary').click()
  await expect(page.locator('.link-result')).toContainText('详情不可用或缺少艺人，未导入')
  await expect(page.getByRole('button', { name: '确认并进入 Copy Playlist 预览' })).toHaveCount(0)
  expect(analyses).toBe(0)
  await page.getByRole('button', { name: '确认并分析真实数据' }).click()
  await expect(page.getByRole('heading', { name: 'Synthetic CSV', exact: true })).toBeVisible()
  expect(analyses).toBe(1)
  await page.getByRole('button', { name: '探索推荐', exact: true }).click()
  await expect(page.locator('.lastfm-readiness')).toContainText('Last.fm Recommendation Service Not Configured')
})

function analysisWithoutLastFmFixture() {
  const track = { id: 'track-1', title: '晴天', normalized_title: '晴天', artists: ['周杰伦'], genres: [], platform: 'local', external_ids: {}, version_type: 'ORIGINAL', mood_tags: [], metadata_confidence: 0 }
  const queryStats = {
    successful_seed_count: 0, failed_seed_count: 0, raw_track_similar_count: 0, raw_artist_similar_count: 0,
    raw_artist_top_tracks_count: 0, raw_tag_top_tracks_count: 0, raw_candidate_count: 0, after_version_filter_count: 0,
    after_normalization_count: 0, after_deduplication_count: 0, after_source_exclusion_count: 0, after_artist_cap_count: 0,
    comfort_candidate_count: 0, expansion_candidate_count: 0, surprise_candidate_count: 0, tag_layer1_candidate_count: 0,
    tag_layer1_rejected_count: 0, tag_layer2_candidate_count: 0, tag_layer2_rejected_count: 0, core_tags: [],
    tag_similar_success_count: 0, tag_similar_failure_count: 0, similar_tag_count: 0, layer1_tags: [], layer2_tags: [],
    tag_top_track_counts: [], request_budget_exhausted_count: 0, request_budget_used_count: 0, retry_count: 0,
    genre_bridge_candidate_count: 0, second_hop_artist_candidate_count: 0, deduplicated_candidate_count: 0,
  }
  return {
    analysis_id: 'analysis-no-lastfm', playlist: { id: 'playlist-1', name: 'Synthetic CSV', owner_label: 'Local', source: 'file', is_demo: false, tracks: [track] },
    report: { playlist_name: 'Synthetic CSV', source_label: '真实文件导入', is_demo: false, track_count: 1, genre_distribution: [], artist_distribution: [['周杰伦', 1]], era_distribution: [], album_distribution: [], duplicate_track_count: 0, collaboration_track_count: 0, genre_matched_count: 0, genre_coverage: 0, energy_matched_count: 0, energy_coverage: 0, metrics: [], core_preferences: [], adjacent_preferences: [], unexplored_preferences: [], summary: '本地分析已完成', confidence: 0.5, limitations: [] },
    recommendations: [], route: [], unmatched_tracks: [], metadata_resolutions: [],
    recommendation_summary: { source_label: 'Last.fm Music Discovery API', status: 'not_configured', message: '未配置 Last.fm', candidate_count: 0, zones: [], seeds: [], query_stats: queryStats, comfort_pool: [], expansion_pool: [], surprise_pool: [] },
  }
}

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
  await expect(page.locator('#public-link-panel')).toContainText('网易云按公开页面可获取范围尝试导入')
  await expect(page.locator('#public-link-panel')).toContainText('最多 20 首 / 20 秒')
  await expect(page.locator('#public-link-panel')).toContainText('不需要账号密码或 Cookie')
})
