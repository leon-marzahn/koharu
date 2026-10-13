'use client'

import { Brush, Check, Eraser, ScanText, Trash2, Undo2, X } from 'lucide-react'
import {
  useEffect,
  useRef,
  useState,
  type PointerEvent as ReactPointerEvent,
  type RefObject,
} from 'react'
import { useTranslation } from 'react-i18next'

import { SelectionControls } from '@/components/editor/SelectionControls'
import { call } from '@/lib/backend'
import { cssFrame, pagePoint, type Camera } from '@/lib/geometry'
import { pageKey, pagesKey, projectKey, refresh } from '@/lib/queries'
import { useKoharuStore } from '@/lib/store'
import {
  commands,
  type AnalysisRegion,
  type EntityId,
  type Frame,
  type Point,
} from '@koharu/bridge/protocol'
import { Button } from '@koharu/ui/components/button'

// Detection regions are edited as their axis-aligned bounds (angle 0 until rotated).
// Overlay-only DOM: it is never drawn into the WebGPU canvas, so it cannot reach export.
function regionFrame({ geometry }: AnalysisRegion): Frame {
  const xs = geometry.points.map((p) => p.x)
  const ys = geometry.points.map((p) => p.y)
  const x = Math.min(...xs)
  const y = Math.min(...ys)
  return { x, y, width: Math.max(...xs) - x, height: Math.max(...ys) - y, angle_degrees: 0 }
}

// A painted mask may reach 2 px past the region box: the padding the backend cleans with.
const PAINT_PAD = 2
function paddedFrame(frame: Frame): Frame {
  return {
    ...frame,
    x: frame.x - PAINT_PAD,
    y: frame.y - PAINT_PAD,
    width: frame.width + PAINT_PAD * 2,
    height: frame.height + PAINT_PAD * 2,
  }
}

function framePoints({ x, y, width, height, angle_degrees }: Frame) {
  const [cx, cy] = [x + width / 2, y + height / 2]
  const [sin, cos] = [
    Math.sin((angle_degrees * Math.PI) / 180),
    Math.cos((angle_degrees * Math.PI) / 180),
  ]
  return [
    [x, y],
    [x + width, y],
    [x + width, y + height],
    [x, y + height],
  ].map(([px, py]) => ({
    x: cx + (px - cx) * cos - (py - cy) * sin,
    y: cy + (px - cx) * sin + (py - cy) * cos,
  }))
}

interface Drag {
  pointer: number
  start: { x: number; y: number }
  original: Frame
}

export function RegionsOverlay({
  page,
  regions,
  camera,
  container,
}: {
  page: EntityId
  regions: AnalysisRegion[]
  camera: Camera
  container: RefObject<HTMLDivElement | null>
}) {
  const selected = useKoharuStore((state) => state.selectedRegion)
  const setSelected = useKoharuStore((state) => state.selectRegion)
  const [preview, setPreview] = useState<Frame | null>(null)
  const [drag, setDrag] = useState<Drag | null>(null)
  const region = regions.find((r) => r.id === selected)
  const present = Boolean(region)
  const { t } = useTranslation()
  const drawing = useKoharuStore((state) => state.drawRegion)
  const selectLayers = useKoharuStore((state) => state.selectLayers)
  const [sketch, setSketch] = useState<{ pointer: number; from: Point; to: Point } | null>(null)
  const brush = useKoharuStore((state) => state.brush)
  // Paint mask mode lives only in this overlay (no canvas edit is started), so cancelling
  // or leaving it cannot strand an active edit.
  // `strokes` are only the new ones; `cleared` drops the stored mask so Apply replaces it.
  const [painting, setPainting] = useState<{
    pointer: number | null
    strokes: Point[][]
    cleared: boolean
  } | null>(null)
  const [storedMask, setStoredMask] = useState<ImageBitmap | null>(null)
  const paintFrame = region && painting ? paddedFrame(regionFrame(region)) : null
  // Fetched only in Paint mode; re-entering Paint mode after Apply refetches it.
  const maskRegion = painting ? region?.id : undefined
  const canvas = useRef<HTMLCanvasElement | null>(null)

  // The stored mask is a page-sized grayscale PNG (white = masked).
  useEffect(() => {
    if (!maskRegion) return
    let live = true
    let bitmap: ImageBitmap | null = null
    void call(commands.getPaintedMask, maskRegion)
      .then(async (bytes) => {
        if (!live || !bytes.length) return
        bitmap = await createImageBitmap(new Blob([new Uint8Array(bytes)], { type: 'image/png' }))
        if (live) setStoredMask(bitmap)
        else bitmap.close()
      })
      .catch(() => undefined)
    return () => {
      live = false
      setStoredMask(null)
      bitmap?.close()
    }
  }, [maskRegion])

  // The canvas works in page pixels (its CSS size stretches it over the box), so neither
  // zoom nor devicePixelRatio enters the drawing: mixing them is what hid the old overlay.
  useEffect(() => {
    const context = canvas.current?.getContext('2d')
    if (!context || !paintFrame || !painting) return
    const { width, height } = context.canvas
    context.clearRect(0, 0, width, height)
    const color = getComputedStyle(context.canvas).color
    if (storedMask && !painting.cleared) {
      // Luminance to alpha, then tint: white mask pixels show, black ones vanish.
      const scratch = new OffscreenCanvas(width, height)
      const s = scratch.getContext('2d')!
      s.drawImage(storedMask, -paintFrame.x, -paintFrame.y)
      const pixels = s.getImageData(0, 0, width, height)
      for (let i = 0; i < pixels.data.length; i += 4) pixels.data[i + 3] = pixels.data[i]
      s.putImageData(pixels, 0, 0)
      s.globalCompositeOperation = 'source-in'
      s.fillStyle = color
      s.fillRect(0, 0, width, height)
      context.drawImage(scratch, 0, 0)
    }
    context.strokeStyle = color
    context.lineCap = 'round'
    context.lineJoin = 'round'
    context.lineWidth = brush.diameter
    for (const stroke of painting.strokes) {
      context.beginPath()
      // A lone point repeats so the round cap still draws a dot.
      for (const p of stroke.length === 1 ? [stroke[0], stroke[0]] : stroke)
        context.lineTo(p.x - paintFrame.x, p.y - paintFrame.y)
      context.stroke()
    }
  })
  // ponytail: the backend clips again, so this clamp only keeps the preview honest.
  const clamp = (p: Point, f: Frame): Point => ({
    x: Math.min(Math.max(p.x, f.x), f.x + f.width),
    y: Math.min(Math.max(p.y, f.y), f.y + f.height),
  })

  const applyMask = () => {
    if (!region || !painting?.strokes.length) return
    const { strokes, cleared } = painting
    setPainting(null)
    void call(commands.paintRegionMask, region.id, strokes, brush.diameter, cleared)
      .then(() => refresh(projectKey, pagesKey, pageKey))
      .catch(() => undefined)
  }

  const undoStroke = () =>
    setPainting((current) => current && { ...current, strokes: current.strokes.slice(0, -1) })

  const painted = painting !== null
  useEffect(() => {
    if (!painted) return
    // Capture phase on window runs before CanvasWorkspace's bubble-phase shortcut, so the
    // stroke undo owns Cmd/Ctrl+Z exclusively while painting.
    const onKey = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'z') {
        event.preventDefault()
        event.stopImmediatePropagation()
        undoStroke()
      }
    }
    window.addEventListener('keydown', onKey, true)
    return () => window.removeEventListener('keydown', onKey, true)
  }, [painted])

  // Deleting a region also deletes the text box recognized from it (one undo step).
  const remove = (id: EntityId) => {
    setSelected(null)
    void call(commands.deleteRegions, [id])
      .then(() => refresh(projectKey, pagesKey, pageKey))
      .catch(() => undefined)
  }

  useEffect(() => {
    if (!selected) return
    const onKey = (event: KeyboardEvent) => {
      // Escape cancels paint mode first; only a second press deselects.
      if (event.key === 'Escape') return painted ? setPainting(null) : setSelected(null)
      const target = event.target
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      )
        return
      if (event.key !== 'Delete' && event.key !== 'Backspace') return
      // The id lives in the store, so it can outlive a page switch: delete only a region shown here.
      if (!present) return
      event.preventDefault()
      remove(selected)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected, painted, present])

  // Draw a user region, then OCR, translate and clean only that region.
  const finishSketch = (from: Point, to: Point) => {
    if (Math.abs(from.x - to.x) < 2 || Math.abs(from.y - to.y) < 2) return
    void call(commands.drawRegion, page, from, to)
      .then(async (id) => {
        await refresh(projectKey, pagesKey, pageKey)
        await call(commands.processRegion, id, true)
      })
      .catch(() => undefined)
  }

  // Keep the preview until the refreshed page arrives, otherwise the region snaps back to
  // its stale geometry while the attached text has already moved.
  const commit = (frame: Frame | null) => {
    if (!region || !frame) return setPreview(null)
    void call(commands.setRegionGeometry, [{ layer: region.id, points: framePoints(frame) }])
      .then(() => refresh(projectKey, pagesKey, pageKey))
      .catch(() => undefined)
      .finally(() => setPreview(null))
  }

  const point = (event: ReactPointerEvent) => {
    const bounds = container.current?.getBoundingClientRect()
    return bounds ? pagePoint(event.clientX, event.clientY, bounds, camera) : null
  }

  return (
    <>
      {regions.map((item) => {
        const frame = item.id === selected && preview ? preview : regionFrame(item)
        const css = cssFrame(frame, camera)
        return (
          <div
            key={item.id}
            data-testid={`region-${item.id}`}
            data-region-kind={item.kind}
            className='pointer-events-auto absolute box-border cursor-move touch-none border border-dashed border-[var(--canvas-region-stroke)]'
            style={{
              left: css.left,
              top: css.top,
              width: css.width,
              height: css.height,
              transform: `rotate(${css.angle}deg)`,
            }}
            onPointerDown={(event) => {
              if (event.button !== 0) return
              event.stopPropagation()
              setPainting(null)
              setSelected(item.id)
              selectLayers([]) // so Delete targets the region, not a selected text layer
              const start = point(event)
              if (!start) return
              event.currentTarget.setPointerCapture(event.pointerId)
              setDrag({ pointer: event.pointerId, start, original: regionFrame(item) })
            }}
            onPointerMove={(event) => {
              const now = point(event)
              if (!drag || drag.pointer !== event.pointerId || !now) return
              setPreview({
                ...drag.original,
                x: drag.original.x + now.x - drag.start.x,
                y: drag.original.y + now.y - drag.start.y,
              })
            }}
            onPointerUp={(event) => {
              if (!drag || drag.pointer !== event.pointerId) return
              setDrag(null)
              commit(preview)
            }}
          />
        )
      })}
      {region && !drawing && (
        // The wrapper carries the offset: Button's own active:translate-y-px would override it
        // while pressed and make the button jump away from the cursor.
        <div
          className='pointer-events-none absolute flex -translate-y-full gap-1'
          style={{
            left: cssFrame(preview ?? regionFrame(region), camera).left,
            top: cssFrame(preview ?? regionFrame(region), camera).top,
          }}
        >
          <Button
            type='button'
            variant='secondary'
            size='icon'
            aria-label={t('tools.reocrRegion')}
            className='pointer-events-auto size-6 active:not-aria-[haspopup]:translate-y-0'
            onPointerDown={(event) => event.stopPropagation()}
            onClick={() =>
              void call(commands.processRegion, region.id, false).catch(() => undefined)
            }
          >
            <ScanText className='size-3.5' />
          </Button>
          {painting ? (
            <>
              <Button
                type='button'
                variant='secondary'
                size='icon'
                aria-label={t('tools.applyMask')}
                disabled={!painting.strokes.length}
                className='pointer-events-auto size-6 active:not-aria-[haspopup]:translate-y-0'
                onPointerDown={(event) => event.stopPropagation()}
                onClick={applyMask}
              >
                <Check className='size-3.5' />
              </Button>
              <Button
                type='button'
                variant='secondary'
                size='icon'
                aria-label={t('tools.undoMask')}
                disabled={!painting.strokes.length}
                className='pointer-events-auto size-6 active:not-aria-[haspopup]:translate-y-0'
                onPointerDown={(event) => event.stopPropagation()}
                onClick={undoStroke}
              >
                <Undo2 className='size-3.5' />
              </Button>
              <Button
                type='button'
                variant='secondary'
                size='icon'
                aria-label={t('tools.clearMask')}
                disabled={painting.cleared && !painting.strokes.length}
                className='pointer-events-auto size-6 active:not-aria-[haspopup]:translate-y-0'
                onPointerDown={(event) => event.stopPropagation()}
                onClick={() => setPainting({ ...painting, strokes: [], cleared: true })}
              >
                <Eraser className='size-3.5' />
              </Button>
              <Button
                type='button'
                variant='secondary'
                size='icon'
                aria-label={t('tools.cancelMask')}
                className='pointer-events-auto size-6 active:not-aria-[haspopup]:translate-y-0'
                onPointerDown={(event) => event.stopPropagation()}
                onClick={() => setPainting(null)}
              >
                <X className='size-3.5' />
              </Button>
            </>
          ) : (
            <Button
              type='button'
              variant='secondary'
              size='icon'
              aria-label={t('tools.paintMask')}
              className='pointer-events-auto size-6 active:not-aria-[haspopup]:translate-y-0'
              onPointerDown={(event) => event.stopPropagation()}
              onClick={() => setPainting({ pointer: null, strokes: [], cleared: false })}
            >
              <Brush className='size-3.5' />
            </Button>
          )}
          <Button
            type='button'
            variant='secondary'
            size='icon'
            aria-label={t('tools.deleteRegion')}
            className='pointer-events-auto size-6 active:not-aria-[haspopup]:translate-y-0'
            onPointerDown={(event) => event.stopPropagation()}
            onClick={() => remove(region.id)}
          >
            <Trash2 className='size-3.5' />
          </Button>
        </div>
      )}
      {painting && paintFrame && (
        <div
          data-testid='region-paint-surface'
          className='pointer-events-auto absolute cursor-crosshair touch-none overflow-hidden bg-[var(--canvas-region-stroke)]/15 outline-2 outline-[var(--canvas-region-stroke)]'
          style={{ ...cssFrame(paintFrame, camera), transform: undefined }}
          onPointerDown={(event) => {
            if (event.button !== 0) return
            event.stopPropagation()
            const at = point(event)
            if (!at) return
            event.currentTarget.setPointerCapture?.(event.pointerId)
            // Each press starts its own stroke: lifting the pen must not draw a connecting line.
            setPainting({
              ...painting,
              pointer: event.pointerId,
              strokes: [...painting.strokes, [clamp(at, paintFrame)]],
            })
          }}
          onPointerMove={(event) => {
            const at = point(event)
            if (painting.pointer !== event.pointerId || !at) return
            const strokes = painting.strokes.slice()
            strokes[strokes.length - 1] = [...strokes[strokes.length - 1], clamp(at, paintFrame)]
            setPainting({ ...painting, strokes })
          }}
          onPointerUp={(event) => {
            if (painting.pointer === event.pointerId) setPainting({ ...painting, pointer: null })
          }}
          onPointerCancel={() => setPainting({ ...painting, pointer: null })}
        >
          <canvas
            ref={canvas}
            data-testid='region-paint-canvas'
            width={Math.round(paintFrame.width)}
            height={Math.round(paintFrame.height)}
            className='pointer-events-none absolute inset-0 size-full text-[var(--canvas-region-stroke)] opacity-60'
          />
        </div>
      )}
      {drawing && (
        <div
          data-testid='region-draw-surface'
          className='pointer-events-auto absolute inset-0 cursor-crosshair touch-none'
          onPointerDown={(event) => {
            if (event.button !== 0) return
            event.stopPropagation()
            const from = point(event)
            if (!from) return
            event.currentTarget.setPointerCapture?.(event.pointerId)
            setSketch({ pointer: event.pointerId, from, to: from })
          }}
          onPointerMove={(event) => {
            const to = point(event)
            if (sketch && sketch.pointer === event.pointerId && to) setSketch({ ...sketch, to })
          }}
          onPointerUp={(event) => {
            if (!sketch || sketch.pointer !== event.pointerId) return
            const to = point(event) ?? sketch.to
            setSketch(null)
            finishSketch(sketch.from, to)
          }}
          onPointerCancel={() => setSketch(null)}
        >
          {sketch && (
            <div
              className='absolute box-border border border-dashed border-[var(--canvas-region-stroke)]'
              style={{
                ...cssFrame(
                  {
                    x: Math.min(sketch.from.x, sketch.to.x),
                    y: Math.min(sketch.from.y, sketch.to.y),
                    width: Math.abs(sketch.from.x - sketch.to.x),
                    height: Math.abs(sketch.from.y - sketch.to.y),
                    angle_degrees: 0,
                  },
                  camera,
                ),
                transform: undefined,
              }}
            />
          )}
        </div>
      )}
      {/* Hidden while painting: its rotate handle sits over the toolbar and its edge handles
          over the paint surface, so both would take the clicks meant for Paint mode. */}
      {region && !drawing && !painting && (
        <SelectionControls
          container={container}
          element={region.id}
          frame={preview ?? regionFrame(region)}
          camera={camera}
          edgesOnly
          onTransformStart={([e]) => setPreview(e.frame)}
          onTransformFrame={([e]) => setPreview(e.frame)}
          onTransformEnd={() => commit(preview)}
        />
      )}
    </>
  )
}
