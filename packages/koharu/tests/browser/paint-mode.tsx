// Mounts the real RegionsOverlay (with its toolbar and SelectionControls) for paint-mode.mjs.
// The backend is mocked at the Tauri IPC boundary, so the real `call`/`commands` path runs.
import '@/app/globals.css'
import { useRef } from 'react'
import { createRoot } from 'react-dom/client'

import { RegionsOverlay } from '@/components/editor/RegionsOverlay'
import { useKoharuStore } from '@/lib/store'
import type { AnalysisRegion } from '@koharu/bridge/protocol'

declare global {
  interface Window {
    __TAURI_INTERNALS__: unknown
    calls: Array<{ cmd: string; args: unknown }>
    globalUndos: number
  }
}

// Page-sized stored mask: white square inside the region (page px 120..160).
const mask = new OffscreenCanvas(800, 600)
const m = mask.getContext('2d')!
m.fillStyle = '#000'
m.fillRect(0, 0, 800, 600)
m.fillStyle = '#fff'
m.fillRect(120, 120, 40, 40)
// Tauri delivers raw binary replies as an ArrayBuffer, not number[].
const maskBytes = await (await mask.convertToBlob()).arrayBuffer()

window.calls = []
window.__TAURI_INTERNALS__ = {
  transformCallback: () => 0,
  invoke: async (cmd: string, args: unknown) => {
    window.calls.push({ cmd, args })
    return cmd === 'get_painted_mask' ? maskBytes : null
  },
}

// Stand-in for CanvasWorkspace's global shortcut: a bubble-phase window keydown listener.
window.globalUndos = 0
window.addEventListener('keydown', (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'z') window.globalUndos++
})

const region = {
  id: 'r1',
  parent: null,
  kind: 'user',
  label: null,
  geometry: {
    points: [
      { x: 100, y: 100 },
      { x: 300, y: 100 },
      { x: 300, y: 200 },
      { x: 100, y: 200 },
    ],
  },
} as unknown as AnalysisRegion
useKoharuStore.getState().selectRegion(region.id)

// zoom 2 in device px at DPR 2: one page px is one CSS px, offset 40 CSS px.
const camera = { zoom: 2, translation: [80, 80] as [number, number] }

function Harness() {
  const root = useRef<HTMLDivElement>(null)
  return (
    <div
      ref={root}
      data-testid='canvas-overlay'
      className='pointer-events-none absolute inset-0 overflow-hidden'
    >
      <RegionsOverlay page='p1' regions={[region]} camera={camera} container={root} />
    </div>
  )
}

createRoot(document.getElementById('root')!).render(<Harness />)
