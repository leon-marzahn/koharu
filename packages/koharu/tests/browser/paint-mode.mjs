// Real-browser check of RegionsOverlay Paint mode at devicePixelRatio 2.
// Run from packages/koharu: `bun tests/browser/paint-mode.mjs`
// (needs `bunx playwright install chromium` once). Not part of `bun run test` (jsdom).
import assert from 'node:assert/strict'
import path from 'node:path'

import react from '@vitejs/plugin-react'
import { chromium } from 'playwright'
import { createServer } from 'vite'

const server = await createServer({
  configFile: false,
  plugins: [react()],
  resolve: {
    // Same aliases as vitest.config.ts, plus the CSS export that its '@koharu/ui' alias shadows.
    alias: [
      { find: '@koharu/ui/globals.css', replacement: path.resolve('../ui/src/styles/globals.css') },
      { find: '@koharu/ui', replacement: path.resolve('../ui/src') },
      { find: '@', replacement: path.resolve('.') },
    ],
  },
  server: { port: 0 },
  logLevel: 'error',
})
await server.listen()
const browser = await chromium.launch()
try {
  const page = await browser.newPage({
    deviceScaleFactor: 2,
    viewport: { width: 800, height: 600 },
  })
  page.on('pageerror', (error) => console.error(error))
  await page.goto(`${server.resolvedUrls.local[0]}tests/browser/paint-mode.html`)
  await page.getByRole('button', { name: 'tools.paintMask' }).click()
  await page.waitForFunction(() => window.calls.some((c) => c.cmd === 'get_painted_mask'))

  // Paint surface covers the region box (CSS 140..340 x 140..240) exactly (no pad).
  const surface = await page.getByTestId('region-paint-surface').boundingBox()
  assert.deepEqual(surface, { x: 140, y: 140, width: 200, height: 100 })

  // 3a. Stored mask (page px 120..160) is drawn: canvas px 20..60.
  const alphaAt = (x, y) =>
    page
      .getByTestId('region-paint-canvas')
      .evaluate((c, [x, y]) => c.getContext('2d').getImageData(x, y, 1, 1).data[3], [x, y])
  await page.waitForFunction(
    () =>
      document
        .querySelector('[data-testid=region-paint-canvas]')
        .getContext('2d')
        .getImageData(40, 40, 1, 1).data[3] > 0,
    null,
    { timeout: 3000 },
  )

  // 3b. A real drag leaves pixels: page (200,150)->(260,150) is CSS (240,190)->(300,190).
  await page.mouse.move(240, 190)
  await page.mouse.down()
  await page.mouse.move(270, 190, { steps: 5 })
  await page.mouse.move(300, 190, { steps: 5 })
  await page.mouse.up()
  assert.ok((await alphaAt(130, 50)) > 0, 'stroke not drawn')
  // Final window pixels, not just the canvas backing store.
  await page.screenshot({ path: `${process.env.TMPDIR ?? '/tmp'}/paint-mode.png` })

  // 1. With a stroke (so Apply/Undo are enabled), each toolbar button is topmost at its centre.
  for (const name of ['tools.cancelMask', 'tools.clearMask', 'tools.undoMask', 'tools.applyMask']) {
    const hit = await page.getByRole('button', { name }).evaluate((button) => {
      const r = button.getBoundingClientRect()
      const top = document.elementFromPoint(r.x + r.width / 2, r.y + r.height / 2)
      return top?.closest('button') === button ? 'ok' : top?.outerHTML.slice(0, 120)
    })
    assert.equal(hit, 'ok', `${name} covered by ${hit}`)
  }

  // 2. Ctrl+Z undoes the stroke only; the global handler never sees it.
  await page.keyboard.press('Control+z')
  assert.equal(await alphaAt(130, 50), 0, 'stroke not undone')
  assert.ok((await alphaAt(40, 40)) > 0, 'stored mask lost on undo')
  assert.equal(await page.evaluate(() => window.globalUndos), 0, 'global undo also ran')

  // Apply sends the stroke; toolbar click goes through.
  await page.mouse.move(240, 190)
  await page.mouse.down()
  await page.mouse.move(250, 200, { steps: 3 })
  await page.mouse.up()
  await page.getByRole('button', { name: 'tools.applyMask' }).click()
  assert.ok(await page.evaluate(() => window.calls.some((c) => c.cmd === 'paint_region_mask')))
  console.log('paint-mode: ok')
} finally {
  await browser.close()
  await server.close()
}
