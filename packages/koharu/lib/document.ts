import type { Layer, Typography } from '@koharu/bridge/protocol'

export const defaultTypography: Typography = {
  preferred_font: null,
  font_weight: 400,
  font_style: 'normal',
  size: null,
  auto_fit: true,
  color: [0, 0, 0, 255],
  stroke_color: [255, 255, 255, 255],
  stroke_width: 0,
  alignment: null,
  writing_mode: null,
}

const clampStep = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value))

/** Cmd+Up/Down: auto-fit boxes step the fit scale by 5% (Shift: 25%); fixed ones the size by 1 (Shift: 10). */
export function stepTypography(typography: Typography, dir: number, big: boolean): Typography {
  if (typography.auto_fit) {
    const percent = (typography.fit_scale ?? 1) * 100 + dir * (big ? 25 : 5)
    return { ...typography, fit_scale: Math.round(clampStep(percent, 50, 300)) / 100 }
  }
  return { ...typography, size: clampStep((typography.size ?? 24) + dir * (big ? 10 : 1), 1, 300) }
}

export function isTextLayer(layer: Layer): layer is Extract<Layer, { type: 'text' }> {
  return layer.type === 'text'
}

export function isGroupLayer(layer: Layer): layer is Extract<Layer, { type: 'group' }> {
  return layer.type === 'group'
}

export function isLockedLayer(layer: Layer): boolean {
  return layer.type === 'artwork'
}

export function layerChildren(layers: Layer[], parent: string): Layer[] {
  return layers.filter((layer) => layer.parent === parent)
}

export function expandLayerSelection(layers: Layer[], selected: string[]): string[] {
  const result: string[] = []
  const visit = (id: string) => {
    const layer = layers.find((candidate) => candidate.id === id)
    if (!layer) return
    if (isGroupLayer(layer)) {
      for (const child of layerChildren(layers, id)) visit(child.id)
    } else if (!result.includes(id)) {
      result.push(id)
    }
  }
  for (const id of selected) visit(id)
  return result
}

export function effectiveLayerVisibility(layers: Layer[], layer: Layer) {
  let visible = layer.visibility.visible
  let opacity = layer.visibility.opacity
  let parent = layer.parent
  while (parent) {
    const group = layers.find((candidate) => candidate.id === parent)
    if (!group || !isGroupLayer(group)) break
    visible &&= group.visibility.visible
    opacity *= group.visibility.opacity
    parent = group.parent
  }
  return { visible, opacity }
}
