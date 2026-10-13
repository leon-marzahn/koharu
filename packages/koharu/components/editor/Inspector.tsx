'use client'

import type { TFunction } from 'i18next'
import {
  AlignCenter,
  AlignLeft,
  AlignRight,
  ArrowDown,
  ArrowUp,
  Brush,
  ChevronDown,
  Eye,
  EyeOff,
  ClipboardPaste,
  Copy,
  Folder,
  Image as ImageIcon,
  Layers3,
  Lock,
  LockOpen,
  Minus,
  Plus,
  RotateCcw,
  Trash2,
  Type,
} from 'lucide-react'
import { type MouseEvent, useEffect, useId, useMemo, useRef, useState } from 'react'
import { useTranslation } from 'react-i18next'

import { ColorWell } from '@/components/controls/ColorWell'
import { CommitTextarea } from '@/components/controls/CommitTextarea'
import { FontPicker } from '@/components/controls/FontPicker'
import { call } from '@/lib/backend'
import { copyStyle, pasteStyle } from '@/lib/clipboard'
import {
  defaultTypography,
  expandLayerSelection,
  isGroupLayer,
  isLockedLayer,
  isTextLayer,
  layerChildren,
} from '@/lib/document'
import { pageKey, projectKey, queryClient, refresh, useFonts, usePage } from '@/lib/queries'
import { useKoharuStore } from '@/lib/store'
import { previewCanvasOpacity } from '@koharu/bridge/canvas'
import {
  commands,
  type EntityId,
  type FontFamily,
  type FontStyle,
  type Layer,
  type TextAlignment,
  type Typography,
  type WritingMode,
} from '@koharu/bridge/protocol'
import { Button } from '@koharu/ui/components/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from '@koharu/ui/components/dropdown-menu'
import {
  NumberField,
  NumberFieldDecrement,
  NumberFieldGroup,
  NumberFieldIncrement,
  NumberFieldInput,
} from '@koharu/ui/components/number-field'
import { Popover, PopoverContent, PopoverTrigger } from '@koharu/ui/components/popover'
import { ScrollArea } from '@koharu/ui/components/scroll-area'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@koharu/ui/components/select'
import { Slider } from '@koharu/ui/components/slider'
import { Tooltip, TooltipContent, TooltipTrigger } from '@koharu/ui/components/tooltip'

const defaultFont: FontFamily = {
  name: 'CCWildWords',
  metadata: {
    primary_script: 'latn',
    scripts: ['latn'],
    languages: ['en'],
    category: 'HANDWRITING',
    classifications: ['comic', 'dialogue'],
    use_cases: ['comic-dialogue', 'word-balloons'],
  },
  sources: ['bundled'],
  faces: [
    {
      postscript_name: 'CCWildWords-Regular',
      weight: 400,
      weight_range: null,
      style: 'normal',
    },
  ],
}

export function Inspector() {
  const { t } = useTranslation()
  return (
    <aside className='flex h-full min-h-0 flex-col bg-[var(--surface-panel)]'>
      <div className='flex h-8 shrink-0 items-center gap-1.5 border-b border-border/80 px-2'>
        <Type className='size-3 text-primary' />
        <h2 className='text-[10px] font-semibold'>{t('inspector.type')}</h2>
      </div>

      <section className='min-w-0 shrink-0 overflow-hidden border-b'>
        <TypeInspector />
      </section>

      <section className='flex min-h-0 flex-1 flex-col'>
        <LayersInspector />
      </section>
    </aside>
  )
}

function TypeInspector() {
  const { t } = useTranslation()
  const borderWidthId = useId()
  const page = usePage().data
  const selectedIds = useKoharuStore((state) => state.selectedLayers)
  const availableFonts = useFonts().data
  const expandedSelection = page ? expandLayerSelection(page.layers, selectedIds) : []
  const selected =
    page?.layers.filter(isTextLayer).filter((layer) => expandedSelection.includes(layer.id)) ?? []
  const current = selected[0]
  const [draft, setDraft] = useState<{
    layer: EntityId
    typography: Typography
  } | null>(null)
  const updateSequence = useRef(0)

  useEffect(() => setDraft(null), [current?.id])

  const apply = (update: (value: Typography) => Typography) => {
    if (!selected.length) return
    const updates = selected.map((layer) => ({
      layer: layer.id,
      typography: update(layer.typography ?? defaultTypography),
    }))
    const optimistic = current && updates.find(({ layer }) => layer === current.id)
    if (optimistic) setDraft(optimistic)
    const sequence = ++updateSequence.current
    void call(commands.setTypography, updates)
      .then(() => refresh(projectKey, pageKey))
      .catch(() => undefined)
      .finally(() => {
        if (updateSequence.current === sequence) setDraft(null)
      })
  }

  const typography =
    current && draft?.layer === current.id
      ? draft.typography
      : (current?.typography ?? defaultTypography)
  const disabled = !current
  const styleClipboard = useKoharuStore((state) => state.styleClipboard)
  const families = useMemo(() => {
    const available = availableFonts ?? []
    return available.some(
      (family) => normalizeFontName(family.name) === normalizeFontName(defaultFont.name),
    )
      ? available
      : [...available, defaultFont]
  }, [availableFonts])
  const size = Math.round((typography.size ?? 24) * 100) / 100
  const weight = typography.font_weight ?? 400
  const selectedFamily = findFontFamily(families, typography.preferred_font ?? defaultFont.name)
  const styles = usableFontStyles(selectedFamily)
  const style = styles.includes(typography.font_style ?? 'normal')
    ? (typography.font_style ?? 'normal')
    : (styles[0] ?? 'normal')
  const weights = usableFontWeights(selectedFamily, style)
  const strokeWidth = typography.stroke_width ?? 0
  const strokeColor = typography.stroke_color ?? defaultTypography.stroke_color!
  const strokeEnabled = strokeWidth > 0 && strokeColor[3] > 0
  const displayedStrokeWidth = strokeWidth > 0 ? strokeWidth : 1.5
  const writingMode = typography.writing_mode ?? 'Horizontal'
  const writingModeChoice = typography.writing_mode ?? 'Auto'
  const effectiveAlignment =
    typography.alignment ?? (writingMode === 'Vertical' ? 'Start' : 'Center')

  return (
    <div className='min-w-0 p-2' data-testid='type-inspector' aria-disabled={disabled}>
      <div className='grid min-w-0 gap-1.5'>
        <div className='flex gap-1.5'>
          <Button
            type='button'
            variant='outline'
            size='xs'
            disabled={disabled}
            onClick={() => page && copyStyle(page)}
          >
            <Copy />
            {t('inspector.copyStyle')}
          </Button>
          <Button
            type='button'
            variant='outline'
            size='xs'
            disabled={disabled || !styleClipboard}
            onClick={() => page && void pasteStyle(page).catch(() => undefined)}
          >
            <ClipboardPaste />
            {t('inspector.pasteStyle')}
          </Button>
        </div>
        <div className='grid min-w-0 grid-cols-[minmax(0,1fr)_2.5rem] gap-1.5'>
          <InspectorField label={t('inspector.font')}>
            <FontPicker
              value={typography.preferred_font ?? defaultFont.name}
              families={families}
              disabled={disabled}
              size='sm'
              onChange={(preferred_font) => {
                const family = findFontFamily(families, preferred_font)
                const nextStyles = usableFontStyles(family)
                const fontStyle = nextStyles.includes(style) ? style : (nextStyles[0] ?? 'normal')
                const nextWeights = usableFontWeights(family, fontStyle)
                const fontWeight = nearestFontWeight(nextWeights, weight)
                apply((value) => ({
                  ...value,
                  preferred_font,
                  font_weight: fontWeight,
                  font_style: fontStyle,
                }))
              }}
            />
          </InspectorField>
          <InspectorField label={t('inspector.color')}>
            <ColorWell
              label={t('inspector.textColor')}
              size='sm'
              disabled={disabled}
              value={rgbaToHex(typography.color ?? defaultTypography.color!)}
              onChange={(color) => apply((value) => ({ ...value, color: hexToRgba(color) }))}
            />
          </InspectorField>
        </div>

        <div className='grid min-w-0 grid-cols-[minmax(0,1fr)_4.25rem_4.75rem] gap-1.5'>
          <InspectorField label={t('inspector.size')}>
            <FontSizeField
              disabled={disabled}
              value={size}
              autoFit={typography.auto_fit}
              onChange={(next) => apply((value) => ({ ...value, size: next, auto_fit: false }))}
              onAutoFit={() =>
                apply((value) => ({
                  ...value,
                  size: value.size ?? size,
                  auto_fit: true,
                }))
              }
            />
          </InspectorField>
          <InspectorField label={t('inspector.weight')}>
            <Select
              disabled={disabled}
              value={String(weight)}
              onValueChange={(font_weight) =>
                apply((value) => ({
                  ...value,
                  font_weight: Number(font_weight),
                }))
              }
            >
              <SelectTrigger
                size='sm'
                aria-label={t('inspector.fontWeight')}
                className='w-full min-w-0'
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                {weights.map((value) => (
                  <SelectItem key={value} value={String(value)}>
                    {value}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </InspectorField>
          <InspectorField label={t('inspector.style')}>
            <Select
              disabled={disabled}
              value={style}
              onValueChange={(font_style) => {
                const nextStyle = font_style as FontStyle
                const nextWeights = usableFontWeights(selectedFamily, nextStyle)
                apply((value) => ({
                  ...value,
                  font_style: nextStyle,
                  font_weight: nearestFontWeight(nextWeights, weight),
                }))
              }}
            >
              <SelectTrigger
                size='sm'
                aria-label={t('inspector.fontStyle')}
                className='w-full min-w-0'
              >
                <SelectValue>{t(`inspector.fontStyles.${style}`)}</SelectValue>
              </SelectTrigger>
              <SelectContent align='end'>
                {styles.map((value) => (
                  <SelectItem key={value} value={value}>
                    {t(`inspector.fontStyles.${value}`)}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </InspectorField>
        </div>

        {typography.auto_fit && (
          <InspectorField label={t('inspector.fitScale')}>
            <NumberField
              name='fit-scale'
              className='min-w-0'
              disabled={disabled}
              value={Math.round((typography.fit_scale ?? 1) * 100)}
              min={50}
              max={300}
              step={5}
              onValueChange={(next) => {
                if (next !== null && next >= 50 && next <= 300) {
                  apply((value) => ({ ...value, fit_scale: next / 100 }))
                }
              }}
            >
              <NumberFieldGroup>
                <NumberFieldDecrement aria-label={t('inspector.decreaseFitScale')}>
                  <Minus />
                </NumberFieldDecrement>
                <NumberFieldInput data-testid='fit-scale' aria-label={t('inspector.fitScale')} />
                <NumberFieldIncrement aria-label={t('inspector.increaseFitScale')}>
                  <Plus />
                </NumberFieldIncrement>
              </NumberFieldGroup>
            </NumberField>
          </InspectorField>
        )}

        <div className='grid min-w-0 grid-cols-[minmax(0,1fr)_minmax(6.5rem,1fr)] gap-1.5'>
          <InspectorField label={t('inspector.alignment')}>
            <div className='grid h-6 grid-cols-3 rounded-md border border-input bg-background p-px'>
              {(
                [
                  [
                    'Start',
                    AlignLeft,
                    writingMode === 'Vertical' ? t('inspector.alignTop') : t('inspector.alignLeft'),
                  ],
                  [
                    'Center',
                    AlignCenter,
                    writingMode === 'Vertical'
                      ? t('inspector.alignMiddle')
                      : t('inspector.alignCenter'),
                  ],
                  [
                    'End',
                    AlignRight,
                    writingMode === 'Vertical'
                      ? t('inspector.alignBottom')
                      : t('inspector.alignRight'),
                  ],
                ] as const
              ).map(([alignment, Icon, label]) => (
                <button
                  key={alignment}
                  type='button'
                  aria-label={label}
                  aria-pressed={effectiveAlignment === alignment}
                  disabled={disabled}
                  data-active={effectiveAlignment === alignment}
                  className='grid place-items-center rounded-[4px] text-muted-foreground hover:text-foreground disabled:cursor-not-allowed disabled:opacity-40 data-[active=true]:bg-primary data-[active=true]:text-primary-foreground data-[active=true]:hover:bg-primary/90'
                  onClick={() =>
                    apply((value) => ({
                      ...value,
                      alignment: alignment as TextAlignment,
                    }))
                  }
                >
                  <Icon className='size-3' />
                </button>
              ))}
            </div>
          </InspectorField>
          <InspectorField label={t('inspector.direction')}>
            <Select
              disabled={disabled}
              value={writingModeChoice}
              onValueChange={(writing_mode) =>
                apply((value) => ({
                  ...value,
                  writing_mode: writing_mode === 'Auto' ? null : (writing_mode as WritingMode),
                }))
              }
            >
              <SelectTrigger size='sm' aria-label={t('inspector.textDirection')} className='w-full'>
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value='Auto'>{t('inspector.auto')}</SelectItem>
                <SelectItem value='Horizontal'>{t('inspector.horizontal')}</SelectItem>
                <SelectItem value='Vertical'>{t('inspector.vertical')}</SelectItem>
              </SelectContent>
            </Select>
          </InspectorField>
        </div>

        <div className='grid min-w-0 grid-cols-[2.75rem_minmax(5.5rem,1fr)] items-end gap-1.5'>
          <InspectorField label={t('inspector.border')}>
            <ColorWell
              label={t('inspector.borderColor')}
              size='sm'
              disabled={disabled}
              allowTransparent
              value={strokeEnabled ? rgbaToHex(strokeColor) : null}
              onChange={(stroke_color) => {
                apply((value) => {
                  const width = value.stroke_width ?? 0
                  if (stroke_color !== null) {
                    return {
                      ...value,
                      stroke_color: hexToRgba(stroke_color),
                      stroke_width: width > 0 ? width : 1.5,
                    }
                  }
                  const [red, green, blue] = value.stroke_color ?? defaultTypography.stroke_color!
                  return {
                    ...value,
                    stroke_color: [red, green, blue, 0],
                    stroke_width: width > 0 ? width : 1.5,
                  }
                })
              }}
            />
          </InspectorField>
          <InspectorField label={t('inspector.width')}>
            <NumberField
              id={borderWidthId}
              name='border-width'
              className='min-w-0'
              disabled={disabled}
              value={displayedStrokeWidth}
              min={0.5}
              max={32}
              step={0.5}
              onValueChange={(next) => {
                if (next !== null && next >= 0.5 && next <= 32) {
                  apply((value) => ({ ...value, stroke_width: next }))
                }
              }}
            >
              <NumberFieldGroup>
                <NumberFieldDecrement aria-label={t('inspector.decreaseBorderWidth')}>
                  <Minus />
                </NumberFieldDecrement>
                <NumberFieldInput aria-label={t('inspector.borderWidth')} />
                <NumberFieldIncrement aria-label={t('inspector.increaseBorderWidth')}>
                  <Plus />
                </NumberFieldIncrement>
              </NumberFieldGroup>
            </NumberField>
          </InspectorField>
        </div>
      </div>
    </div>
  )
}

function findFontFamily(families: FontFamily[], name: string): FontFamily | undefined {
  return families.find((family) => normalizeFontName(family.name) === normalizeFontName(name))
}

function usableFontStyles(family: FontFamily | undefined): FontStyle[] {
  if (!family) return ['normal']
  const styles = new Set(family.faces.map((font) => font.style))
  const available = (['normal', 'italic', 'oblique'] satisfies FontStyle[]).filter((style) =>
    styles.has(style),
  )
  return available.length ? available : ['normal']
}

function usableFontWeights(family: FontFamily | undefined, style: FontStyle): number[] {
  if (!family) return [400]
  const styled = family.faces.filter((font) => font.style === style)
  const faces = styled.length ? styled : family.faces
  const weights = new Set(faces.map((font) => font.weight))
  for (const face of faces) {
    if (!face.weight_range) continue
    weights.add(face.weight_range.minimum)
    weights.add(face.weight_range.maximum)
    for (let weight = 100; weight <= 900; weight += 100) {
      if (weight >= face.weight_range.minimum && weight <= face.weight_range.maximum) {
        weights.add(weight)
      }
    }
  }
  return [...weights].sort((left, right) => left - right)
}

function nearestFontWeight(weights: number[], target: number): number {
  return weights.reduce((nearest, weight) =>
    Math.abs(weight - target) < Math.abs(nearest - target) ? weight : nearest,
  )
}

function normalizeFontName(value: string): string {
  return value.normalize('NFKC').trim().replace(/\s+/g, ' ').toLowerCase()
}

function displayedLayers(layers: Layer[], page: EntityId) {
  const indexes = new Map(layers.map((layer, index) => [layer.id, index]))
  const rows: { layer: Layer; index: number; depth: number }[] = []
  const append = (layer: Layer, depth: number) => {
    rows.push({ layer, index: indexes.get(layer.id) ?? 0, depth })
    if (!isGroupLayer(layer)) return
    const children = layerChildren(layers, layer.id)
    const ordered = layer.role === 'text' ? children : [...children].reverse()
    for (const child of ordered) append(child, depth + 1)
  }
  const roots = layers.filter((layer) => (layer.parent ?? page) === page).reverse()
  for (const layer of roots) append(layer, 0)
  return rows
}

function LayersInspector() {
  const { t } = useTranslation()
  const page = usePage().data
  const selected = useKoharuStore((state) => state.selectedLayers)
  const selectLayers = useKoharuStore((state) => state.selectLayers)
  const [expandedLayer, setExpandedLayer] = useState<EntityId | null>(
    selected.length === 1 ? (selected[0] ?? null) : null,
  )
  const [movingLayer, setMovingLayer] = useState<EntityId | null>(null)
  const [dragged, setDragged] = useState<EntityId | null>(null)
  const anchor = useRef<EntityId | null>(null)

  useEffect(() => {
    setExpandedLayer(selected.length === 1 ? (selected[0] ?? null) : null)
  }, [selected])

  const layers = useMemo(() => (page ? displayedLayers(page.layers, page.id) : []), [page])

  if (!page) return <EmptyInspector>{t('inspector.selectPage')}</EmptyInspector>

  // Unlocked siblings in stored order, and in the order the panel shows them.
  const siblingsOf = (layer: Layer) => {
    const parent = layer.parent ?? page.id
    const stored = page.layers.filter(
      (candidate) => !isLockedLayer(candidate) && (candidate.parent ?? page.id) === parent,
    )
    const parentLayer = page.layers.find((candidate) => candidate.id === layer.parent)
    const shown =
      parentLayer && isGroupLayer(parentLayer) && parentLayer.role === 'text'
        ? stored
        : [...stored].reverse()
    return { parent, stored, shown }
  }

  const moveTo = (layer: Layer, target: Layer) => {
    if (movingLayer !== null || layer.id === target.id) return
    if (isLockedLayer(layer) || isLockedLayer(target)) return
    const { parent, stored } = siblingsOf(layer)
    const index = stored.findIndex((candidate) => candidate.id === target.id)
    if (index < 0) return

    setMovingLayer(layer.id)
    void call(commands.moveLayer, layer.id, parent, index).then(
      (next) => {
        queryClient.setQueryData(pageKey, next)
        setMovingLayer(null)
        void refresh(projectKey)
      },
      () => setMovingLayer(null),
    )
  }

  const move = (layer: Layer, displayDelta: number) => {
    const { shown } = siblingsOf(layer)
    const target = shown[shown.findIndex((candidate) => candidate.id === layer.id) + displayDelta]
    if (target) moveTo(layer, target)
  }

  const deleteLayer = (layer: EntityId) =>
    void call(commands.deleteLayers, [layer])
      .then(() => {
        if (selected.includes(layer)) {
          selectLayers(selected.filter((selectedLayer) => selectedLayer !== layer))
        }
        setExpandedLayer((current) => (current === layer ? null : current))
        return refresh(projectKey, pageKey)
      })
      .catch(() => undefined)

  // Modifier semantics mirror the page rail: ctrl/meta toggles, shift selects a display range.
  const selectLayer = (layer: EntityId, additive: boolean, range: boolean) => {
    if (!additive && !range) {
      anchor.current = layer
      if (selected.length === 1 && selected[0] === layer) {
        setExpandedLayer((current) => (current === layer ? null : layer))
        return
      }
      selectLayers([layer])
      setExpandedLayer(layer)
      return
    }
    const order = layers.map((row) => row.layer.id)
    const anchorIndex = anchor.current ? order.indexOf(anchor.current) : -1
    if (range && anchorIndex >= 0) {
      const targetIndex = order.indexOf(layer)
      const span = layers
        .slice(Math.min(anchorIndex, targetIndex), Math.max(anchorIndex, targetIndex) + 1)
        .filter((row) => !isLockedLayer(row.layer))
        .map((row) => row.layer.id)
      selectLayers(additive ? [...selected, ...span] : span)
      return
    }
    anchor.current = layer
    if (!additive) {
      selectLayers([layer])
      return
    }
    selectLayers(
      selected.includes(layer)
        ? selected.filter((selectedLayer) => selectedLayer !== layer)
        : [...selected, layer],
    )
  }

  return (
    <div className='flex min-h-0 flex-1 flex-col'>
      <header className='flex h-8 shrink-0 items-center gap-1.5 border-b border-border/80 px-2'>
        <Layers3 className='size-3 text-primary' />
        <h2 className='text-[10px] font-semibold'>{t('layers.title')}</h2>
        <span className='text-[9px] text-muted-foreground tabular-nums'>
          {page.layers.filter((layer) => !isGroupLayer(layer)).length}
        </span>
      </header>

      <div className='shrink-0 border-b border-border/80 px-2 py-1.5'>
        <InspectorField label={t('inspector.sceneDescription')}>
          <LockToggle
            id={page.id}
            locked={page.description_locked}
            lockLabel={t('inspector.lockDescription')}
            unlockLabel={t('inspector.unlockDescription')}
            onChanged={() => refresh(projectKey, pageKey)}
          />
          <CommitTextarea
            key={page.id}
            aria-label={t('inspector.sceneDescription')}
            placeholder={t('inspector.sceneDescriptionPlaceholder')}
            wrap='soft'
            className='max-h-24 min-h-9 w-full max-w-full min-w-0 resize-y overflow-y-auto rounded-md bg-background px-1.5 py-1 text-[12px] leading-4 md:text-[12px]'
            value={page.description ?? ''}
            onCommit={(text) =>
              void call(commands.setSceneDescription, page.id, text)
                .then(() => refresh(projectKey, pageKey))
                .catch(() => undefined)
            }
          />
        </InspectorField>
      </div>

      <ScrollArea className='min-h-0 flex-1'>
        <div className='py-0.5'>
          {layers.map(({ layer, index, depth }) => {
            const locked = isLockedLayer(layer)
            const { shown: siblings } = siblingsOf(layer)
            const position = siblings.findIndex((candidate) => candidate.id === layer.id)
            return (
              <LayerRow
                key={`${layer.type}:${layer.id}`}
                layer={layer}
                index={index}
                depth={depth}
                selected={selected.includes(layer.id)}
                expanded={!locked && expandedLayer === layer.id}
                locked={locked}
                onSelect={(event) =>
                  selectLayer(layer.id, event.ctrlKey || event.metaKey, event.shiftKey)
                }
                onToggle={() =>
                  void call(commands.setVisibility, [layer.id], !layer.visibility.visible, null)
                    .then(() => refresh(projectKey, pageKey))
                    .catch(() => undefined)
                }
                onMove={(delta) => move(layer, delta)}
                canMoveUp={!locked && position > 0}
                canMoveDown={!locked && position >= 0 && position < siblings.length - 1}
                reordering={movingLayer !== null}
                dragged={dragged === layer.id}
                onDragStart={() => setDragged(layer.id)}
                onDragEnd={() => setDragged(null)}
                onDrop={() => {
                  const source = page.layers.find((candidate) => candidate.id === dragged)
                  if (source) moveTo(source, layer)
                  setDragged(null)
                }}
                onDelete={isGroupLayer(layer) ? undefined : () => deleteLayer(layer.id)}
              />
            )
          })}
          {layers.length === 0 && <EmptyInspector>{t('layers.empty')}</EmptyInspector>}
        </div>
      </ScrollArea>
    </div>
  )
}

function LayerRow({
  layer,
  index,
  depth,
  selected,
  expanded,
  locked,
  onSelect,
  onToggle,
  onMove,
  canMoveUp,
  canMoveDown,
  reordering,
  dragged,
  onDragStart,
  onDragEnd,
  onDrop,
  onDelete,
}: {
  layer: Layer
  index: number
  depth: number
  selected: boolean
  expanded: boolean
  locked: boolean
  onSelect: (event: MouseEvent<HTMLButtonElement>) => void
  onToggle: () => void
  onMove: (delta: number) => void
  canMoveUp: boolean
  canMoveDown: boolean
  reordering: boolean
  dragged: boolean
  onDragStart: () => void
  onDragEnd: () => void
  onDrop: () => void
  onDelete?: () => void
}) {
  const { t } = useTranslation()
  const name = localizedLayerName(layer, index, t)
  const detail = localizedLayerKind(layer, t)
  const Icon = layerIcon(layer)
  return (
    <div
      draggable={!locked}
      className={`group min-w-0 px-1 py-px ${dragged ? 'opacity-50' : ''}`}
      style={{ paddingLeft: `${depth * 10 + 4}px` }}
      onDragStart={onDragStart}
      onDragEnd={onDragEnd}
      onDragOver={(event) => event.preventDefault()}
      onDrop={(event) => {
        event.preventDefault()
        onDrop()
      }}
    >
      <div
        data-selected={selected}
        data-expanded={expanded}
        className='min-w-0 overflow-hidden rounded-lg transition-colors duration-150 data-[selected=true]:bg-accent motion-reduce:transition-none'
      >
        <div className='relative flex min-w-0 items-center gap-0.5'>
          <button
            type='button'
            aria-label={t('layers.edit', { name })}
            aria-expanded={locked ? undefined : expanded}
            disabled={locked}
            className='flex min-w-0 flex-1 items-center gap-1.5 rounded-lg px-1.5 py-1 text-left hover:bg-foreground/[0.05] focus-visible:ring-2 focus-visible:ring-ring/25'
            onClick={onSelect}
          >
            <Icon className='size-3.5 shrink-0 text-muted-foreground' />
            <span className='min-w-0 flex-1'>
              <span className='block truncate text-[11px] font-medium'>{name}</span>
              <span className='block truncate text-[9px] leading-3 text-muted-foreground capitalize'>
                {detail}
              </span>
            </span>
          </button>
          {!locked && (
            <div
              className={`pointer-events-none absolute top-1/2 z-10 flex -translate-y-1/2 rounded-md bg-background/80 p-0.5 opacity-0 shadow-sm ring-1 ring-border/40 backdrop-blur-md transition-opacity duration-150 group-hover:pointer-events-auto group-hover:opacity-100 focus-within:pointer-events-auto focus-within:opacity-100 motion-reduce:transition-none ${expanded ? 'right-7' : 'right-[3.25rem]'}`}
            >
              <button
                type='button'
                aria-label={t('layers.moveUp', { name })}
                disabled={reordering || !canMoveUp}
                className='grid size-5 place-items-center rounded-sm text-muted-foreground hover:bg-foreground/[0.07] hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/25 disabled:pointer-events-none disabled:opacity-30'
                onClick={() => onMove(-1)}
              >
                <ArrowUp className='size-3' />
              </button>
              <button
                type='button'
                aria-label={t('layers.moveDown', { name })}
                disabled={reordering || !canMoveDown}
                className='grid size-5 place-items-center rounded-sm text-muted-foreground hover:bg-foreground/[0.07] hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/25 disabled:pointer-events-none disabled:opacity-30'
                onClick={() => onMove(1)}
              >
                <ArrowDown className='size-3' />
              </button>
            </div>
          )}
          {!expanded && (
            <span className='w-7 shrink-0 text-right text-[9px] text-muted-foreground tabular-nums'>
              {Math.round(layer.visibility.opacity * 100)}%
            </span>
          )}
          {locked ? (
            <Tooltip>
              <TooltipTrigger
                render={
                  <span
                    role='img'
                    className='grid size-6 shrink-0 place-items-center text-muted-foreground'
                    aria-label={t('layers.lockedLabel', { name })}
                  />
                }
              >
                <Lock className='size-3.5' />
              </TooltipTrigger>
              <TooltipContent side='left'>{t('layers.locked')}</TooltipContent>
            </Tooltip>
          ) : (
            <button
              type='button'
              aria-label={
                layer.visibility.visible ? t('layers.hide', { name }) : t('layers.show', { name })
              }
              className='grid size-6 shrink-0 place-items-center rounded-md text-muted-foreground hover:bg-foreground/[0.06] hover:text-foreground focus-visible:ring-2 focus-visible:ring-ring/25'
              onClick={onToggle}
            >
              {layer.visibility.visible ? (
                <Eye className='size-3.5' />
              ) : (
                <EyeOff className='size-3.5' />
              )}
            </button>
          )}
        </div>
        {expanded && (
          <div className='animate-in duration-150 fade-in slide-in-from-top-1 motion-reduce:animate-none'>
            <LayerEditor layer={layer} onDelete={onDelete} />
          </div>
        )}
      </div>
    </div>
  )
}

function LayerEditor({ layer, onDelete }: { layer: Layer; onDelete?: () => void }) {
  const { t } = useTranslation()
  const name = localizedLayerName(layer, 0, t)
  const [opacity, setOpacity] = useState(layer.visibility.opacity * 100)

  useEffect(() => {
    setOpacity(layer.visibility.opacity * 100)
  }, [layer.id, layer.visibility.opacity])

  const commitOpacity = (next: number) => {
    void call(commands.setVisibility, [layer.id], null, next / 100)
      .then(() => refresh(projectKey, pageKey))
      .catch(() => {
        setOpacity(layer.visibility.opacity * 100)
        previewCanvasOpacity(layer.id, null)
      })
  }

  const previewOpacity = (next: number) => {
    setOpacity(next)
    previewCanvasOpacity(layer.id, next / 100)
  }

  const resetTextFrame = () => {
    if (!isTextLayer(layer) || !layer.automatic_region || !layer.geometry) return
    void call(commands.setGeometry, [{ layer: layer.id, points: null }])
      .then(() => refresh(projectKey, pageKey))
      .catch(() => undefined)
  }

  return (
    <div className='grid min-w-0 gap-1.5 px-1.5 pt-0.5 pb-1.5'>
      <div className='flex min-w-0 items-center gap-1.5'>
        <span className='shrink-0 text-[8px] font-medium text-muted-foreground uppercase'>
          {t('inspector.opacity')}
        </span>
        <div className='flex min-w-0 flex-1 items-center gap-1.5'>
          <Slider
            aria-label={t('layers.opacityLabel', { name })}
            min={0}
            max={100}
            step={1}
            value={opacity}
            className='[&_[data-slot=slider-thumb]]:size-2'
            onValueChange={previewOpacity}
            onValueCommitted={commitOpacity}
          />
          <span className='w-7 shrink-0 text-right text-[8px] text-muted-foreground tabular-nums'>
            {Math.round(opacity)}%
          </span>
        </div>
        {onDelete && (
          <Tooltip>
            <TooltipTrigger
              render={
                <Button
                  type='button'
                  variant='ghost'
                  size='icon-xs'
                  aria-label={t('layers.delete', { name })}
                  className='size-5 rounded-md text-muted-foreground hover:text-foreground'
                  onClick={onDelete}
                />
              }
            >
              <Trash2 className='size-3' />
            </TooltipTrigger>
            <TooltipContent side='left'>{t('layers.delete', { name })}</TooltipContent>
          </Tooltip>
        )}
      </div>
      {isTextLayer(layer) && (
        <>
          <div className='flex h-5 min-w-0 items-center gap-1.5'>
            <span className='text-[8px] font-medium text-muted-foreground uppercase'>
              {t('inspector.placement')}
            </span>
            <span className='rounded-md bg-foreground/[0.055] px-1.5 py-0.5 text-[9px] leading-none text-foreground/75'>
              {layer.geometry
                ? t('inspector.customFrame')
                : layer.automatic_region
                  ? t('inspector.autoFit')
                  : t('inspector.unplaced')}
            </span>
            {layer.geometry && layer.automatic_region && (
              <Tooltip>
                <TooltipTrigger
                  render={
                    <Button
                      type='button'
                      variant='ghost'
                      size='xs'
                      aria-label={t('inspector.resetAutoFit')}
                      className='ml-auto h-5 gap-1 rounded-md px-1.5 text-[9px] font-normal text-muted-foreground hover:text-foreground'
                      onClick={resetTextFrame}
                    />
                  }
                >
                  <RotateCcw className='size-3' />
                  {t('common.reset')}
                </TooltipTrigger>
                <TooltipContent side='left'>{t('inspector.resetAutoFit')}</TooltipContent>
              </Tooltip>
            )}
          </div>
          <InspectorField label={t('inspector.source')}>
            <CommitTextarea
              data-testid={`edit-source-${layer.id}`}
              aria-label={t('layers.sourceLabel', { name })}
              wrap='soft'
              className='max-h-14 min-h-8 w-full max-w-full min-w-0 resize-y overflow-y-auto rounded-md bg-background px-1.5 py-1 text-[12px] leading-4 md:text-[12px]'
              value={layer.content.source?.text ?? ''}
              onCommit={(text) =>
                void call(commands.setSourceText, layer.id, text)
                  .then(() => refresh(projectKey, pageKey))
                  .catch(() => undefined)
              }
            />
          </InspectorField>
          <InspectorField label={t('inspector.translation')}>
            <CommitTextarea
              data-testid={`edit-translation-${layer.id}`}
              aria-label={t('layers.translationLabel', { name })}
              wrap='soft'
              className='max-h-16 min-h-9 w-full max-w-full min-w-0 resize-y overflow-y-auto rounded-md border-primary/25 bg-background px-1.5 py-1 text-[12px] leading-4 md:text-[12px]'
              value={layer.content.translation?.text ?? ''}
              onCommit={(text) =>
                void call(commands.setTranslation, layer.id, text.trim() ? text : null)
                  .then(() => refresh(projectKey, pageKey))
                  .catch(() => undefined)
              }
            />
            <div className='flex items-center justify-end gap-0.5'>
              <LockToggle
                id={layer.id}
                locked={layer.locked}
                lockLabel={t('inspector.lockTranslation')}
                unlockLabel={t('inspector.unlockTranslation')}
                onChanged={() => refresh(projectKey, pageKey)}
              />
            </div>
            <TranslateBox id={layer.id} locked={layer.locked} />
          </InspectorField>
        </>
      )}
    </div>
  )
}

// Translates one box through the entity scope. The note is one-shot: it lives only while the
// popover is open and is never saved. The explanation arrives on the job and is kept in the store.
function TranslateBox({ id, locked }: { id: string; locked: boolean }) {
  const { t } = useTranslation()
  const [open, setOpen] = useState(false)
  const [note, setNote] = useState('')
  const explanation = useKoharuStore((state) => state.explanations[id])
  const run = (text: string) =>
    call(
      commands.process,
      { scope: 'entities', value: [id] },
      { operation: 'translate_box', note: text.trim() || null },
    )
      .then((job) => useKoharuStore.getState().expectExplanation(job, id))
      .catch(() => undefined)

  return (
    <>
      <div className='flex justify-end gap-0.5'>
        <Popover
          open={open}
          onOpenChange={(next) => {
            setOpen(next)
            if (!next) setNote('')
          }}
        >
          <PopoverTrigger
            disabled={locked}
            title={locked ? t('inspector.unlockToTranslate') : undefined}
            data-testid={`translate-note-${id}`}
            className='h-5 rounded-md px-1.5 text-[9px] text-muted-foreground hover:text-foreground'
          >
            {t('inspector.translateWithNote')}
          </PopoverTrigger>
          <PopoverContent className='grid w-64 gap-1.5 p-2'>
            <textarea
              data-testid={`translate-note-input-${id}`}
              aria-label={t('inspector.translateNote')}
              placeholder={t('inspector.translateNotePlaceholder')}
              value={note}
              onChange={(event) => setNote(event.target.value)}
              className='min-h-16 w-full resize-y rounded-md border bg-background px-1.5 py-1 text-[12px] leading-4'
            />
            <Button
              type='button'
              size='xs'
              data-testid={`translate-note-run-${id}`}
              className='justify-self-end'
              onClick={() => void run(note)}
            >
              {t('inference.translateOnly')}
            </Button>
          </PopoverContent>
        </Popover>
        <Button
          type='button'
          variant='ghost'
          size='xs'
          disabled={locked}
          title={locked ? t('inspector.unlockToTranslate') : undefined}
          data-testid={`translate-box-${id}`}
          className='h-5 rounded-md px-1.5 text-[9px] font-normal text-muted-foreground hover:text-foreground'
          onClick={() => void run('')}
        >
          {t('inference.translateOnly')}
        </Button>
      </div>
      {explanation && (
        <div
          data-testid={`explanation-${id}`}
          className='grid gap-1 rounded-md border bg-muted/40 p-1.5 text-[11px] leading-4'
        >
          <p>{explanation}</p>
          <Button
            type='button'
            variant='ghost'
            size='xs'
            data-testid={`explanation-dismiss-${id}`}
            className='h-5 justify-self-end px-1.5 text-[9px]'
            onClick={() => useKoharuStore.getState().dismissExplanation(id)}
          >
            {t('inspector.dismissExplanation')}
          </Button>
        </div>
      )}
    </>
  )
}

// Locks a box's translation or a page's description against Translate: one backend commit
// (and so one undo step) per toggle.
function LockToggle({
  id,
  locked,
  lockLabel,
  unlockLabel,
  onChanged,
}: {
  id: string
  locked: boolean
  lockLabel: string
  unlockLabel: string
  onChanged: () => unknown
}) {
  const label = locked ? unlockLabel : lockLabel
  return (
    <Button
      type='button'
      variant='ghost'
      size='xs'
      data-testid={`lock-${id}`}
      aria-label={label}
      aria-pressed={locked}
      title={label}
      className={`h-5 w-5 justify-self-end rounded-md p-0 ${locked ? 'text-primary' : 'text-muted-foreground'}`}
      onClick={() =>
        void call(commands.setLocked, id, !locked)
          .then(onChanged)
          .catch(() => undefined)
      }
    >
      {locked ? <Lock className='size-3' /> : <LockOpen className='size-3' />}
    </Button>
  )
}

function InspectorField({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className='grid min-w-0 gap-0.5'>
      <span className='text-[8px] font-medium tracking-[0.06em] text-muted-foreground uppercase'>
        {label}
      </span>
      {children}
    </div>
  )
}

const fontSizePresets = [8, 9, 10, 12, 14, 16, 18, 20, 24, 28, 32, 36, 48, 64, 72, 96]

function FontSizeField({
  value,
  autoFit,
  disabled,
  onChange,
  onAutoFit,
}: {
  value: number
  autoFit: boolean
  disabled: boolean
  onChange: (value: number) => void
  onAutoFit: () => void
}) {
  const { t } = useTranslation()
  const id = useId()
  const [draft, setDraft] = useState<number | null>(value)

  useEffect(() => {
    setDraft(autoFit ? null : value)
  }, [autoFit, value])

  const select = (choice: string) => {
    if (choice === 'auto') {
      setDraft(null)
      onAutoFit()
      return
    }
    const size = Number(choice)
    if (Number.isFinite(size) && size > 0 && size <= 300) {
      setDraft(size)
      onChange(size)
    }
  }

  const commit = (next: number | null) => {
    if (next !== null && next > 0 && next <= 300) {
      onChange(next)
    } else {
      setDraft(autoFit ? null : value)
    }
  }

  return (
    <NumberField
      id={id}
      name='font-size'
      className='min-w-0'
      disabled={disabled}
      value={draft}
      min={1}
      max={300}
      step={1}
      onValueChange={setDraft}
      onValueCommitted={commit}
    >
      <NumberFieldGroup>
        <NumberFieldDecrement aria-label={t('inspector.decreaseFontSize')}>
          <Minus />
        </NumberFieldDecrement>
        <NumberFieldInput
          data-testid='type-size'
          aria-label={t('inspector.fontSize')}
          placeholder={t('inspector.auto')}
          className='px-2 text-left'
        />
        <NumberFieldIncrement aria-label={t('inspector.increaseFontSize')}>
          <Plus />
        </NumberFieldIncrement>
        <DropdownMenu>
          <DropdownMenuTrigger
            disabled={disabled}
            aria-label={t('inspector.chooseFontSize')}
            className='grid size-6 shrink-0 place-items-center border-l border-input text-muted-foreground outline-none hover:bg-muted hover:text-foreground focus-visible:bg-muted focus-visible:text-foreground disabled:pointer-events-none disabled:opacity-40'
          >
            <ChevronDown className='size-3' />
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align='end'
            className='w-28 min-w-28 border border-border/50 p-0.5 shadow-sm ring-0'
          >
            <DropdownMenuRadioGroup value={autoFit ? 'auto' : String(value)} onValueChange={select}>
              <DropdownMenuRadioItem value='auto' className='min-h-6 py-0.5 text-[10px]'>
                {t('inspector.auto')}
              </DropdownMenuRadioItem>
              {fontSizePresets.map((size) => (
                <DropdownMenuRadioItem
                  key={size}
                  value={String(size)}
                  className='min-h-6 py-0.5 text-[10px] tabular-nums'
                >
                  {size}
                </DropdownMenuRadioItem>
              ))}
            </DropdownMenuRadioGroup>
          </DropdownMenuContent>
        </DropdownMenu>
      </NumberFieldGroup>
    </NumberField>
  )
}

function EmptyInspector({ children }: { children: React.ReactNode }) {
  return (
    <div className='px-4 py-8 text-center text-[10px] leading-4 text-muted-foreground'>
      {children}
    </div>
  )
}

function rgbaToHex([red, green, blue]: [number, number, number, number]): string {
  return `#${[red, green, blue]
    .map((channel) => channel.toString(16).padStart(2, '0'))
    .join('')}`.toUpperCase()
}

function hexToRgba(hex: string): [number, number, number, number] {
  const value = hex.replace('#', '')
  return [
    Number.parseInt(value.slice(0, 2), 16),
    Number.parseInt(value.slice(2, 4), 16),
    Number.parseInt(value.slice(4, 6), 16),
    255,
  ]
}

function layerIcon(layer: Layer): typeof Type {
  if (layer.type === 'group') return Folder
  if (layer.type === 'raster') return Brush
  if (layer.type === 'text') return Type
  return ImageIcon
}

function localizedLayerName(layer: Layer, index: number, t: TFunction): string {
  if (layer.type === 'group' || layer.type === 'raster') return layer.name
  if (layer.type === 'text') {
    const text = layer.content.translation?.text || layer.content.source?.text
    return text?.trim() || t('layers.textName', { index: index + 1 })
  }
  if (layer.type === 'artwork') return t('layers.originalArtwork')
  return t('layers.imageName', { index: index + 1 })
}

function localizedLayerKind(layer: Layer, t: TFunction): string {
  if (layer.type === 'group') {
    return t(layer.role === 'text' ? 'layers.kinds.textGroup' : 'layers.kinds.group')
  }
  if (layer.type === 'raster') {
    return t(`layers.kinds.${layer.kind}`, { defaultValue: layer.kind })
  }
  if (layer.type === 'text') {
    const role = layer.content.role?.split('.').at(-1)
    if (role === 'dialogue') return t('layers.kinds.dialogue')
    if (role === 'free-text') return t('layers.kinds.freeText')
    return t('layers.kinds.text')
  }
  return t('layers.kinds.image')
}
