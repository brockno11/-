import { convertFileSrc, invoke } from '@tauri-apps/api/core'
import type {
  AppSettings,
  AttachmentResult,
  Folder,
  Note,
  NoteSummary,
  NoteWindowMode,
  NoteWindowState,
  SaveNoteInput,
  WorkspaceState,
} from './types'
import { DEFAULT_SETTINGS, EMPTY_DOCUMENT, normalizeHotkeyAction } from './types'

const inTauri = '__TAURI_INTERNALS__' in window
const MOCK_NOTES_KEY = 'qingjian.mock.notes'
const MOCK_SETTINGS_KEY = 'qingjian.mock.settings'

function now() {
  return new Date().toISOString()
}

function newMockNote(): Note {
  const timestamp = now()
  return {
    id: crypto.randomUUID(),
    title: '新便签',
    preview: '',
    content: structuredClone(EMPTY_DOCUMENT),
    createdAt: timestamp,
    updatedAt: timestamp,
    pinned: false,
    archived: false,
    trashedAt: null,
    folderId: null,
    sortOrder: 0,
    tags: [],
  }
}

function readMockNotes(): Note[] {
  const value = localStorage.getItem(MOCK_NOTES_KEY)
  if (!value) {
    const first = newMockNote()
    localStorage.setItem(MOCK_NOTES_KEY, JSON.stringify([first]))
    return [first]
  }
  return (JSON.parse(value) as Note[]).map((note, index) => ({
    ...note,
    trashedAt: note.trashedAt ?? null,
    folderId: note.folderId ?? null,
    sortOrder: note.sortOrder ?? index + 1,
  }))
}

function writeMockNotes(notes: Note[]) {
  localStorage.setItem(MOCK_NOTES_KEY, JSON.stringify(notes))
}

export const backend = {
  isDesktop: inTauri,

  async listNotes(
    query = '',
    includeArchived = false,
    includeTrashed = false,
  ): Promise<NoteSummary[]> {
    if (inTauri) {
      return invoke('list_notes', { query, includeArchived, includeTrashed })
    }
    const q = query.trim().toLocaleLowerCase()
    return readMockNotes()
      .filter((note) => includeArchived || !note.archived)
      .filter((note) => includeTrashed || !note.trashedAt)
      .filter((note) =>
        !q ||
        note.title.toLocaleLowerCase().includes(q) ||
        note.preview.toLocaleLowerCase().includes(q) ||
        note.tags.some((tag) => tag.name.toLocaleLowerCase().includes(q)),
      )
      .sort((a, b) =>
        Number(b.pinned) - Number(a.pinned) ||
        b.updatedAt.localeCompare(a.updatedAt),
      )
  },

  async getNote(id: string): Promise<Note> {
    if (inTauri) return invoke('get_note', { id })
    const note = readMockNotes().find((item) => item.id === id)
    if (!note) throw new Error('便签不存在')
    return note
  },

  async createNote(folderId: string | null = null): Promise<Note> {
    if (inTauri) return invoke('create_note', { folderId })
    const notes = readMockNotes()
    const note = newMockNote()
    note.folderId = folderId
    note.sortOrder = notes.filter((item) => item.folderId === folderId).length + 1
    writeMockNotes([note, ...notes])
    return note
  },

  async saveNote(input: SaveNoteInput): Promise<Note> {
    if (inTauri) return invoke('save_note', { input })
    const notes = readMockNotes()
    const index = notes.findIndex((note) => note.id === input.id)
    if (index < 0) throw new Error('便签不存在')
    const saved: Note = {
      ...notes[index],
      ...input,
      preview: input.plainText.trim().slice(0, 160),
      updatedAt: now(),
      tags: input.tags.map((name) => ({
        id: name,
        name,
        color: '#E5B83B',
      })),
    }
    notes[index] = saved
    writeMockNotes(notes)
    return saved
  },

  async deleteNote(id: string): Promise<void> {
    if (inTauri) return invoke('delete_note', { id })
    writeMockNotes(readMockNotes().map((note) =>
      note.id === id ? { ...note, trashedAt: now(), pinned: false } : note,
    ))
  },

  async restoreNote(id: string): Promise<void> {
    if (inTauri) return invoke('restore_note', { id })
    writeMockNotes(readMockNotes().map((note) =>
      note.id === id ? { ...note, trashedAt: null } : note,
    ))
  },

  async permanentlyDeleteNote(id: string): Promise<void> {
    if (inTauri) return invoke('permanently_delete_note', { id })
    writeMockNotes(readMockNotes().filter((note) => note.id !== id))
  },

  async emptyTrash(): Promise<void> {
    if (inTauri) return invoke('empty_trash')
    writeMockNotes(readMockNotes().filter((note) => !note.trashedAt))
  },

  async setNoteArchived(id: string, archived: boolean): Promise<void> {
    if (inTauri) return invoke('set_note_archived', { id, archived })
    writeMockNotes(readMockNotes().map((note) =>
      note.id === id ? { ...note, archived, updatedAt: now() } : note,
    ))
  },

  async setNotePinned(id: string, pinned: boolean): Promise<void> {
    if (inTauri) return invoke('set_note_pinned', { id, pinned })
    writeMockNotes(readMockNotes().map((note) =>
      note.id === id ? { ...note, pinned, updatedAt: now() } : note,
    ))
  },

  async listFolders(): Promise<Folder[]> {
    if (inTauri) return invoke('list_folders')
    return JSON.parse(localStorage.getItem('qingjian.mock.folders') || '[]')
  },

  async createFolder(name: string): Promise<Folder> {
    if (inTauri) return invoke('create_folder', { name })
    const folders = await this.listFolders()
    if (folders.some((folder) => folder.name.toLowerCase() === name.trim().toLowerCase())) {
      throw new Error('已经存在同名文件夹')
    }
    const timestamp = now()
    const folder: Folder = {
      id: crypto.randomUUID(),
      name: name.trim(),
      sortOrder: folders.length + 1,
      createdAt: timestamp,
      updatedAt: timestamp,
    }
    localStorage.setItem('qingjian.mock.folders', JSON.stringify([...folders, folder]))
    return folder
  },

  async renameFolder(id: string, name: string): Promise<Folder> {
    if (inTauri) return invoke('rename_folder', { id, name })
    const folders = await this.listFolders()
    const folder = folders.find((item) => item.id === id)
    if (!folder) throw new Error('文件夹不存在')
    folder.name = name.trim()
    folder.updatedAt = now()
    localStorage.setItem('qingjian.mock.folders', JSON.stringify(folders))
    return folder
  },

  async deleteFolder(id: string): Promise<void> {
    if (inTauri) return invoke('delete_folder', { id })
    const folders = (await this.listFolders()).filter((folder) => folder.id !== id)
    localStorage.setItem('qingjian.mock.folders', JSON.stringify(folders))
    const notes = readMockNotes().map((note) =>
      note.folderId === id ? { ...note, folderId: null } : note,
    )
    writeMockNotes(notes)
  },

  async moveNote(
    noteId: string,
    folderId: string | null,
    beforeNoteId: string | null = null,
  ): Promise<void> {
    if (inTauri) return invoke('move_note', { noteId, folderId, beforeNoteId })
    const notes = readMockNotes()
    const note = notes.find((item) => item.id === noteId)
    if (note) note.folderId = folderId
    writeMockNotes(notes)
  },

  async reorderNote(noteId: string, beforeNoteId: string | null): Promise<void> {
    if (inTauri) return invoke('reorder_note', { noteId, beforeNoteId })
  },

  async getWorkspaceState(): Promise<WorkspaceState> {
    if (inTauri) return invoke('get_workspace_state')
    return JSON.parse(localStorage.getItem('qingjian.mock.workspace') || '{"openNoteIds":[],"activeNoteId":null}')
  },

  async saveWorkspaceState(workspace: WorkspaceState): Promise<WorkspaceState> {
    if (inTauri) return invoke('save_workspace_state', { workspace })
    localStorage.setItem('qingjian.mock.workspace', JSON.stringify(workspace))
    return workspace
  },

  async getSettings(): Promise<AppSettings> {
    if (inTauri) return invoke('get_settings')
    const value = localStorage.getItem(MOCK_SETTINGS_KEY)
    if (!value) return DEFAULT_SETTINGS
    const stored = JSON.parse(value)
    return {
      ...DEFAULT_SETTINGS,
      ...stored,
      defaultNoteAlwaysOnTop: stored.defaultNoteAlwaysOnTop ?? stored.alwaysOnTop ?? false,
      defaultNoteOpacity: Math.max(60, Math.min(100, stored.defaultNoteOpacity ?? 100)),
      hotkeyAction: normalizeHotkeyAction(stored.hotkeyAction),
    }
  },

  async saveSettings(settings: AppSettings): Promise<AppSettings> {
    if (inTauri) return invoke('save_settings', { settings })
    localStorage.setItem(MOCK_SETTINGS_KEY, JSON.stringify(settings))
    return settings
  },

  async openNoteWindow(noteId: string, preferredMode?: NoteWindowMode): Promise<NoteWindowState | null> {
    if (inTauri) return invoke('open_note_window', { noteId, preferredMode: preferredMode ?? null })
    const state: NoteWindowState = {
      noteId, mode: preferredMode ?? 'card', visible: true,
      alwaysOnTop: DEFAULT_SETTINGS.defaultNoteAlwaysOnTop,
      opacity: DEFAULT_SETTINGS.defaultNoteOpacity, locked: false,
      dockEdge: null, dockOffset: 0, monitorKey: null, updatedAt: now(),
    }
    localStorage.setItem(`qingjian.mock.window.${noteId}`, JSON.stringify(state))
    return state
  },

  async noteWindowReady(noteId: string): Promise<NoteWindowState> {
    if (inTauri) return invoke('note_window_ready', { noteId })
    return this.getNoteWindowState(noteId)
  },

  async getNoteWindowState(noteId: string): Promise<NoteWindowState> {
    if (inTauri) return invoke('get_note_window_state', { noteId })
    const stored = localStorage.getItem(`qingjian.mock.window.${noteId}`)
    if (stored) return JSON.parse(stored)
    return (await this.openNoteWindow(noteId, 'card'))!
  },

  async listVisibleNoteWindows(): Promise<NoteWindowState[]> {
    if (inTauri) return invoke('list_visible_note_windows')
    return Object.keys(localStorage).filter((key) => key.startsWith('qingjian.mock.window.'))
      .map((key) => JSON.parse(localStorage.getItem(key)!)).filter((state) => state.visible)
  },

  async setNoteWindowMode(noteId: string, mode: NoteWindowMode): Promise<NoteWindowState> {
    if (inTauri) return invoke('set_note_window_mode', { noteId, mode })
    const state = await this.getNoteWindowState(noteId); state.mode = mode
    localStorage.setItem(`qingjian.mock.window.${noteId}`, JSON.stringify(state)); return state
  },

  async setNoteWindowAlwaysOnTop(noteId: string, alwaysOnTop: boolean): Promise<NoteWindowState> {
    if (inTauri) return invoke('set_note_window_always_on_top', { noteId, alwaysOnTop })
    const state = await this.getNoteWindowState(noteId); state.alwaysOnTop = alwaysOnTop
    localStorage.setItem(`qingjian.mock.window.${noteId}`, JSON.stringify(state)); return state
  },

  async setNoteWindowOpacity(noteId: string, opacity: number): Promise<NoteWindowState> {
    if (inTauri) return invoke('set_note_window_opacity', { noteId, opacity })
    const state = await this.getNoteWindowState(noteId); state.opacity = Math.max(60, Math.min(100, opacity))
    localStorage.setItem(`qingjian.mock.window.${noteId}`, JSON.stringify(state)); return state
  },

  async setNoteWindowLocked(noteId: string, locked: boolean): Promise<NoteWindowState> {
    if (inTauri) return invoke('set_note_window_locked', { noteId, locked })
    const state = await this.getNoteWindowState(noteId); state.locked = locked
    localStorage.setItem(`qingjian.mock.window.${noteId}`, JSON.stringify(state)); return state
  },

  async hideNoteWindow(noteId: string): Promise<void> {
    if (inTauri) return invoke('hide_note_window', { noteId })
    const state = await this.getNoteWindowState(noteId); state.visible = false
    localStorage.setItem(`qingjian.mock.window.${noteId}`, JSON.stringify(state))
  },

  async saveNoteWindowGeometry(noteId: string): Promise<void> {
    if (inTauri) return invoke('save_note_window_geometry', { noteId })
  },

  async finalizeNoteWindowMove(noteId: string): Promise<NoteWindowState> {
    if (inTauri) return invoke('finalize_note_window_move', { noteId })
    return this.getNoteWindowState(noteId)
  },

  async collapseDockedNoteWindow(noteId: string): Promise<void> {
    if (inTauri) return invoke('collapse_docked_note_window', { noteId })
  },

  async expandDockedNoteWindow(noteId: string): Promise<void> {
    if (inTauri) return invoke('expand_docked_note_window', { noteId })
  },

  async unlockAllNoteWindows(): Promise<void> {
    if (inTauri) return invoke('unlock_all_note_windows')
  },

  async showHubWindow(): Promise<void> {
    if (inTauri) await invoke('show_hub_window')
  },

  async toggleHubWindow(): Promise<boolean> {
    return inTauri ? invoke('toggle_hub_window') : true
  },

  async listSystemFonts(): Promise<string[]> {
    if (inTauri) return invoke('list_system_fonts')
    return [
      'Microsoft YaHei UI',
      'Microsoft YaHei',
      'DengXian',
      'SimSun',
      'SimHei',
      'KaiTi',
      'Segoe UI Variable',
      'Arial',
      'Georgia',
      'Consolas',
    ]
  },

  async saveAttachment(dataBase64: string, mime: string): Promise<AttachmentResult> {
    if (inTauri) return invoke('save_attachment', { dataBase64, mime })
    return {
      id: crypto.randomUUID(),
      assetPath: '',
      filePath: dataBase64,
      mime,
      width: 0,
      height: 0,
      sizeBytes: dataBase64.length,
    }
  },

  assetUrl(filePath: string): string {
    return inTauri ? convertFileSrc(filePath) : filePath
  },

  async exportHtml(noteId: string, destination: string, html: string): Promise<void> {
    if (inTauri) return invoke('export_html', { noteId, destination, html })
    throw new Error('请在桌面版中使用导出功能')
  },

  async exportMarkdown(destination: string, markdown: string): Promise<void> {
    if (inTauri) return invoke('export_markdown', { destination, markdown })
    throw new Error('请在桌面版中使用导出功能')
  },

  async exportPdf(destination: string): Promise<void> {
    if (inTauri) return invoke('export_pdf', { destination })
    window.print()
  },

  async exportBackup(destination: string): Promise<void> {
    if (inTauri) return invoke('export_backup', { destination })
    throw new Error('请在桌面版中使用备份功能')
  },

  async restoreBackup(source: string, replace: boolean): Promise<void> {
    if (inTauri) return invoke('restore_backup', { source, replace })
    throw new Error('请在桌面版中使用恢复功能')
  },

  async quit(): Promise<void> {
    if (inTauri) await invoke('quit_app')
  },
}
