'use client'

import { useState, type PointerEvent as ReactPointerEvent, type RefObject } from 'react'

import { SelectionControls } from '@/components/editor/SelectionControls'
import { call } from '@/lib/backend'
import { cssFrame, pagePoint, type Camera } from '@/lib/geometry'
import { commands, type AnalysisRegion, type EntityId, type Frame } from '@koharu/bridge/protocol'

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
  regions,
  camera,
  container,
}: {
  regions: AnalysisRegion[]
  camera: Camera
  container: RefObject<HTMLDivElement | null>
}) {
  const [selected, setSelected] = useState<EntityId | null>(null)
  const [preview, setPreview] = useState<Frame | null>(null)
  const [drag, setDrag] = useState<Drag | null>(null)
  const region = regions.find((r) => r.id === selected)

  const commit = (frame: Frame | null) => {
    setPreview(null)
    if (!region || !frame) return
    void call(commands.setRegionGeometry, [{ layer: region.id, points: framePoints(frame) }])
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
      {region && (
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
