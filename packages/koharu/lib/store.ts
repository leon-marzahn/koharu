'use client'

import { create } from 'zustand'

import type {
  CanvasState,
  Download,
  EntityId,
  Job,
  Model,
  Frame,
  Preferences,
  StartupState,
  ModelResources,
  PageSize,
  Typography,
} from '@koharu/bridge/protocol'
import { toast } from '@koharu/ui/components/toast'

export type CanvasTool = 'select' | 'text' | 'draw' | 'eraser' | 'color_picker' | 'remove' | 'pan'
export const MIN_BRUSH_DIAMETER = 1
export function maxBrushDiameter(size: PageSize): number {
  // Assume that for all sizes below 2048 width, 128 is sufficient.
  // For anything above, scale by percentage greater than 2048 width.
  return Math.round(Math.max(size.width / 2048, 1.0) * 128)
}

export function isBrushTool(tool: CanvasTool): boolean {
  return tool === 'draw' || tool === 'eraser' || tool === 'remove'
}

export interface CanvasBrush {
  diameter: number
  color: string
}
export type InspectorSection = 'copy' | 'type' | 'layers'
export type ShortcutAction = CanvasTool | 'fit'
export type Shortcuts = Record<ShortcutAction, string>
export type PipelineScope = 'page' | 'selected-pages' | 'project'

// A copied text box. Deliberately carries no region relation: a paste is always a fresh user layer.
export interface CopiedBox {
  source: string | null
  translation: string | null
  typography: Typography | null
  frame: Frame
}

interface KoharuStore {
  initialized: boolean
  preferences: Preferences | null
  translationModels: Model[]
  resources: ModelResources | null
  jobs: Record<string, Job>
  // ponytail: one-box explanations live only in memory; reloading loses them. Upgrade: a stored
  // project component written by the translation stage.
  explanations: Record<string, string>
  explainJobs: Record<string, string>
  expectExplanation: (job: string, entity: string) => void
  dismissExplanation: (entity: string) => void
  downloads: Record<string, Download>
  camera: { zoom: number; translation: [number, number]; fitted: boolean }
  canvasPage: EntityId | null
  canvasRevision: number | null
  canvasGeneration: number
  canvasSize: [number, number]
  fitCanvasRequest: number
  layerFrames: Record<EntityId, Frame>
  selectedLayers: EntityId[]
  // Lives here, not in RegionsOverlay, so it survives the overlay remounting on a refresh.
  selectedRegion: EntityId | null
  selectedPages: EntityId[]
  boxClipboard: { page: EntityId; boxes: CopiedBox[] } | null
  styleClipboard: Typography | null
  tool: CanvasTool
  showRegions: boolean
  drawRegion: boolean
  brush: CanvasBrush
  inspector: InspectorSection
  processingScope: PipelineScope
  settingsOpen: boolean
  shortcuts: Shortcuts
  selectPages: (pages: EntityId[]) => void
  showInspector: (section: InspectorSection) => void
  setProcessingScope: (scope: PipelineScope) => void
  setSettingsOpen: (open: boolean) => void
  selectLayers: (layers: EntityId[]) => void
  selectRegion: (region: EntityId | null) => void
  setTool: (tool: CanvasTool) => void
  toggleRegions: () => void
  toggleDrawRegion: () => void
  setBrush: (brush: CanvasBrush) => void
  setShortcut: (action: ShortcutAction, key: string) => void
  requestCanvasFit: () => void
  dismissJob: (id: string) => void
  dismissDownload: (id: number) => void
}

export const defaultShortcuts: Shortcuts = {
  select: 'v',
  text: 't',
  draw: 'b',
  eraser: 'e',
  color_picker: 'i',
  remove: 'j',
  pan: 'h',
  fit: '0',
}

export const useKoharuStore = create<KoharuStore>()((set) => ({
  initialized: false,
  preferences: null,
  translationModels: [],
  resources: null,
  jobs: {},
  explanations: {},
  explainJobs: {},
  expectExplanation: (job, entity) =>
    set((state) => ({ explainJobs: { ...state.explainJobs, [job]: entity } })),
  dismissExplanation: (entity) =>
    set((state) => {
      const explanations = { ...state.explanations }
      delete explanations[entity]
      return { explanations }
    }),
  downloads: {},
  camera: { zoom: 1, translation: [0, 0], fitted: true },
  canvasPage: null,
  canvasRevision: null,
  canvasGeneration: 0,
  canvasSize: [0, 0],
  fitCanvasRequest: 0,
  layerFrames: {},
  selectedLayers: [],
  selectedRegion: null,
  selectedPages: [],
  boxClipboard: null,
  styleClipboard: null,
  tool: 'select',
  showRegions: false,
  drawRegion: false,
  brush: { diameter: 48, color: '#FFFFFF' },
  inspector: 'copy',
  processingScope: 'selected-pages',
  settingsOpen: false,
  shortcuts: defaultShortcuts,
  selectPages: (selectedPages) => set({ selectedPages: [...new Set(selectedPages)] }),
  showInspector: (inspector) => set({ inspector }),
  setProcessingScope: (processingScope) => set({ processingScope }),
  setSettingsOpen: (settingsOpen) => set({ settingsOpen }),
  // Selecting a layer drops the region selection; clearing layers keeps it (a region press clears layers).
  selectLayers: (selectedLayers) =>
    set((state) => ({
      selectedLayers: [...new Set(selectedLayers)],
      selectedRegion: selectedLayers.length ? null : state.selectedRegion,
    })),
  selectRegion: (selectedRegion) => set({ selectedRegion }),
  setTool: (tool) => set({ tool }),
  toggleRegions: () => set((state) => ({ showRegions: !state.showRegions, drawRegion: false })),
  toggleDrawRegion: () => set((state) => ({ drawRegion: !state.drawRegion })),
  setBrush: (brush) => set({ brush }),
  setShortcut: (action, key) =>
    set((state) => ({
      shortcuts: { ...state.shortcuts, [action]: key.toLowerCase().slice(0, 1) },
    })),
  requestCanvasFit: () => set((state) => ({ fitCanvasRequest: state.fitCanvasRequest + 1 })),
  dismissJob: (id) =>
    set((state) => {
      const jobs = { ...state.jobs }
      delete jobs[id]
      return { jobs }
    }),
  dismissDownload: (id) =>
    set((state) => {
      const downloads = { ...state.downloads }
      delete downloads[String(id)]
      return { downloads }
    }),
}))

export function receiveStartupState(state: StartupState): void {
  useKoharuStore.setState({
    initialized: true,
    preferences: state.preferences,
    jobs: byId(state.jobs),
    ...canvasSnapshot(state.canvas),
  })
}

export function receiveCanvas(canvas: CanvasState): void {
  useKoharuStore.setState(canvasSnapshot(canvas))
}

export function receiveJob(job: Job): void {
  useKoharuStore.setState((state) => {
    const entity = state.explainJobs[job.id]
    return {
      jobs: { ...state.jobs, [job.id]: job },
      ...(entity && job.explanation
        ? { explanations: { ...state.explanations, [entity]: job.explanation } }
        : {}),
    }
  })
}

export function receiveDownload(download: Download): void {
  useKoharuStore.setState((state) => {
    const downloads = { ...state.downloads }
    if (download.state === 'finished') delete downloads[String(download.id)]
    else downloads[String(download.id)] = download
    return { downloads }
  })
}

export function receivePreferences(preferences: Preferences): void {
  useKoharuStore.setState({ preferences })
}

export function receiveTranslationModels(translationModels: Model[]): void {
  useKoharuStore.setState({ translationModels })
}

export function receiveResources(resources: ModelResources): void {
  useKoharuStore.setState({ resources })
}

export function receiveError(message: string): void {
  toast.add({ type: 'error', title: 'Could not complete that action', description: message })
}

function canvasSnapshot(canvas: CanvasState) {
  return {
    canvasPage: canvas.page,
    canvasRevision: canvas.revision,
    canvasGeneration: canvas.generation,
    canvasSize: canvas.size,
    layerFrames: Object.fromEntries(
      canvas.element_frames.map(({ element, frame }) => [element, frame]),
    ),
  }
}

function byId<T extends { id: string | number }>(items: T[]): Record<string, T> {
  return Object.fromEntries(items.map((item) => [String(item.id), item]))
}
