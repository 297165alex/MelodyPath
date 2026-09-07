import { test, expect } from '@playwright/test'

test('public demo explains expiration and independent self-host credentials', async ({ page }) => {
  await page.route('**/api/**', route => route.fulfill({ json: [] }))
  await page.route('**/api/public-config', route => route.fulfill({ json: { public_demo_expires_at: '2030-02-28' } }))
  await page.goto('/')
  const notice = page.getByRole('complementary', { name: '课程 Demo 说明' })
  await expect(notice).toContainText('2030-02-28')
  await expect(notice).toContainText('必须配置自己的 Developer Credentials')
  await expect(notice).toContainText('不能共享')
})

test('local public config does not display an expiration notice', async ({ page }) => {
  await page.route('**/api/**', route => route.fulfill({ json: [] }))
  await page.route('**/api/public-config', route => route.fulfill({ json: { public_demo_expires_at: null } }))
  await page.goto('/')
  await expect(page.getByRole('complementary', { name: '课程 Demo 说明' })).toHaveCount(0)
})
