export type Theme = 'system' | 'light' | 'dark'
export type SaveState = 'saved' | 'saving' | 'dirty' | 'error'
export type WindowLayout = 'integrated' | 'separate'
export type HotkeyWindowScope = 'editor' | 'both'
export type HotkeyAction = 'hub' | 'last-note' | 'new-note'

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
  alwaysOnTop: boolean
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

export const DEFAULT_SETTINGS: AppSettings = {
  hotkey: 'Ctrl+Alt+N',
  startOnBoot: false,
  theme: 'system',
  defaultFontFamily: 'Microsoft YaHei UI',
  defaultFontSize: 16,
  sidebarCollapsed: false,
  alwaysOnTop: false,
  windowLayout: 'integrated',
  hotkeyWindowScope: 'editor',
  hotkeyAction: 'hub',
}

export const EMPTY_DOCUMENT: Record<string, unknown> = {
  type: 'doc',
  content: [{ type: 'paragraph' }],
}
