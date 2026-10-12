'use client'

import { Plus } from 'lucide-react'
import { useMemo, useRef } from 'react'

import { RegionsOverlay } from '@/components/editor/RegionsOverlay'
import { SelectionControls } from '@/components/editor/SelectionControls'
import { effectiveLayerVisibility, expandLayerSelection, isTextLayer } from '@/lib/document'
import {
  controlFrame,
  cssFrame,
  selectableLayer,
  type Camera,
  type SnapGuide,
} from '@/lib/geometry'
import { useKoharuStore } from '@/lib/store'
import type {
  EntityId,
  Frame,
  Geometry,
  Page,
  Point,
  TransformFrame,
} from '@koharu/bridge/protocol'

interface CanvasOverlayProps {
  page: Page
  camera: Camera
  selected: EntityId[]
  hovered: EntityId | null
  frames: Readonly<Record<EntityId, Frame>>
  previews: Readonly<Record<EntityId, Frame>>
  draft: Frame | null
  snapGuide?: SnapGuide | null
  cursor: Point | null
  brushSize: number
  showBrushCursor: boolean
  onTransformStart: (elements: TransformFrame[]) => void
  onTransformFrame: (elements: TransformFrame[]) => void
  onTransformEnd: () => void
}

export function CanvasOverlay({
  page,
  camera,
  selected,
  hovered,
  frames,
  previews,
  draft,
  snapGuide,
  cursor,
  brushSize,
  showBrushCursor,
  onTransformStart,
  onTransformFrame,
  onTransformEnd,
}: CanvasOverlayProps) {
  const root = useRef<HTMLDivElement>(null)
  const showRegions = useKoharuStore((state) => state.showRegions)
  const expandedSelection = useMemo(
    () => expandLayerSelection(page.layers, selected),
    [page.layers, selected],
  )
  const selectedIds = useMemo(() => new Set(expandedSelection), [expandedSelection])
  const multipleSelected = expandedSelection.length > 1
  const layers = useMemo(
    () =>
      page.layers.flatMap((layer) => {
        const visibility = effectiveLayerVisibility(page.layers, layer)
        if (!visibility.visible || visibility.opacity <= 0) return []
        const frame = previews[layer.id] ?? controlFrame(layer, frames)
        return frame ? [{ layer, frame, opacity: visibility.opacity }] : []
      }),
    [page.layers, previews, frames],
  )
  const selectedLayer =
    expandedSelection.length === 1
      ? page.layers.find((layer) => layer.id === expandedSelection[0])
      : undefined
  const selectedTextLayer = selectedLayer && isTextLayer(selectedLayer) ? selectedLayer : undefined
  const automaticRegion = selectedTextLayer?.automatic_region
    ? page.regions.find((region) => region.id === selectedTextLayer.automatic_region)
    : undefined
  const selectionControl = multipleSelected
    ? undefined
    : layers.find(({ layer }) => selectedIds.has(layer.id) && selectableLayer(layer))
  const scale = camera.zoom / window.devicePixelRatio

  return (
    <div
      ref={root}
      data-testid='canvas-overlay'
      className='pointer-events-none absolute inset-0 overflow-hidden'
      aria-hidden
    >
      {automaticRegion && (
        <AutomaticRegionOverlay geometry={automaticRegion.geometry} camera={camera} />
      )}
      {layers.map(({ layer, frame, opacity }) => {
        const position = cssFrame(frame, camera)
        const selected = selectedIds.has(layer.id) && selectableLayer(layer)
        const highlighted = !selected && hovered === layer.id
        return (
          <div
            key={layer.id}
            data-element={layer.id}
            className='absolute box-border bg-transparent'
            style={{
              left: position.left,
              top: position.top,
              width: position.width,
              height: position.height,
              transform: `rotate(${position.angle}deg)`,
              transformOrigin: '50% 50%',
              border:
                highlighted || (selected && multipleSelected)
                  ? '1px solid var(--canvas-selection)'
                  : undefined,
              opacity,
              willChange: selected ? 'left, top, width, height, transform' : undefined,
            }}
          />
        )
      })}

      {showRegions && (
        <RegionsOverlay page={page.id} regions={page.regions} camera={camera} container={root} />
      )}
      {draft && <DraftOverlay frame={draft} camera={camera} />}
      {snapGuide && <SnapGuideLines guide={snapGuide} camera={camera} />}
      {showBrushCursor && cursor && (
        <div
          className='absolute rounded-full border border-white/95 shadow-[0_0_0_1px_rgb(0_0_0/0.9),0_1px_3px_rgb(0_0_0/0.45)]'
          style={{
            left: cursor.x / window.devicePixelRatio - (brushSize * scale) / 2,
            top: cursor.y / window.devicePixelRatio - (brushSize * scale) / 2,
            width: brushSize * scale,
            height: brushSize * scale,
          }}
        />
      )}

      {selectedTextLayer && selectionControl && (
        <CenterMark frame={selectionControl.frame} camera={camera} />
      )}

      {selectionControl && (
        <SelectionControls
          container={root}
          element={selectionControl.layer.id}
          frame={selectionControl.frame}
          camera={camera}
          edgesOnly={Boolean(selectedTextLayer)}
          onTransformStart={onTransformStart}
          onTransformFrame={onTransformFrame}
          onTransformEnd={onTransformEnd}
        />
      )}
    </div>
  )
}

// Overlay-only guide: a DOM element above the canvas, so it can never reach rendered or exported pixels.
function CenterMark({ frame, camera }: { frame: Frame; camera: Camera }) {
  const { left, top, width, height } = cssFrame(frame, camera)
  return (
    <Plus
      data-testid='center-mark'
      aria-hidden='true'
      className='absolute size-3 -translate-x-1/2 -translate-y-1/2 text-[var(--canvas-selection)] opacity-50'
      style={{ left: left + width / 2, top: top + height / 2 }}
    />
  )
}

// Overlay-only faint line(s) through the bubble center while a drag is snapped.
function SnapGuideLines({ guide, camera }: { guide: SnapGuide; camera: Camera }) {
  const { left, top, width, height } = cssFrame({ ...guide.bubble, angle_degrees: 0 }, camera)
  const line = 'pointer-events-none absolute bg-[var(--canvas-selection)] opacity-40'
  return (
    <>
      {guide.x !== undefined && (
        <div
          data-testid='snap-guide-x'
          aria-hidden='true'
          className={line}
          style={{ left: left + width / 2, top, width: 1, height }}
        />
      )}
      {guide.y !== undefined && (
        <div
          data-testid='snap-guide-y'
          aria-hidden='true'
          className={line}
          style={{ left, top: top + height / 2, width, height: 1 }}
        />
      )}
    </>
  )
}

function AutomaticRegionOverlay({ geometry, camera }: { geometry: Geometry; camera: Camera }) {
  const dpr = window.devicePixelRatio
  const points = geometry.points
    .map(
      (point) =>
        `${(point.x * camera.zoom + camera.translation[0]) / dpr},${(point.y * camera.zoom + camera.translation[1]) / dpr}`,
    )
    .join(' ')
  if (!points) return null

  return (
    <svg className='absolute inset-0 size-full overflow-hidden' data-testid='text-fit-region'>
      <polygon
        points={points}
        fill='none'
        stroke='var(--canvas-region-contrast)'
        strokeWidth='7'
        strokeDasharray='10 6'
        strokeLinecap='round'
        strokeLinejoin='round'
        vectorEffect='non-scaling-stroke'
      />
      <polygon
        points={points}
        fill='none'
        stroke='var(--canvas-region-stroke)'
        strokeWidth='3'
        strokeDasharray='10 6'
        strokeLinecap='round'
        strokeLinejoin='round'
        vectorEffect='non-scaling-stroke'
      />
    </svg>
  )
}

function DraftOverlay({ frame, camera }: { frame: Frame; camera: Camera }) {
  const position = cssFrame(frame, camera)
  return (
    <div
      className='absolute border border-dashed border-primary bg-primary/5'
      style={{
        left: position.left,
        top: position.top,
        width: position.width,
        height: position.height,
        transform: `rotate(${position.angle}deg)`,
      }}
    />
  )
}
