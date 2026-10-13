import { QueryClientProvider } from '@tanstack/react-query'
import { act, fireEvent, render, screen, waitFor } from '@testing-library/react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { CanvasWorkspace } from '@/components/editor/CanvasWorkspace'
import { pageKey, pagesKey, projectKey, queryClient } from '@/lib/queries'
import { useKoharuStore } from '@/lib/store'
import { commands, type Layer } from '@koharu/bridge/protocol'
import { TooltipProvider } from '@koharu/ui/components/tooltip'

const canvas = vi.hoisted(() => ({
  resize: vi.fn(),
  setView: vi.fn(),
  stageManifest: vi.fn(),
  installResource: vi.fn(),
  activateFrame: vi.fn(),
  activatePage: vi.fn(),
  clear: vi.fn(),
  previewOpacity: vi.fn(),
  beginTransform: vi.fn(),
  updateTransform: vi.fn(),
  finishTransform: vi.fn(),
  cancelTransform: vi.fn(),
  beginStroke: vi.fn(),
  extendStroke: vi.fn(),
  finishStroke: vi.fn(),
  cancelStroke: vi.fn(),
  sampleColor: vi.fn(),
  dispose: vi.fn(),
}))

const canvasState = vi.hoisted(() => ({
  canvas,
  error: null as Error | null,
  generation: 1 as number | null,
  hasFrame: true,
  retry: vi.fn(),
  status: 'ready' as 'loading' | 'switching' | 'ready' | 'recovering' | 'error',
}))
const prefetchCanvasPages = vi.hoisted(() => vi.fn(async () => []))

vi.mock('@/components/editor/useCanvas', () => ({
  useCanvas: () => canvasState,
}))
vi.mock('@koharu/bridge/canvas', () => ({
  prefetchCanvasPages,
  workspaceColor: () => [245, 245, 245],
}))

const layer: Layer = {
  type: 'image',
  id: 'element',
  parent: 'page',
  geometry: {
    points: [
      { x: 10, y: 20 },
      { x: 110, y: 20 },
      { x: 110, y: 70 },
      { x: 10, y: 70 },
    ],
  },
  visibility: { visible: true, opacity: 1 },
  image: 'image',
}

const paintLayer: Layer = {
  type: 'raster',
  id: 'paint',
  parent: 'page',
  visibility: { visible: true, opacity: 1 },
  image: 'paint-image',
  name: 'Paint 1',
  kind: 'paint',
}

let nextAnimationFrame = 1
let animationFrames = new Map<number, FrameRequestCallback>()

beforeEach(() => {
  nextAnimationFrame = 1
  animationFrames = new Map()
  vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
    const frame = nextAnimationFrame++
    animationFrames.set(frame, callback)
    return frame
  })
  vi.stubGlobal('cancelAnimationFrame', (frame: number) => animationFrames.delete(frame))
  canvasState.error = null
  canvasState.generation = 1
  canvasState.hasFrame = true
  canvasState.status = 'ready'
  prefetchCanvasPages.mockClear()
})

afterEach(() => vi.unstubAllGlobals())

function installProject() {
  const page = {
    id: 'page',
    label: 'Page',
    size: { width: 1000, height: 1000 },
    layers: [layer],
    regions: [],
  }
  queryClient.setQueryData(projectKey, {
    name: 'Book',
    revision: 1,
    active_page: 'page',
    can_undo: false,
    can_redo: false,
  })
  queryClient.setQueryData(pagesKey, [])
  queryClient.setQueryData(pageKey, page)
  useKoharuStore.setState({ selectedLayers: [], tool: 'select' })
  useKoharuStore.setState({
    canvasPage: 'page',
    canvasRevision: 1,
    canvasGeneration: 1,
    canvasSize: [1000, 1000],
  })
}

function renderWorkspace() {
  render(
    <QueryClientProvider client={queryClient}>
      <TooltipProvider>
        <CanvasWorkspace />
      </TooltipProvider>
    </QueryClientProvider>,
  )
  const surface = screen.getByLabelText('Koharu canvas')
  Object.defineProperty(surface, 'getBoundingClientRect', {
    value: () => ({ x: 10, y: 20, width: 800, height: 600 }),
  })
  return surface
}

describe('canvas interaction adapter', () => {
  it('prefetches only after the authoritative canvas page and generation are active', async () => {
    installProject()
    queryClient.setQueryData(pagesKey, [
      {
        id: 'page',
        label: 'Page',
        size: { width: 1000, height: 1000 },
        source_asset: null,
        layer_count: 1,
      },
      {
        id: 'next',
        label: 'Next',
        size: { width: 1000, height: 1000 },
        source_asset: null,
        layer_count: 1,
      },
    ])
    useKoharuStore.setState({ canvasPage: 'previous' })
    renderWorkspace()

    await Promise.resolve()
    expect(prefetchCanvasPages).not.toHaveBeenCalled()

    act(() => useKoharuStore.setState({ canvasPage: 'page' }))
    await waitFor(() => expect(prefetchCanvasPages).toHaveBeenCalledWith(['next']))
  })

  it('renders an accessible browser canvas and keeps camera updates local', async () => {
    installProject()
    const surface = renderWorkspace()
    expect(screen.getByTestId('webgpu-canvas')).toBeInstanceOf(HTMLCanvasElement)

    fireEvent.wheel(surface, { clientX: 100, clientY: 100, deltaY: 4 })
    fireEvent.wheel(surface, { clientX: 100, clientY: 100, deltaY: 6 })

    await waitFor(() => expect(canvas.setView).toHaveBeenLastCalledWith(1, [0, -10]))
  })

  it('preserves a manual camera when a newer canvas generation becomes ready', () => {
    installProject()
    renderWorkspace()
    const camera = { zoom: 2, translation: [-120, -80] as [number, number], fitted: false }

    act(() => useKoharuStore.setState({ camera }))
    act(() => {
      canvasState.status = 'switching'
      useKoharuStore.setState({ canvasGeneration: 2 })
    })
    act(() => {
      canvasState.status = 'ready'
      canvasState.generation = 2
      useKoharuStore.setState({ canvasRevision: 2 })
    })

    expect(useKoharuStore.getState().camera).toEqual(camera)
  })

  it('suppresses Alt browser handling while an editor input retains focus', () => {
    installProject()
    renderWorkspace()
    const input = document.createElement('input')
    document.body.append(input)
    input.focus()

    const event = new KeyboardEvent('keydown', { key: 'Alt', bubbles: true, cancelable: true })
    input.dispatchEvent(event)

    expect(event.defaultPrevented).toBe(true)
    input.remove()
  })

  it('announces WebGPU startup failures and offers recovery', () => {
    installProject()
    canvasState.status = 'error'
    canvasState.error = new Error('No compatible WebGPU adapter was found.')
    renderWorkspace()
    expect(screen.getByRole('alert')).toHaveTextContent('WebGPU canvas unavailable')
    expect(screen.getByRole('alert')).toHaveTextContent('No compatible WebGPU adapter was found.')
    fireEvent.click(screen.getByRole('button', { name: 'Try again' }))
    expect(canvasState.retry).toHaveBeenCalledOnce()
  })

  it('previews brush input locally and sends only the durable paint commit to Rust', async () => {
    installProject()
    useKoharuStore.setState({ tool: 'draw', brush: { diameter: 48, color: '#FFFFFF' } })
    const commit = vi
      .spyOn(commands, 'commitPaint')
      .mockResolvedValue({ revision: 2, layer: 'paint' })
    const surface = renderWorkspace()
    expect(surface).toHaveStyle({ cursor: 'none' })

    fireEvent.pointerDown(surface, { button: 0, pointerId: 7, clientX: 30, clientY: 40 })
    fireEvent.pointerMove(surface, { pointerId: 7, clientX: 55, clientY: 65 })
    fireEvent.pointerUp(surface, { pointerId: 7, clientX: 58, clientY: 70 })

    await waitFor(() => expect(commit).toHaveBeenCalledOnce())
    expect(canvas.beginStroke).toHaveBeenCalledWith({
      kind: 'paint',
      layer: null,
      point: { x: 20, y: 20 },
      diameter: 48,
      color: [255, 255, 255, 255],
    })
    expect(canvas.extendStroke).toHaveBeenCalledWith(expect.arrayContaining([{ x: 45, y: 45 }]))
    expect(canvas.finishStroke).toHaveBeenCalledOnce()
    expect(commit).toHaveBeenCalledWith(1, null, expect.arrayContaining([{ x: 45, y: 45 }]), {
      diameter: 48,
      color: [255, 255, 255, 255],
    })
  })

  it.each([
    ['revision', { canvasRevision: 2 }],
    ['generation', { canvasGeneration: 2 }],
  ])('cancels an active gesture when the canvas %s changes', async (_name, update) => {
    installProject()
    useKoharuStore.setState({ tool: 'draw', brush: { diameter: 48, color: '#FFFFFF' } })
    const commit = vi
      .spyOn(commands, 'commitPaint')
      .mockResolvedValue({ revision: 2, layer: 'paint' })
    const surface = renderWorkspace()

    fireEvent.pointerDown(surface, { button: 0, pointerId: 8, clientX: 30, clientY: 40 })
    expect(canvas.beginStroke).toHaveBeenCalledOnce()

    act(() => {
      useKoharuStore.setState(update)
    })

    await waitFor(() => expect(canvas.cancelStroke).toHaveBeenCalledOnce())
    fireEvent.pointerUp(surface, { pointerId: 8, clientX: 30, clientY: 40 })
    expect(canvas.finishStroke).not.toHaveBeenCalled()
    expect(commit).not.toHaveBeenCalled()
  })

  it('uses rendered text bounds for hit testing and semantic transforms', async () => {
    installProject()
    queryClient.setQueryData(pageKey, (page: { layers: Layer[] }) => ({
      ...page,
      layers: [
        {
          type: 'text',
          id: 'element',
          parent: 'page',
          geometry: layer.geometry,
          angle_degrees: 0,
          visibility: { visible: true, opacity: 1 },
          content: {
            id: 'content',
            source: { text: 'Source', language: 'en' },
            translation: { text: 'Rendered', language: null },
            role: null,
            source_region: null,
          },
          typography: null,
          layout: 'paragraph',
          automatic_region: null,
        },
      ],
    }))
    useKoharuStore.setState({
      layerFrames: {
        element: { x: 30, y: 40, width: 50, height: 20, angle_degrees: 0 },
      },
    })
    const commit = vi.spyOn(commands, 'commitTransform').mockResolvedValue(2)
    const surface = renderWorkspace()

    fireEvent.pointerDown(surface, { button: 0, pointerId: 9, clientX: 50, clientY: 60 })
    fireEvent.pointerMove(surface, { pointerId: 9, clientX: 70, clientY: 80 })
    fireEvent.pointerUp(surface, { pointerId: 9, clientX: 70, clientY: 80 })

    await waitFor(() => expect(commit).toHaveBeenCalledOnce())
    expect(useKoharuStore.getState().selectedLayers).toEqual(['element'])
    expect(canvas.beginTransform).toHaveBeenCalledWith([
      {
        element: 'element',
        frame: { x: 30, y: 40, width: 50, height: 20, angle_degrees: 0 },
      },
    ])
    expect(canvas.updateTransform).toHaveBeenCalledWith(
      expect.arrayContaining([
        expect.objectContaining({
          element: 'element',
          frame: expect.objectContaining({ x: 50, y: 60 }),
        }),
      ]),
    )
    expect(canvas.finishTransform).toHaveBeenCalledOnce()
    expect(commit).toHaveBeenCalledWith(
      1,
      expect.arrayContaining([
        expect.objectContaining({
          element: 'element',
          frame: expect.objectContaining({ x: 50, y: 60 }),
        }),
      ]),
    )
  })

  it('snaps a dragged box to the bubble center and can be dragged out again after reselecting', async () => {
    installProject()
    queryClient.setQueryData(pageKey, (page: { layers: Layer[] }) => ({
      ...page,
      layers: [
        {
          type: 'text',
          id: 'element',
          parent: 'page',
          geometry: layer.geometry,
          angle_degrees: 0,
          visibility: { visible: true, opacity: 1 },
          content: {
            id: 'content',
            source: { text: 'Source', language: 'en' },
            translation: { text: 'Rendered', language: null },
            role: null,
            source_region: null,
          },
          typography: null,
          layout: 'paragraph',
          automatic_region: null,
        },
      ],
      regions: [
        {
          id: 'bubble',
          parent: 'page',
          geometry: {
            points: [
              { x: 20, y: 30 },
              { x: 100, y: 30 },
              { x: 100, y: 90 },
              { x: 20, y: 90 },
            ],
          },
          kind: 'dev.koharu.region.bubble',
          label: null,
        },
      ],
    }))
    useKoharuStore.setState({
      layerFrames: { element: { x: 30, y: 40, width: 50, height: 20, angle_degrees: 0 } },
    })
    const commit = vi.spyOn(commands, 'commitTransform').mockResolvedValue(2)
    const surface = renderWorkspace()

    // Bubble center is (60, 60); the box center starts at (55, 50). Client = page + (10, 20).
    fireEvent.pointerDown(surface, { button: 0, pointerId: 20, clientX: 50, clientY: 70 })
    fireEvent.pointerMove(surface, { pointerId: 20, clientX: 54, clientY: 79 })
    fireEvent.pointerUp(surface, { pointerId: 20, clientX: 54, clientY: 79 })
    await waitFor(() => expect(commit).toHaveBeenCalledOnce())
    expect(canvas.updateTransform).toHaveBeenLastCalledWith([
      { element: 'element', frame: expect.objectContaining({ x: 35, y: 50 }) },
    ])

    // Deselect, then grab the snapped box again and pull it well past the snap tolerance.
    useKoharuStore.setState({
      layerFrames: { element: { x: 35, y: 50, width: 50, height: 20, angle_degrees: 0 } },
    })
    fireEvent.pointerDown(surface, { button: 0, pointerId: 21, clientX: 300, clientY: 300 })
    fireEvent.pointerUp(surface, { pointerId: 21, clientX: 300, clientY: 300 })
    expect(useKoharuStore.getState().selectedLayers).toEqual([])
    fireEvent.pointerDown(surface, { button: 0, pointerId: 22, clientX: 50, clientY: 80 })
    fireEvent.pointerMove(surface, { pointerId: 22, clientX: 90, clientY: 80 })
    fireEvent.pointerUp(surface, { pointerId: 22, clientX: 90, clientY: 80 })
    await waitFor(() => expect(commit).toHaveBeenCalledTimes(2))
    expect(canvas.updateTransform).toHaveBeenLastCalledWith([
      { element: 'element', frame: expect.objectContaining({ x: 75, y: 50 }) },
    ])
  })

  it('snaps a dragged box flush to the bubble edge and to another box', async () => {
    installProject()
    const text = (id: string) => ({
      type: 'text',
      id,
      parent: 'page',
      geometry: layer.geometry,
      angle_degrees: 0,
      visibility: { visible: true, opacity: 1 },
      content: {
        id: `${id}-content`,
        source: { text: 'Source', language: 'en' },
        translation: { text: 'Rendered', language: null },
        role: null,
        source_region: null,
      },
      typography: null,
      layout: 'paragraph',
      automatic_region: null,
    })
    queryClient.setQueryData(pageKey, (page: { layers: Layer[] }) => ({
      ...page,
      layers: [text('element'), text('other')],
      regions: [
        {
          id: 'bubble',
          parent: 'page',
          geometry: {
            points: [
              { x: 20, y: 30 },
              { x: 100, y: 30 },
              { x: 100, y: 90 },
              { x: 20, y: 90 },
            ],
          },
          kind: 'dev.koharu.region.bubble',
          label: null,
        },
      ],
    }))
    useKoharuStore.setState({
      layerFrames: {
        element: { x: 30, y: 40, width: 50, height: 20, angle_degrees: 0 },
        other: { x: 300, y: 200, width: 10, height: 10, angle_degrees: 0 },
      },
    })
    vi.spyOn(commands, 'commitTransform').mockResolvedValue(2)
    const surface = renderWorkspace()

    // Right edge 80 + 17 = 97 snaps flush to the bubble's 100; y stays 40 (nothing within 8).
    fireEvent.pointerDown(surface, { button: 0, pointerId: 40, clientX: 50, clientY: 70 })
    fireEvent.pointerMove(surface, { pointerId: 40, clientX: 67, clientY: 70 })
    fireEvent.pointerUp(surface, { pointerId: 40, clientX: 67, clientY: 70 })
    await waitFor(() => expect(commands.commitTransform).toHaveBeenCalledOnce())
    expect(canvas.updateTransform).toHaveBeenLastCalledWith([
      { element: 'element', frame: expect.objectContaining({ x: 50, y: 40 }) },
    ])
  })

  it('snaps a dragged box to the drawn region it belongs to, which is not a bubble', async () => {
    installProject()
    queryClient.setQueryData(pageKey, (page: { layers: Layer[] }) => ({
      ...page,
      layers: [
        {
          type: 'text',
          id: 'element',
          parent: 'page',
          geometry: layer.geometry,
          angle_degrees: 0,
          visibility: { visible: true, opacity: 1 },
          content: {
            id: 'element-content',
            source: { text: 'Source', language: 'en' },
            translation: { text: 'Rendered', language: null },
            role: null,
            source_region: null,
          },
          typography: null,
          layout: 'paragraph',
          automatic_region: 'drawn',
        },
      ],
      regions: [
        {
          id: 'drawn',
          parent: 'page',
          geometry: {
            points: [
              { x: 20, y: 30 },
              { x: 100, y: 30 },
              { x: 100, y: 90 },
              { x: 20, y: 90 },
            ],
          },
          kind: 'dev.koharu.region.text',
          label: null,
        },
      ],
    }))
    useKoharuStore.setState({
      layerFrames: { element: { x: 30, y: 40, width: 50, height: 20, angle_degrees: 0 } },
    })
    vi.spyOn(commands, 'commitTransform').mockResolvedValue(2)
    const surface = renderWorkspace()

    // Right edge 80 + 17 = 97 snaps flush to the region's 100.
    fireEvent.pointerDown(surface, { button: 0, pointerId: 41, clientX: 50, clientY: 70 })
    fireEvent.pointerMove(surface, { pointerId: 41, clientX: 67, clientY: 70 })
    fireEvent.pointerUp(surface, { pointerId: 41, clientX: 67, clientY: 70 })
    await waitFor(() => expect(commands.commitTransform).toHaveBeenCalledOnce())
    expect(canvas.updateTransform).toHaveBeenLastCalledWith([
      { element: 'element', frame: expect.objectContaining({ x: 50, y: 40 }) },
    ])
  })

  it('cancels the canvas edit when a move changes nothing, so the next drag can start', async () => {
    installProject()
    useKoharuStore.setState({
      layerFrames: { element: { x: 30, y: 40, width: 50, height: 20, angle_degrees: 0 } },
    })
    const commit = vi.spyOn(commands, 'commitTransform').mockResolvedValue(null)
    const surface = renderWorkspace()

    fireEvent.pointerDown(surface, { button: 0, pointerId: 30, clientX: 50, clientY: 70 })
    fireEvent.pointerMove(surface, { pointerId: 30, clientX: 52, clientY: 70 })
    fireEvent.pointerUp(surface, { pointerId: 30, clientX: 52, clientY: 70 })

    await waitFor(() => expect(commit).toHaveBeenCalledOnce())
    await waitFor(() => expect(canvas.cancelTransform).toHaveBeenCalled())
  })

  it('resizes a selected layer through Koharu selection controls', async () => {
    installProject()
    useKoharuStore.setState({ selectedLayers: ['element'] })
    const commit = vi.spyOn(commands, 'commitTransform').mockResolvedValue(2)
    renderWorkspace()
    Object.defineProperty(screen.getByTestId('canvas-overlay'), 'getBoundingClientRect', {
      value: () => ({ x: 10, y: 20, width: 800, height: 600 }),
    })
    const handle = document.querySelector<HTMLElement>('[data-resize-handle="e"]')!

    fireEvent.pointerDown(handle, { button: 0, pointerId: 10, clientX: 120, clientY: 65 })
    fireEvent.pointerMove(handle, { pointerId: 10, clientX: 140, clientY: 65 })
    fireEvent.pointerUp(handle, { pointerId: 10, clientX: 140, clientY: 65 })

    await waitFor(() => expect(commit).toHaveBeenCalledOnce())
    expect(canvas.beginTransform).toHaveBeenCalledWith([
      { element: 'element', frame: { x: 10, y: 20, width: 100, height: 50, angle_degrees: 0 } },
    ])
    expect(canvas.updateTransform).toHaveBeenCalledWith(
      expect.arrayContaining([
        {
          element: 'element',
          frame: { x: 10, y: 20, width: 120, height: 50, angle_degrees: 0 },
        },
      ]),
    )
    expect(commit).toHaveBeenCalledWith(
      1,
      expect.arrayContaining([
        {
          element: 'element',
          frame: { x: 10, y: 20, width: 120, height: 50, angle_degrees: 0 },
        },
      ]),
    )
  })

  it('shows the automatic region behind the selected text controls', () => {
    installProject()
    queryClient.setQueryData(pageKey, (page: { layers: Layer[] }) => ({
      ...page,
      layers: [
        {
          type: 'text',
          id: 'element',
          parent: 'page',
          geometry: null,
          angle_degrees: null,
          visibility: { visible: true, opacity: 1 },
          content: {
            id: 'content',
            source: { text: 'Source', language: 'en' },
            translation: { text: 'Rendered', language: null },
            role: null,
            source_region: null,
          },
          typography: null,
          layout: 'paragraph',
          automatic_region: 'bubble',
        },
      ],
      regions: [
        {
          id: 'bubble',
          parent: 'page',
          geometry: {
            points: [
              { x: 20, y: 30 },
              { x: 100, y: 30 },
              { x: 100, y: 90 },
              { x: 20, y: 90 },
            ],
          },
          kind: 'dev.koharu.region.bubble',
          label: null,
        },
      ],
    }))
    useKoharuStore.setState({
      selectedLayers: ['element'],
      layerFrames: {
        element: { x: 30, y: 40, width: 50, height: 20, angle_degrees: 0 },
      },
    })

    renderWorkspace()

    expect(screen.getByTestId('text-fit-region').querySelector('polygon')).toHaveAttribute(
      'points',
      '20,30 100,30 100,90 20,90',
    )
  })

  it('targets the selected raster layer with the eraser', async () => {
    installProject()
    queryClient.setQueryData(pageKey, (page: { layers: Layer[] }) => ({
      ...page,
      layers: [...page.layers, paintLayer],
    }))
    useKoharuStore.setState({ tool: 'eraser', selectedLayers: ['paint'] })
    const commit = vi
      .spyOn(commands, 'commitErase')
      .mockResolvedValue({ revision: 2, layer: 'paint' })
    const surface = renderWorkspace()

    fireEvent.pointerDown(surface, { button: 0, pointerId: 11, clientX: 30, clientY: 40 })
    fireEvent.pointerUp(surface, { pointerId: 11, clientX: 30, clientY: 40 })

    await waitFor(() => expect(commit).toHaveBeenCalledOnce())
    expect(canvas.beginStroke).toHaveBeenCalledWith({
      kind: 'erase',
      layer: 'paint',
      point: { x: 20, y: 20 },
      diameter: 48,
    })
    expect(commit).toHaveBeenCalledWith(1, 'paint', expect.arrayContaining([{ x: 20, y: 20 }]), 48)
  })

  it('maps the Remove tool to an inpainting mask gesture', async () => {
    installProject()
    useKoharuStore.setState({ tool: 'remove' })
    const commit = vi.spyOn(commands, 'commitInpaint').mockResolvedValue('job')
    const surface = renderWorkspace()

    fireEvent.pointerDown(surface, { button: 0, pointerId: 12, clientX: 30, clientY: 40 })
    fireEvent.pointerUp(surface, { pointerId: 12, clientX: 30, clientY: 40 })

    await waitFor(() => expect(commit).toHaveBeenCalledOnce())
    expect(canvas.beginStroke).toHaveBeenCalledWith({
      kind: 'inpaint',
      layer: null,
      point: { x: 20, y: 20 },
      diameter: 48,
    })
    expect(commit).toHaveBeenCalledWith(1, expect.arrayContaining([{ x: 20, y: 20 }]), 48)
  })

  it('creates point text on click and paragraph text on drag', async () => {
    installProject()
    useKoharuStore.setState({ tool: 'text' })
    const point = vi
      .spyOn(commands, 'addPointText')
      .mockResolvedValue({ revision: 2, layer: 'point-text' })
    const box = vi
      .spyOn(commands, 'addTextBox')
      .mockResolvedValue({ revision: 3, layer: 'box-text' })
    const surface = renderWorkspace()

    fireEvent.pointerDown(surface, { button: 0, pointerId: 13, clientX: 30, clientY: 40 })
    fireEvent.pointerUp(surface, { pointerId: 13, clientX: 30, clientY: 40 })
    await waitFor(() => expect(point).toHaveBeenCalledWith({ x: 20, y: 20 }))

    fireEvent.pointerDown(surface, { button: 0, pointerId: 14, clientX: 40, clientY: 50 })
    fireEvent.pointerMove(surface, { pointerId: 14, clientX: 140, clientY: 110 })
    fireEvent.pointerUp(surface, { pointerId: 14, clientX: 140, clientY: 110 })
    await waitFor(() => expect(box).toHaveBeenCalledOnce())
    expect(box).toHaveBeenCalledWith({
      x: 30,
      y: 30,
      width: 100,
      height: 60,
      angle_degrees: 0,
    })
  })

  describe('keyboard nudge', () => {
    function installText() {
      installProject()
      queryClient.setQueryData(pageKey, (page: { layers: Layer[] }) => ({
        ...page,
        layers: [
          {
            type: 'text',
            id: 'element',
            parent: 'page',
            geometry: (layer as Extract<Layer, { type: 'image' }>).geometry,
            angle_degrees: 0,
            visibility: { visible: true, opacity: 1 },
            content: {
              id: 'content',
              source: { text: 'Source', language: 'en' },
              translation: { text: 'Rendered', language: null },
              role: null,
              source_region: null,
            },
            typography: null,
            layout: 'paragraph',
            automatic_region: null,
          },
        ],
      }))
      useKoharuStore.setState({
        selectedLayers: ['element'],
        layerFrames: { element: { x: 30, y: 40, width: 50, height: 20, angle_degrees: 0 } },
      })
    }

    beforeEach(() => vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] }))
    afterEach(() => vi.useRealTimers())

    it('moves 1 page pixel, or 10 with Shift, and commits a burst once', async () => {
      installText()
      const commit = vi.spyOn(commands, 'commitTransform').mockResolvedValue(2)
      commit.mockClear()
      renderWorkspace()

      fireEvent.keyDown(window, { key: 'ArrowRight' })
      fireEvent.keyDown(window, { key: 'ArrowRight' })
      fireEvent.keyDown(window, { key: 'ArrowDown', shiftKey: true })
      expect(commit).not.toHaveBeenCalled()
      await act(async () => {
        vi.advanceTimersByTime(1000)
      })

      expect(commit).toHaveBeenCalledOnce()
      expect(commit).toHaveBeenCalledWith(1, [
        { element: 'element', frame: { x: 32, y: 50, width: 50, height: 20, angle_degrees: 0 } },
      ])
    })

    it('Cmd+Up/Down step the font size by 1, Shift by 10, and plain arrows do not', async () => {
      installText()
      queryClient.setQueryData(pageKey, (page: { layers: Layer[] }) => ({
        ...page,
        layers: page.layers.map((value) => ({
          ...value,
          typography: { size: 20, auto_fit: false },
        })),
      }))
      const setTypography = vi.spyOn(commands, 'setTypography').mockResolvedValue(null)
      setTypography.mockClear()
      renderWorkspace()

      fireEvent.keyDown(window, { key: 'ArrowUp', metaKey: true })
      fireEvent.keyDown(window, { key: 'ArrowDown', ctrlKey: true, shiftKey: true })
      fireEvent.keyDown(window, { key: 'ArrowUp' })

      expect(setTypography).toHaveBeenCalledTimes(2)
      expect(setTypography).toHaveBeenNthCalledWith(1, [
        { layer: 'element', typography: { size: 21, auto_fit: false } },
      ])
      expect(setTypography).toHaveBeenNthCalledWith(2, [
        { layer: 'element', typography: { size: 10, auto_fit: false } },
      ])
    })

    it('Cmd+Up on an auto-fit box steps the fit scale and keeps auto-fit', async () => {
      installText()
      queryClient.setQueryData(pageKey, (page: { layers: Layer[] }) => ({
        ...page,
        layers: page.layers.map((value) => ({ ...value, typography: null })),
      }))
      const setTypography = vi.spyOn(commands, 'setTypography').mockResolvedValue(null)
      setTypography.mockClear()
      renderWorkspace()

      fireEvent.keyDown(window, { key: 'ArrowUp', metaKey: true })

      expect(setTypography).toHaveBeenCalledWith([
        {
          layer: 'element',
          typography: expect.objectContaining({ fit_scale: 1.05, auto_fit: true }),
        },
      ])
    })

    it('does nothing while a text field is focused', async () => {
      installText()
      const commit = vi.spyOn(commands, 'commitTransform').mockResolvedValue(2)
      commit.mockClear()
      renderWorkspace()
      const input = document.body.appendChild(document.createElement('input'))

      fireEvent.keyDown(input, { key: 'ArrowLeft' })
      await act(async () => {
        vi.advanceTimersByTime(1000)
      })

      expect(commit).not.toHaveBeenCalled()
      input.remove()
    })
  })
})
