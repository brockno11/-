export type Theme = 'system' | 'light' | 'dark'
export type SaveState = 'saved' | 'saving' | 'dirty' | 'error'
export type WindowLayout = 'integrated' | 'separate'
export type HotkeyWindowScope = 'editor' | 'both'
export type HotkeyAction = 'last-note' | 'new-note'
export type NoteWindowMode = 'capsule' | 'card' | 'editor'
export type DockEdge = 'left' | 'right' | 'top' | 'bottom' | null
export type OpacityLayer = 'background' | 'text'

export interface Tag {
  id: string
  name: string
  color: string
}

export interface NoteSummary {
  id: string
  title: string
  preview: string
  createdAt: string
  updatedAt: string
  pinned: boolean
  archived: boolean
  trashedAt: string | null
  folderId: string | null
  sortOrder: number
  tags: Tag[]
}

export interface Folder {
  id: string
  name: string
  sortOrder: number
  createdAt: string
  updatedAt: string
}

export interface WorkspaceState {
  openNoteIds: string[]
  activeNoteId: string | null
}

export interface NoteWindowState {
  noteId: string
  mode: NoteWindowMode
  visible: boolean
  alwaysOnTop: boolean
  backgroundOpacity: number
  textOpacity: number
  locked: boolean
  dockEdge: DockEdge
  dockOffset: number
  monitorKey: string | null
  updatedAt: string
}

export interface NoteWindowGeometry {
  noteId: string
  mode: NoteWindowMode
  xRatio: number
  yRatio: number
  widthDip: number
  heightDip: number
  workAreaX: number
  workAreaY: number
  workAreaWidth: number
  workAreaHeight: number
  scaleFactor: number
}

export interface Note extends NoteSummary {
  content: Record<string, unknown>
}

export interface AppSettings {
  hotkey: string
  startOnBoot: boolean
  theme: Theme
  defaultFontFamily: string
  defaultFontSize: number
  sidebarCollapsed: boolean
  defaultNoteAlwaysOnTop: boolean
  defaultNoteBackgroundOpacity: number
  defaultNoteTextOpacity: number
  restoreNoteWindowsOnBoot: boolean
  checkUpdatesOnStartup: boolean
  windowLayout: WindowLayout
  hotkeyWindowScope: HotkeyWindowScope
  hotkeyAction: HotkeyAction
}

export interface AttachmentResult {
  id: string
  assetPath: string
  filePath: string
  mime: string
  width: number
  height: number
  sizeBytes: number
}

export interface SaveNoteInput {
  id: string
  title: string
  content: Record<string, unknown>
  plainText: string
  pinned: boolean
  archived: boolean
  tags: string[]
}

export function normalizeHotkeyAction(value: unknown): HotkeyAction {
  return value === 'new-note' ? 'new-note' : 'last-note'
}

export const DEFAULT_SETTINGS: AppSettings = {
  hotkey: 'Ctrl+Alt+N',
  startOnBoot: false,
  theme: 'system',
  defaultFontFamily: 'Microsoft YaHei UI',
  defaultFontSize: 16,
  sidebarCollapsed: false,
  defaultNoteAlwaysOnTop: false,
  defaultNoteBackgroundOpacity: 100,
  defaultNoteTextOpacity: 100,
  restoreNoteWindowsOnBoot: true,
  checkUpdatesOnStartup: true,
  windowLayout: 'integrated',
  hotkeyWindowScope: 'editor',
  hotkeyAction: 'last-note',
}

export const EMPTY_DOCUMENT: Record<string, unknown> = {
  type: 'doc',
  content: [{ type: 'paragraph' }],
}
