import { QueryClientProvider } from '@tanstack/react-query'
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { beforeEach, describe, expect, it, vi } from 'vitest'

import { CanvasWorkspace } from '@/components/editor/CanvasWorkspace'
import { Inspector } from '@/components/editor/Inspector'
import { pageKey, pagesKey, projectKey, queryClient } from '@/lib/queries'
import { useKoharuStore } from '@/lib/store'
import { commands, type Layer, type Typography } from '@koharu/bridge/protocol'
import { TooltipProvider } from '@koharu/ui/components/tooltip'

vi.mock('@/components/editor/useCanvas', () => ({
  useCanvas: () => ({
    canvas: null,
    error: null,
    generation: 1,
    hasFrame: true,
    retry: vi.fn(),
    status: 'ready',
  }),
}))
vi.mock('@koharu/bridge/canvas', () => ({
  prefetchCanvasPages: vi.fn(async () => []),
  workspaceColor: () => [245, 245, 245],
}))

const styled: Typography = {
  preferred_font: 'Anime Ace',
  font_weight: 700,
  font_style: 'italic',
  size: 30,
  auto_fit: false,
  color: [255, 0, 0, 255],
  stroke_color: [0, 0, 255, 255],
  stroke_width: 3,
  alignment: 'Start',
  writing_mode: 'Vertical',
}

function textLayer(id: string, typography: Typography | null): Layer {
  return {
    type: 'text',
    id,
    parent: 'page',
    geometry: {
      points: [
        { x: 30, y: 40 },
        { x: 80, y: 40 },
        { x: 80, y: 60 },
        { x: 30, y: 60 },
      ],
    },
    angle_degrees: 0,
    visibility: { visible: true, opacity: 1 },
    content: {
      id: `${id}-content`,
      source: { text: 'Source', language: 'en' },
      translation: { text: 'Rendered', language: null },
      role: 'dialogue',
      source_region: 'region',
    },
    typography,
    layout: 'paragraph',
    automatic_region: 'region',
    locked: false,
  }
}

function installPage(id: string, layers: Layer[]) {
  queryClient.setQueryData(projectKey, {
    name: 'Book',
    revision: 1,
    active_page: id,
    can_undo: false,
    can_redo: false,
  })
  queryClient.setQueryData(pagesKey, [])
  queryClient.setQueryData(pageKey, {
    id,
    label: id,
    size: { width: 1000, height: 1000 },
    layers,
    regions: [],
  })
  useKoharuStore.setState({ canvasPage: id, canvasRevision: 1, canvasSize: [1000, 1000] })
}

function renderWorkspace() {
  render(
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        <CanvasWorkspace />
      </TooltipProvider>
    </QueryClientProvider>,
  )
}

const frame = { x: 30, y: 40, width: 50, height: 20, angle_degrees: 0 }

function spyCommands() {
  return {
    box: vi.spyOn(commands, 'addTextBox').mockResolvedValue({ revision: 2, layer: 'new' }),
    source: vi.spyOn(commands, 'setSourceText').mockResolvedValue(null),
    translation: vi.spyOn(commands, 'setTranslation').mockResolvedValue(null),
    typography: vi.spyOn(commands, 'setTypography').mockResolvedValue(null),
    geometry: vi.spyOn(commands, 'setGeometry').mockResolvedValue(null),
  }
}

beforeEach(() => {
  vi.restoreAllMocks()
  useKoharuStore.setState({
    selectedLayers: ['a'],
    layerFrames: { a: frame, b: frame },
    tool: 'select',
    boxClipboard: null,
    styleClipboard: null,
  })
})

describe('copy and paste boxes', () => {
  it('pastes a new text layer offset 10 px on the same page, without a region', async () => {
    installPage('p1', [textLayer('a', styled)])
    const spies = spyCommands()
    renderWorkspace()

    fireEvent.keyDown(window, { key: 'c', metaKey: true })
    fireEvent.keyDown(window, { key: 'v', metaKey: true })

    await waitFor(() => expect(spies.typography).toHaveBeenCalled())
    expect(spies.box).toHaveBeenCalledWith({ ...frame, x: 40, y: 50 })
    expect(spies.source).toHaveBeenCalledWith('new', 'Source')
    expect(spies.translation).toHaveBeenCalledWith('new', 'Rendered')
    expect(spies.typography).toHaveBeenCalledWith([{ layer: 'new', typography: styled }])
    // Only the box, text and style commands exist for paste: no region is attached.
    expect(spies.geometry).not.toHaveBeenCalled()
  })

  it('keeps the position when pasting on a different page', async () => {
    installPage('p1', [textLayer('a', styled)])
    const spies = spyCommands()
    renderWorkspace()
    fireEvent.keyDown(window, { key: 'c', ctrlKey: true })

    act(() => installPage('p2', []))
    fireEvent.keyDown(window, { key: 'v', ctrlKey: true })

    await waitFor(() => expect(spies.box).toHaveBeenCalledWith(frame))
  })

  it('ignores copy and paste while a text field is focused', () => {
    installPage('p1', [textLayer('a', styled)])
    const spies = spyCommands()
    renderWorkspace()
    const input = document.body.appendChild(document.createElement('input'))

    fireEvent.keyDown(input, { key: 'c', metaKey: true })
    fireEvent.keyDown(input, { key: 'v', metaKey: true })
    fireEvent.keyDown(input, { key: 'c', metaKey: true, shiftKey: true })
    fireEvent.keyDown(input, { key: 'v', metaKey: true, shiftKey: true })

    expect(useKoharuStore.getState().boxClipboard).toBeNull()
    expect(useKoharuStore.getState().styleClipboard).toBeNull()
    expect(spies.box).not.toHaveBeenCalled()
    expect(spies.typography).not.toHaveBeenCalled()
    input.remove()
  })
})

describe('copy and paste style', () => {
  it('copies typography only, then applies it to every selected box in one command', async () => {
    installPage('p1', [textLayer('a', styled), textLayer('b', null)])
    const spies = spyCommands()
    renderWorkspace()

    fireEvent.keyDown(window, { key: 'C', metaKey: true, shiftKey: true })
    expect(useKoharuStore.getState().styleClipboard).toEqual(styled)
    expect(JSON.stringify(useKoharuStore.getState().styleClipboard)).not.toContain('Source')

    useKoharuStore.setState({ selectedLayers: ['a', 'b'] })
    fireEvent.keyDown(window, { key: 'V', metaKey: true, shiftKey: true })

    await waitFor(() => expect(spies.typography).toHaveBeenCalledOnce())
    expect(spies.typography).toHaveBeenCalledWith([
      { layer: 'a', typography: styled },
      { layer: 'b', typography: styled },
    ])
    expect(spies.box).not.toHaveBeenCalled()
    expect(spies.source).not.toHaveBeenCalled()
    expect(spies.geometry).not.toHaveBeenCalled()
  })

  it('offers labelled Copy style and Paste style buttons in the Inspector', async () => {
    installPage('p1', [textLayer('a', styled), textLayer('b', null)])
    const spies = spyCommands()
    render(
      <QueryClientProvider client={queryClient}>
        <TooltipProvider>
          <Inspector />
        </TooltipProvider>
      </QueryClientProvider>,
    )
    const paste = screen.getByRole('button', { name: 'Paste style' })
    expect(paste).toBeDisabled()

    fireEvent.click(screen.getByRole('button', { name: 'Copy style' }))
    expect(useKoharuStore.getState().styleClipboard).toEqual(styled)
    expect(paste).toBeEnabled()

    useKoharuStore.setState({ selectedLayers: ['b'] })
    fireEvent.click(paste)
    await waitFor(() =>
      expect(spies.typography).toHaveBeenCalledWith([{ layer: 'b', typography: styled }]),
    )
  })

  it('disables both buttons with no text selection', () => {
    installPage('p1', [textLayer('a', styled)])
    useKoharuStore.setState({ selectedLayers: [], styleClipboard: styled })
    render(
      <QueryClientProvider client={queryClient}>
        <TooltipProvider>
          <Inspector />
        </TooltipProvider>
      </QueryClientProvider>,
    )
    expect(screen.getByRole('button', { name: 'Copy style' })).toBeDisabled()
    expect(screen.getByRole('button', { name: 'Paste style' })).toBeDisabled()
  })
})
