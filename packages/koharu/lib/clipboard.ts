import { call } from '@/lib/backend'
import { defaultTypography, expandLayerSelection, isTextLayer } from '@/lib/document'
import { controlFrame } from '@/lib/geometry'
import { pageKey, pagesKey, projectKey, refresh } from '@/lib/queries'
import { useKoharuStore, type CopiedBox } from '@/lib/store'
import { commands, type Page } from '@koharu/bridge/protocol'

export const PASTE_OFFSET = 10

function selectedText(page: Page) {
  const { selectedLayers } = useKoharuStore.getState()
  const ids = expandLayerSelection(page.layers, selectedLayers)
  return page.layers.filter(isTextLayer).filter((layer) => ids.includes(layer.id))
}

export function copyBoxes(page: Page) {
  const { layerFrames } = useKoharuStore.getState()
  const boxes = selectedText(page).flatMap((layer): CopiedBox[] => {
    const frame = controlFrame(layer, layerFrames)
    return frame
      ? [
          {
            source: layer.content.source?.text ?? null,
            translation: layer.content.translation?.text ?? null,
            typography: layer.typography,
            frame,
          },
        ]
      : []
  })
  if (boxes.length) useKoharuStore.setState({ boxClipboard: { page: page.id, boxes } })
}

// ponytail: a paste is several backend commands, so undo takes a few steps per box; add a
// single create-with-content command if that proves annoying.
export async function pasteBoxes(page: Page) {
  const { boxClipboard } = useKoharuStore.getState()
  if (!boxClipboard) return
  const offset = boxClipboard.page === page.id ? PASTE_OFFSET : 0
  const created: string[] = []
  try {
    for (const box of boxClipboard.boxes) {
      const { layer } = await call(commands.addTextBox, {
        ...box.frame,
        x: box.frame.x + offset,
        y: box.frame.y + offset,
      })
      created.push(layer)
      if (box.source) await call(commands.setSourceText, layer, box.source)
      if (box.translation) await call(commands.setTranslation, layer, box.translation)
      if (box.typography) {
        await call(commands.setTypography, [{ layer, typography: box.typography }])
      }
    }
  } finally {
    if (created.length) useKoharuStore.getState().selectLayers(created)
    await refresh(projectKey, pagesKey, pageKey)
  }
}

// Typography is exactly the style field set (font, size, colour, stroke, alignment, writing
// mode); text, position and box size live elsewhere and are never copied.
export function copyStyle(page: Page) {
  const [layer] = selectedText(page)
  if (layer) useKoharuStore.setState({ styleClipboard: layer.typography ?? defaultTypography })
}

export async function pasteStyle(page: Page) {
  const { styleClipboard } = useKoharuStore.getState()
  const layers = selectedText(page)
  if (!styleClipboard || !layers.length) return
  await call(
    commands.setTypography,
    layers.map(({ id }) => ({ layer: id, typography: styleClipboard })),
  )
  await refresh(projectKey, pageKey)
}
