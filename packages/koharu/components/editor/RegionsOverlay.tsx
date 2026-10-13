'use client'

import { ScanText, Trash2 } from 'lucide-react'
import { useEffect, useState, type PointerEvent as ReactPointerEvent, type RefObject } from 'react'
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
  const [selected, setSelected] = useState<EntityId | null>(null)
  const [preview, setPreview] = useState<Frame | null>(null)
  const [drag, setDrag] = useState<Drag | null>(null)
  const region = regions.find((r) => r.id === selected)
  const { t } = useTranslation()
  const drawing = useKoharuStore((state) => state.drawRegion)
  const selectLayers = useKoharuStore((state) => state.selectLayers)
  const [sketch, setSketch] = useState<{ pointer: number; from: Point; to: Point } | null>(null)

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
      const target = event.target
      if (
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        (target instanceof HTMLElement && target.isContentEditable)
      )
        return
      if (event.key !== 'Delete' && event.key !== 'Backspace') return
      event.preventDefault()
      remove(selected)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [selected])

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
      {region && !drawing && (
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
