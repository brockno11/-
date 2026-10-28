<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { emit, listen } from '@tauri-apps/api/event'
  import { confirm, open, save } from '@tauri-apps/plugin-dialog'
  import { disable, enable } from '@tauri-apps/plugin-autostart'
  import { register, unregister } from '@tauri-apps/plugin-global-shortcut'
  import {
    Archive, ArchiveRestore, Check, ChevronDown, Clock3, FileText, Folder as FolderIcon,
    Hash, Inbox, Minus, MoreHorizontal, Pencil, Pin, PinOff, Plus, RotateCcw,
    Search, Settings, SlidersHorizontal, Star, Trash2, Undo2, X,
  } from 'lucide-svelte'
  import HubNoteCard from './components/HubNoteCard.svelte'
  import SettingsModal from './components/SettingsModal.svelte'
  import UpdateModal from './components/UpdateModal.svelte'
  import { backend } from './lib/backend'
  import { waitForNoteSaves, type SaveBarrierFailure, type SaveBarrierReady } from './lib/save-barrier'
  import { tooltip } from './lib/tooltip'
  import { themeClass as resolveThemeClass } from './lib/theme'
  import { DEFAULT_SETTINGS, isReservedGlobalHotkey } from './lib/types'
  import type { AppSettings, Folder, NoteSummary, NoteWindowMode, NoteChange, Tag, WorkspaceState } from './lib/types'
  import {
    checkForAppUpdate, downloadAndInstallAppUpdate, getCurrentAppVersion,
    type AppUpdateInfo, type UpdateDownloadProgress,
  } from './lib/updater'

  type HubView = 'notes' | 'pinned' | 'archived' | 'trash'
  type SortMode = 'updated' | 'created' | 'manual'
  type ContextMenu = { note: NoteSummary; x: number; y: number } | null
  type UndoAction = { label: string; run: () => Promise<void> }

  let lifecycleBusy = false
  let searchMatches: Set<string> | null = null
  let searchGeneration = 0
  let searchTimer: ReturnType<typeof setTimeout> | undefined
  let searching = false
  let searchError = ''
  let refreshGeneration = 0
  let notes: NoteSummary[] = []
  let folders: Folder[] = []
  let workspace: WorkspaceState = { openNoteIds: [], activeNoteId: null }
  let settings: AppSettings = { ...DEFAULT_SETTINGS }
  let view: HubView = 'notes'
  let selectedFolderId: string | null | undefined = undefined
  let selectedTag = ''
  let query = ''
  let sortMode: SortMode = 'updated'
  let loading = true
  let settingsOpen = false
  let filtersOpen = false
  let selectionMode = false
  let selectedIds = new Set<string>()
  let contextMenu: ContextMenu = null
  let contextMenuElement: HTMLDivElement
  let contextMenuSource: HTMLElement | null = null
  let searchInput: HTMLInputElement
  let registeredHotkey = ''
  let visibleDesktopNotes = new Set<string>()
  let toast = ''
  let undoAction: UndoAction | null = null
  let creatingFolder = false
  let editingFolderId = ''
  let folderDraft = ''
  let toastTimer: ReturnType<typeof setTimeout> | undefined
  let quitting = false
  let appVersion = '0.8.0'
  let checkingUpdate = false
  let availableUpdate: AppUpdateInfo | null = null
  let installingUpdate = false
  let updateError = ''
  let updateProgress: UpdateDownloadProgress = { downloadedBytes: 0, finished: false }
  let startupUpdateTimer: ReturnType<typeof setTimeout> | undefined
  let draggedNoteId = ''
  let dropBeforeId = ''
  let pendingActions = new Set<string>()
  const compactPreview = !backend.isDesktop
    && new URLSearchParams(window.location.search).get('preview') === 'compact'

  $: scheduleSearch(query)
  $: themeClass = resolveThemeClass(settings.themeId)
  $: activeNotes = notes.filter((note) => !note.trashedAt)
  $: trashNotes = notes.filter((note) => note.trashedAt)
  $: tags = collectTags(activeNotes)
  $: folderCounts = collectFolderCounts(activeNotes.filter((note) => !note.archived))
  $: statusCounts = {
    notes: activeNotes.filter((note) => !note.archived).length,
    pinned: activeNotes.filter((note) => note.pinned && !note.archived).length,
    archived: activeNotes.filter((note) => note.archived).length,
    trash: trashNotes.length,
  }
  $: visibleNotes = notes
    .filter((note) => (!query.trim() || searchMatches?.has(note.id)) && matchesCurrentView(note, view, selectedFolderId, selectedTag))
    .sort((a, b) => sortNotes(a, b, sortMode))
  $: pinnedVisibleNotes = view === 'notes' ? visibleNotes.filter((note) => note.pinned) : []
  $: regularVisibleNotes = view === 'notes' ? visibleNotes.filter((note) => !note.pinned) : visibleNotes
  $: canDragNotes = view === 'notes' && !query.trim() && !selectedTag
  $: manualSortAvailable = canDragNotes && selectedFolderId !== undefined
  $: canManualSort = sortMode === 'manual' && manualSortAvailable
  $: if (sortMode === 'manual' && !manualSortAvailable) sortMode = 'updated'
  $: selectedCount = selectedIds.size
  $: filterCount = Number(selectedFolderId !== undefined) + Number(Boolean(selectedTag))
  $: currentTitle = view === 'notes' ? '全部便签'
    : view === 'pinned' ? '列表置顶'
      : view === 'archived' ? '归档'
        : '回收站'
  $: currentHint = view === 'notes' ? '最近更新的内容'
    : view === 'pinned' ? '需要持续关注的内容'
      : view === 'archived' ? '暂时收起，但仍然保留'
        : '清空后才会永久删除'
  $: folderFilterName = selectedFolderId === undefined ? '所有位置'
    : selectedFolderId === null ? '未分类'
      : folders.find((folder) => folder.id === selectedFolderId)?.name || '文件夹'

  function showToast(message: string, action: UndoAction | null = null) {
    toast = message
    undoAction = action
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => { toast = ''; undoAction = null }, action ? 5000 : 2600)
  }

  async function runHubAction<T>(key: string, action: () => Promise<T>): Promise<T | undefined> {
    if (pendingActions.has(key)) return
    if (lifecycleBusy) { showToast('正在保存便签，请稍后重试'); return }
    pendingActions = new Set(pendingActions).add(key)
    try {
      return await action()
    } catch (error) {
      showToast(error instanceof Error ? error.message : '操作失败，请重试')
      return undefined
    } finally {
      const next = new Set(pendingActions)
      next.delete(key)
      pendingActions = next
    }
  }

  async function runUndo() {
    const action = undoAction
    if (!action) return
    clearTimeout(toastTimer)
    toast = ''
    undoAction = null
    await action.run()
    showToast(`${action.label}已撤销`)
  }

  function openSettings() {
    if (installingUpdate) return
    availableUpdate = null
    filtersOpen = false
    contextMenu = null
    settingsOpen = true
  }

  function handleHubBlur() {
    contextMenu = null
    filtersOpen = false
  }

  async function emitDesktop(event: string, payload?: unknown) {
    if (backend.isDesktop) await emit(event, payload)
  }

  function focusOnMount(node: HTMLInputElement) {
    const frame = requestAnimationFrame(() => {
      node.focus()
      node.select()
    })
    return { destroy: () => cancelAnimationFrame(frame) }
  }

  function collectTags(items: NoteSummary[]): Array<Tag & { count: number }> {
    const found = new Map<string, Tag & { count: number }>()
    for (const note of items) {
      for (const tag of note.tags) {
        const current = found.get(tag.id)
        if (current) current.count += 1
        else found.set(tag.id, { ...tag, count: 1 })
      }
    }
    return [...found.values()].sort((a, b) => a.name.localeCompare(b.name, 'zh-CN'))
  }

  function collectFolderCounts(items: NoteSummary[]) {
    const counts = new Map<string | null, number>()
    for (const note of items) counts.set(note.folderId, (counts.get(note.folderId) || 0) + 1)
    return counts
  }

  function sortNotes(a: NoteSummary, b: NoteSummary, mode: SortMode) {
    const pinned = Number(b.pinned) - Number(a.pinned)
    if (pinned) return pinned
    if (mode === 'manual') return a.sortOrder - b.sortOrder || b.updatedAt.localeCompare(a.updatedAt)
    if (mode === 'created') return b.createdAt.localeCompare(a.createdAt)
    return b.updatedAt.localeCompare(a.updatedAt)
  }

  function matchesCurrentView(
    note: NoteSummary,
    currentView: HubView,
    folderId: string | null | undefined,
    tagName: string,
  ) {
    if (currentView === 'trash') return Boolean(note.trashedAt)
    if (note.trashedAt) return false
    if (currentView === 'archived' && !note.archived) return false
    if (currentView !== 'archived' && note.archived) return false
    if (currentView === 'pinned' && !note.pinned) return false
    if (folderId !== undefined && note.folderId !== folderId) return false
    if (tagName && !note.tags.some((tag) => tag.name === tagName)) return false
    return true
  }

  function formatNoteDate(value: string) {
    const date = new Date(value)
    const today = new Date()
    const days = Math.round((new Date(today.getFullYear(), today.getMonth(), today.getDate()).getTime()
      - new Date(date.getFullYear(), date.getMonth(), date.getDate()).getTime()) / 86_400_000)
    if (days === 0) return new Intl.DateTimeFormat('zh-CN', {
      hour: '2-digit', minute: '2-digit', hour12: false,
    }).format(date)
    if (days === 1) return '昨天'
    return new Intl.DateTimeFormat('zh-CN', {
      month: 'numeric', day: 'numeric',
      ...(date.getFullYear() === today.getFullYear() ? {} : { year: '2-digit' }),
    }).format(date)
  }

  function scheduleSearch(value: string) {
    const generation = ++searchGeneration
    clearTimeout(searchTimer)
    searchMatches = value.trim() ? new Set() : null
    searching = Boolean(value.trim())
    searchError = ''
    if (!value.trim()) return
    searchTimer = setTimeout(() => void runSearch(value, generation), 120)
  }

  async function runSearch(value: string, generation: number) {
    try {
      const results = await backend.listNotes(value, true, true)
      if (generation !== searchGeneration) return
      searchMatches = new Set(results.map(note => note.id))
    } catch (error) {
      if (generation === searchGeneration) searchError = error instanceof Error ? error.message : '搜索失败，请重试'
    } finally {
      if (generation === searchGeneration) searching = false
    }
  }

  async function refresh() {
    const generation = ++refreshGeneration
    const [nextNotes, nextFolders, nextWorkspace, visibleWindows] = await Promise.all([
      backend.listNotes('', true, true), backend.listFolders(),
      backend.getWorkspaceState(), backend.listVisibleNoteWindows(),
    ])
    if (generation !== refreshGeneration) return
    notes = nextNotes; folders = nextFolders; workspace = nextWorkspace
    visibleDesktopNotes = new Set(visibleWindows.map(state => state.noteId))
    scheduleSearch(query)
  }

  async function handleNoteChange(change: NoteChange | undefined) {
    if (!change || change.kind === 'collection') { await refresh(); return }
    if (change.kind === 'appearance') return
    try {
      const updated = await Promise.all(change.noteIds.map(id => backend.getNote(id)))
      const byId = new Map(updated.map(note => [note.id, note]))
      notes = notes.map(note => byId.get(note.id) ?? note)
      scheduleSearch(query)
    } catch { await refresh() }
  }

  async function refreshVisibleWindows() {
    const windows = await backend.listVisibleNoteWindows()
    visibleDesktopNotes = new Set(windows.map(state => state.noteId))
  }

  async function withSavedNotes<T>(operationName: string, action: () => Promise<T>, selected?: string[], processWillExit = false): Promise<T> {
    if (!backend.isDesktop) return action()
    if (lifecycleBusy) throw new Error('正在保存便签，请稍后重试')
    lifecycleBusy = true
    const operationId = crypto.randomUUID()
    let completed = false
    try {
      await backend.beginNoteOperation(operationId)
      const live = await backend.listLiveNoteIds()
      const noteIds = selected ? live.filter(id => selected.includes(id)) : live
      await waitForNoteSaves({
        noteIds, operationId, operationName,
        subscribeReady: handler => listen<SaveBarrierReady>('qingjian://note-save-ready', event => handler(event.payload)),
        subscribeFailed: handler => listen<SaveBarrierFailure>('qingjian://note-save-failed', event => handler(event.payload)),
        requestSave: () => emitDesktop('qingjian://prepare-save', { operationId, operationName, noteIds }),
      })
      const result = await action()
      completed = true
      return result
    } finally {
      if (!completed || !processWillExit) {
        try { await backend.endNoteOperation(operationId) }
        finally {
          await emitDesktop('qingjian://release-save', { operationId }).catch(() => undefined)
          lifecycleBusy = false
        }
      }
    }
  }

  function startWindowDrag(event: MouseEvent) {
    if (!backend.isDesktop || event.button !== 0) return
    const target = event.target
    if (target instanceof Element && target.closest('button,input,select,a,[role="button"]')) return
    void getCurrentWindow().startDragging()
  }

  async function openNote(id: string, preferredMode?: NoteWindowMode, sourceNoteId?: string) {
    await runHubAction(`open:${id}`, async () => {
      const note = notes.find((item) => item.id === id)
      if (!note || note.trashedAt) return
      workspace = {
        openNoteIds: workspace.openNoteIds.includes(id)
          ? workspace.openNoteIds : [...workspace.openNoteIds, id],
        activeNoteId: id,
      }
      await backend.saveWorkspaceState(workspace)
      await backend.openNoteWindow(id, preferredMode, sourceNoteId)
      visibleDesktopNotes = new Set(visibleDesktopNotes).add(id)
    })
  }

  async function createNote(sourceNoteId?: string) {
    await runHubAction('create-note', async () => {
      const folderId = selectedFolderId !== undefined && view !== 'trash' ? selectedFolderId : null
      const note = await backend.createNote(folderId)
      await refresh()
      await emitDesktop('qingjian://notes-changed', { noteIds: [], kind: 'collection' })
      await openNote(note.id, 'card', sourceNoteId)
    })
  }

  function selectView(next: HubView) {
    view = next
    filtersOpen = false
    contextMenu = null
    clearSelection()
    if (next === 'trash') {
      selectedFolderId = undefined
      selectedTag = ''
    }
  }

  function selectFolder(folderId: string | null | undefined) {
    selectedFolderId = folderId
    if (window.innerWidth < 560) filtersOpen = false
  }

  function selectTag(tagName: string) {
    selectedTag = selectedTag === tagName ? '' : tagName
    if (window.innerWidth < 560) filtersOpen = false
  }

  function clearSelection() {
    selectedIds = new Set()
    selectionMode = false
  }

  function toggleSelection(id: string) {
    const next = new Set(selectedIds)
    if (next.has(id)) next.delete(id)
    else next.add(id)
    selectedIds = next
    selectionMode = next.size > 0
  }

  function handleCardClick(note: NoteSummary) {
    if (selectionMode || view === 'trash') toggleSelection(note.id)
    else void openNote(note.id)
  }

  async function showContextMenu(event: MouseEvent, note: NoteSummary) {
    event.preventDefault()
    event.stopPropagation()
    filtersOpen = false
    contextMenuSource = event.currentTarget instanceof HTMLElement ? event.currentTarget
      : document.activeElement instanceof HTMLElement ? document.activeElement : null
    contextMenu = {
      note,
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - 196)),
      y: Math.max(60, Math.min(event.clientY, window.innerHeight - 292)),
    }
    await tick()
    contextMenuElement?.querySelector<HTMLButtonElement>('button')?.focus()
  }

  function closeContextMenu(restoreFocus = false) {
    contextMenu = null
    if (restoreFocus) contextMenuSource?.focus()
    contextMenuSource = null
  }

  function handleContextMenuKeydown(event: KeyboardEvent) {
    if (!contextMenuElement) return
    const buttons = [...contextMenuElement.querySelectorAll<HTMLButtonElement>('button:not(:disabled)')]
    const current = buttons.indexOf(document.activeElement as HTMLButtonElement)
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault()
      const delta = event.key === 'ArrowDown' ? 1 : -1
      buttons[(current + delta + buttons.length) % buttons.length]?.focus()
    } else if (event.key === 'Escape') {
      event.preventDefault(); event.stopPropagation()
      closeContextMenu(true)
    }
  }

  async function trashMany(ids: string[]) {
    await runHubAction('note-mutation', async () => {
      await withSavedNotes('移到回收站', () => backend.batchNoteAction(ids, 'trash'), ids)
      clearSelection()
      contextMenu = null
      await refresh()
      await emitDesktop('qingjian://notes-changed', { noteIds: [], kind: 'collection' })
      showToast(ids.length > 1 ? `已将 ${ids.length} 条便签移到回收站` : '已移到回收站', {
        label: '删除',
        run: async () => {
          await backend.batchNoteAction(ids, 'restore')
          await refresh(); await emitDesktop('qingjian://notes-changed', { noteIds: [], kind: 'collection' })
        },
      })
    })
  }

  async function restoreMany(ids: string[]) {
    await runHubAction('note-mutation', async () => {
      await backend.batchNoteAction(ids, 'restore')
      clearSelection()
      contextMenu = null
      await refresh()
      await emitDesktop('qingjian://notes-changed', { noteIds: [], kind: 'collection' })
      showToast(ids.length > 1 ? `已恢复 ${ids.length} 条便签` : '便签已恢复', {
        label: '恢复',
        run: async () => {
          await withSavedNotes('移到回收站', () => backend.batchNoteAction(ids, 'trash'), ids)
          await refresh(); await emitDesktop('qingjian://notes-changed', { noteIds: [], kind: 'collection' })
        },
      })
    })
  }

  async function archiveMany(ids: string[], archived: boolean) {
    await runHubAction('note-mutation', async () => {
      await withSavedNotes(archived ? '归档' : '移回便签', () => backend.batchNoteAction(ids, archived ? 'archive' : 'unarchive'), ids)
      clearSelection()
      contextMenu = null
      await refresh()
      await emitDesktop('qingjian://notes-changed', { noteIds: [], kind: 'collection' })
      showToast(archived ? '已归档' : '已移回全部便签', {
        label: archived ? '归档' : '移回',
        run: async () => {
          await withSavedNotes('撤销归档', () => backend.batchNoteAction(ids, archived ? 'unarchive' : 'archive'), ids)
          await refresh(); await emitDesktop('qingjian://notes-changed', { noteIds: [], kind: 'collection' })
        },
      })
    })
  }

  async function togglePin(note: NoteSummary) {
    await runHubAction('note-mutation', async () => {
      await backend.setNotePinned(note.id, !note.pinned)
      contextMenu = null
      await handleNoteChange({ noteIds: [note.id], kind: 'metadata' })
      await emitDesktop('qingjian://notes-changed', { noteIds: [note.id], kind: 'metadata' })
    })
  }

  async function permanentlyDeleteMany(ids: string[]) {
    const approved = backend.isDesktop
      ? await confirm(`将永久删除 ${ids.length} 条便签，且无法恢复。`, {
          title: '永久删除', kind: 'warning', okLabel: '永久删除', cancelLabel: '取消',
        })
      : window.confirm(`永久删除 ${ids.length} 条便签？`)
    if (!approved) return
    await runHubAction('note-mutation', async () => {
      await withSavedNotes('永久删除', () => backend.batchNoteAction(ids, 'permanent-delete'), ids)
      clearSelection()
      contextMenu = null
      await refresh()
      await emitDesktop('qingjian://notes-changed', { noteIds: [], kind: 'collection' })
      showToast('便签已永久删除')
    })
  }

  async function emptyTrash() {
    if (!trashNotes.length) return
    const approved = backend.isDesktop
      ? await confirm(`回收站中的 ${trashNotes.length} 条便签将永久删除。`, {
          title: '清空回收站', kind: 'warning', okLabel: '清空', cancelLabel: '取消',
        })
      : window.confirm('清空回收站？')
    if (!approved) return
    await runHubAction('note-mutation', async () => {
      await withSavedNotes('清空回收站', () => backend.emptyTrash())
      clearSelection()
      await refresh()
      await emitDesktop('qingjian://notes-changed', { noteIds: [], kind: 'collection' })
      showToast('回收站已清空')
    })
  }

  function beginCreateFolder() {
    editingFolderId = ''
    folderDraft = ''
    creatingFolder = true
    filtersOpen = window.innerWidth < 560
  }

  async function submitNewFolder() {
    if (!folderDraft.trim()) { creatingFolder = false; return }
    try {
      const folder = await backend.createFolder(folderDraft)
      await refresh()
      selectedFolderId = folder.id
      creatingFolder = false
      folderDraft = ''
      showToast('文件夹已创建')
    } catch (error) {
      showToast(error instanceof Error ? error.message : '创建文件夹失败')
    }
  }

  function beginRenameFolder(folder: Folder) {
    creatingFolder = false
    editingFolderId = folder.id
    folderDraft = folder.name
  }

  async function submitRenameFolder(folder: Folder) {
    if (!folderDraft.trim() || folderDraft.trim() === folder.name) {
      editingFolderId = ''
      return
    }
    await backend.renameFolder(folder.id, folderDraft)
    editingFolderId = ''
    folderDraft = ''
    await refresh()
  }

  async function removeFolder(folder: Folder) {
    const approved = backend.isDesktop
      ? await confirm(`删除“${folder.name}”？其中的便签会变为未分类。`, {
          title: '删除文件夹', kind: 'warning', okLabel: '删除', cancelLabel: '取消',
        })
      : window.confirm(`删除“${folder.name}”？`)
    if (!approved) return
    await backend.deleteFolder(folder.id)
    if (selectedFolderId === folder.id) selectedFolderId = undefined
    await refresh()
    showToast('文件夹已删除，便签仍然保留')
  }

  function handleDragStart(event: DragEvent, note: NoteSummary) {
    if (!canDragNotes) { event.preventDefault(); return }
    draggedNoteId = note.id
    event.dataTransfer?.setData('text/plain', note.id)
    if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
  }

  function handleDragOver(event: DragEvent, note: NoteSummary) {
    if (!canManualSort) return
    if (!draggedNoteId || draggedNoteId === note.id) return
    event.preventDefault()
    dropBeforeId = note.id
    if (event.dataTransfer) event.dataTransfer.dropEffect = 'move'
  }

  async function handleDrop(event: DragEvent, note: NoteSummary) {
    event.preventDefault()
    if (!canManualSort) return
    if (!draggedNoteId || draggedNoteId === note.id) { handleDragEnd(); return }
    await backend.reorderNote(draggedNoteId, note.id)
    handleDragEnd()
    await refresh()
    await emitDesktop('qingjian://notes-changed', { noteIds: [], kind: 'collection' })
    showToast('便签顺序已更新')
  }

  function handleDragEnd() {
    draggedNoteId = ''
    dropBeforeId = ''
  }

  async function dropIntoFolder(event: DragEvent, folderId: string | null) {
    if (!draggedNoteId) return
    event.preventDefault()
    await backend.moveNote(draggedNoteId, folderId)
    handleDragEnd()
    await refresh()
    await emitDesktop('qingjian://notes-changed', { noteIds: [], kind: 'collection' })
    showToast(folderId ? '便签已移入文件夹' : '便签已移到未分类')
  }

  async function handleHotkey() {
    if (settings.hotkeyAction === 'new-note') { await createNote(); return }
    await refresh()
    const id = workspace.activeNoteId ?? activeNotes.find((note) => !note.archived)?.id
    if (id) await openNote(id)
    else await createNote()
  }

  async function registerHotkey(hotkey: string) {
    if (!backend.isDesktop || registeredHotkey === hotkey) return
    const previous = registeredHotkey
    if (previous) { await unregister(previous); registeredHotkey = '' }
    try {
      await register(hotkey, async (event) => {
        if (event.state === 'Pressed') await handleHotkey()
      })
      registeredHotkey = hotkey
    } catch {
      if (previous) {
        await register(previous, async (event) => {
          if (event.state === 'Pressed') await handleHotkey()
        })
        registeredHotkey = previous
      }
      throw new Error(`快捷键 ${hotkey} 已被占用，请换一个组合`)
    }
  }

  async function saveAppSettings(next: AppSettings) {
    if (isReservedGlobalHotkey(next.hotkey)) throw new Error(`${next.hotkey} 是轻笺的固定救援快捷键`)
    const previous = structuredClone(settings)
    const hotkeyChanged = next.hotkey !== previous.hotkey
    const autostartChanged = backend.isDesktop && next.startOnBoot !== previous.startOnBoot
    try {
      if (hotkeyChanged) await registerHotkey(next.hotkey)
      if (autostartChanged) {
        if (next.startOnBoot) await enable()
        else await disable()
      }
      settings = await backend.saveSettings(next)
      await emitDesktop('qingjian://settings-changed', settings)
    } catch (error) {
      if (autostartChanged) {
        try {
          if (previous.startOnBoot) await enable()
          else await disable()
        } catch (rollbackError) { console.warn('恢复开机启动设置失败', rollbackError) }
      }
      if (hotkeyChanged && registeredHotkey !== previous.hotkey) {
        try { await registerHotkey(previous.hotkey) }
        catch (rollbackError) { console.warn('恢复全局快捷键失败', rollbackError) }
      }
      throw error
    }
  }

  async function runUpdateCheck(manual: boolean) {
    if (checkingUpdate) return
    if (!backend.isDesktop) {
      if (manual) showToast('浏览器预览模式不支持检查更新')
      return
    }
    checkingUpdate = true
    updateError = ''
    try {
      const update = await checkForAppUpdate()
      if (!update) {
        if (manual) showToast(`已是最新版本 ${appVersion}`)
        return
      }
      availableUpdate = update
      settingsOpen = false
      await getCurrentWindow().show()
      await getCurrentWindow().setFocus()
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error)
      if (manual) showToast(`检查更新失败：${message}`)
      else console.warn('自动检查更新失败', error)
    } finally {
      checkingUpdate = false
    }
  }

  async function requestQuitSafely() {
    if (quitting) return
    quitting = true
    try {
      await withSavedNotes('退出轻笺', () => backend.quit(), undefined, true)
    } catch (error) {
      showToast(error instanceof Error ? error.message : '退出已取消：便签未能安全保存', null)
    } finally {
      quitting = false
    }
  }

  async function installAvailableUpdate() {
    if (!availableUpdate || installingUpdate) return
    installingUpdate = true
    updateError = ''
    updateProgress = { downloadedBytes: 0, finished: false }
    try {
      await withSavedNotes('安装更新', () => downloadAndInstallAppUpdate((progress) => (updateProgress = progress)), undefined, true)
    } catch (error) {
      updateError = error instanceof Error ? error.message : String(error)
      installingUpdate = false
    }
  }

  async function exportBackup() {
    const destination = backend.isDesktop ? await save({
      title: '导出轻笺完整备份',
      defaultPath: `轻笺备份-${new Date().toISOString().slice(0, 10)}.qingjian-backup`,
      filters: [{ name: '轻笺备份', extensions: ['qingjian-backup'] }],
    }) : null
    if (!destination) return
    await runHubAction('backup', async () => {
      await withSavedNotes('导出备份', () => backend.exportBackup(destination))
      showToast('完整备份已导出')
    })
  }

  async function restoreBackup() {
    const source = backend.isDesktop ? await open({
      title: '选择轻笺备份', multiple: false,
      filters: [{ name: '轻笺备份', extensions: ['qingjian-backup'] }],
    }) : null
    if (!source || Array.isArray(source)) return
    await runHubAction('restore', async () => {
      await withSavedNotes('恢复备份', () => backend.restoreBackup(source, false))
      await refresh()
      await emitDesktop('qingjian://notes-changed', { noteIds: [], kind: 'collection' })
      showToast('备份已合并导入')
    })
  }

  function handleKeyboard(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      if (availableUpdate && !installingUpdate) { availableUpdate = null; updateError = ''; return }
      if (settingsOpen) { settingsOpen = false; return }
      if (filtersOpen) { filtersOpen = false; return }
      if (contextMenu) { contextMenu = null; return }
      if (selectionMode) clearSelection()
    } else if (settingsOpen) {
      return
    } else if (event.ctrlKey && event.key.toLowerCase() === 'k') {
      event.preventDefault(); searchInput?.focus()
    } else if (event.ctrlKey && event.key.toLowerCase() === 'n') {
      event.preventDefault(); void createNote()
    } else if (event.ctrlKey && event.key.toLowerCase() === 'a'
      && document.activeElement !== searchInput) {
      event.preventDefault()
      selectedIds = new Set(visibleNotes.map((note) => note.id))
      selectionMode = selectedIds.size > 0
    } else if (event.ctrlKey && event.key === ',') {
      event.preventDefault(); openSettings()
    }
  }

  onMount(() => {
    const cleanups: Array<() => void> = []
    const registeredRescueHotkeys = new Set<string>()
    let disposed = false
    void (async () => {
      try {
        settings = await backend.getSettings()
        if (isReservedGlobalHotkey(settings.hotkey)) {
          settings = await backend.saveSettings({ ...settings, hotkey: DEFAULT_SETTINGS.hotkey })
        }
        appVersion = await getCurrentAppVersion()
        await refresh()
        if (!backend.isDesktop || disposed) return

        // Register lifecycle events before optional global shortcuts. A shortcut may be
        // occupied by another app, but that must never leave the workbench half-started.
        cleanups.push(...await Promise.all([
          listen<NoteChange>('qingjian://notes-changed', event => void handleNoteChange(event.payload)),
          listen('qingjian://note-windows-changed', () => void refreshVisibleWindows().catch(() => undefined)),
          listen<WorkspaceState>('qingjian://workspace-changed', (event) => (workspace = event.payload)),
          listen<{ sourceNoteId?: string } | null>('qingjian://new-note', event => void createNote(event.payload?.sourceNoteId)),
          listen('qingjian://settings', openSettings),
          listen('qingjian://check-update', () => void runUpdateCheck(true)),
          listen('qingjian://quit-requested', () => void requestQuitSafely()),
          listen<string>('qingjian://operation-blocked', event => showToast(event.payload)),
        ]))

        const unavailableHotkeys: string[] = []
        try {
          await registerHotkey(settings.hotkey)
        } catch {
          unavailableHotkeys.push(settings.hotkey)
        }

        const rescueHotkeys: Array<[string, () => Promise<unknown>]> = [
          ['Ctrl+Alt+L', () => backend.unlockAllNoteWindows()],
          ['Ctrl+Alt+O', () => backend.restoreAllNoteVisibility()],
          ['Ctrl+Alt+H', () => backend.toggleAllNoteWindows()],
        ]
        for (const [hotkey, action] of rescueHotkeys) {
          try {
            await register(hotkey, (event) => {
              if (event.state === 'Pressed') void action()
            })
            registeredRescueHotkeys.add(hotkey)
          } catch {
            unavailableHotkeys.push(hotkey)
          }
        }
        if (unavailableHotkeys.length) {
          showToast(`快捷键 ${unavailableHotkeys.join('、')} 已被占用，应用仍可正常使用`)
        }
        if (settings.checkUpdatesOnStartup) {
          startupUpdateTimer = setTimeout(() => void runUpdateCheck(false), 1800)
        }
      } catch (error) {
        showToast(error instanceof Error ? error.message : '首页初始化失败')
      } finally { loading = false }
    })()
    return () => {
      disposed = true
      cleanups.forEach((cleanup) => cleanup())
      if (registeredHotkey) void unregister(registeredHotkey)
      registeredRescueHotkeys.forEach((hotkey) => void unregister(hotkey))
      clearTimeout(startupUpdateTimer)
      clearTimeout(searchTimer)
      searchGeneration++
    }
  })
</script>

<svelte:window on:keydown={handleKeyboard} on:click={() => closeContextMenu()} on:blur={handleHubBlur} />

<main class={`hub-window qj-workbench ${compactPreview ? 'compact-preview' : ''} ${themeClass}`}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header class="qj-titlebar" on:mousedown={startWindowDrag}>
    <div class="qj-brand"><span>轻</span><strong>轻笺</strong></div>
    <div class="qj-window-actions">
      <button aria-label="设置" use:tooltip={'设置 · Ctrl+,'} on:click={openSettings}><Settings /></button>
      <button aria-label="最小化" use:tooltip={'最小化'} on:click={() => getCurrentWindow().minimize()}><Minus /></button>
      <button class="close-action" aria-label="隐藏到系统托盘" use:tooltip={'隐藏到系统托盘'} on:click={() => getCurrentWindow().hide()}><X /></button>
    </div>
  </header>

  <div class="workbench-shell">
    <aside class="workbench-sidebar" aria-label="便签资料库">
      <div class="sidebar-section-label">资料库</div>
      <nav class="sidebar-primary" aria-label="便签状态">
        <button class:active={view === 'notes'} on:click={() => selectView('notes')}><FileText /><span>全部便签</span><small>{statusCounts.notes}</small></button>
        <button class:active={view === 'pinned'} on:click={() => selectView('pinned')}><Star /><span>置顶</span>{#if statusCounts.pinned}<small>{statusCounts.pinned}</small>{/if}</button>
        <button class:active={view === 'archived'} on:click={() => selectView('archived')}><Archive /><span>归档</span>{#if statusCounts.archived}<small>{statusCounts.archived}</small>{/if}</button>
        <button class:active={view === 'trash'} on:click={() => selectView('trash')}><Trash2 /><span>回收站</span>{#if statusCounts.trash}<small>{statusCounts.trash}</small>{/if}</button>
      </nav>

      <section class="sidebar-library">
        <div class="sidebar-section-heading"><span>文件夹</span><button aria-label="新建文件夹" on:click={beginCreateFolder}><Plus /></button></div>
        <button
          class="sidebar-row"
          class:active={selectedFolderId === undefined}
          on:click={() => selectFolder(undefined)}
        ><FileText /><span>所有位置</span><small>{statusCounts.notes}</small></button>
        <button
          class="sidebar-row"
          class:active={selectedFolderId === null}
          on:click={() => selectFolder(null)}
          on:dragover={(event) => draggedNoteId && event.preventDefault()}
          on:drop={(event) => void dropIntoFolder(event, null)}
        ><Inbox /><span>未分类</span><small>{folderCounts.get(null) || 0}</small></button>
        {#each folders as folder (folder.id)}
          {#if editingFolderId === folder.id}
            <form class="sidebar-folder-editor" on:submit={(event) => { event.preventDefault(); void submitRenameFolder(folder) }}>
              <FolderIcon /><input bind:value={folderDraft} maxlength="40" use:focusOnMount aria-label={`重命名 ${folder.name}`} /><button>保存</button>
            </form>
          {:else}
            <div class="sidebar-folder-row" class:active={selectedFolderId === folder.id}>
              <button
                class="sidebar-row"
                on:click={() => selectFolder(folder.id)}
                on:dragover={(event) => draggedNoteId && event.preventDefault()}
                on:drop={(event) => void dropIntoFolder(event, folder.id)}
              ><FolderIcon /><span>{folder.name}</span><small>{folderCounts.get(folder.id) || 0}</small></button>
              <button aria-label={`重命名 ${folder.name}`} on:click={() => beginRenameFolder(folder)}><Pencil /></button>
              <button class="danger" aria-label={`删除 ${folder.name}`} on:click={() => void removeFolder(folder)}><Trash2 /></button>
            </div>
          {/if}
        {/each}
        {#if creatingFolder}
          <form class="sidebar-folder-editor" on:submit={(event) => { event.preventDefault(); void submitNewFolder() }}>
            <Plus /><input bind:value={folderDraft} maxlength="40" placeholder="文件夹名称" use:focusOnMount aria-label="文件夹名称" /><button>创建</button>
          </form>
        {/if}
      </section>

      <section class="sidebar-library sidebar-tags">
        <div class="sidebar-section-heading"><span>标签</span>{#if selectedTag}<button aria-label="清除标签筛选" on:click={() => (selectedTag = '')}><X /></button>{/if}</div>
        <div class="sidebar-tag-list">
          {#each tags as tag (tag.id)}
            <button class:active={selectedTag === tag.name} on:click={() => selectTag(tag.name)}><i style={`--tag-color:${tag.color}`}></i><span>{tag.name}</span><small>{tag.count}</small></button>
          {:else}<p><Hash />暂无标签</p>{/each}
        </div>
      </section>
    </aside>

    <section class="workbench-main">
      <div class="workbench-command-row">
        <label class="workbench-search">
          <Search />
          <input bind:this={searchInput} bind:value={query} placeholder="搜索标题、正文或标签" aria-label="搜索便签" />
          {#if !query}<kbd>Ctrl K</kbd>{/if}
          {#if query}<button aria-label="清空搜索" on:click={() => (query = '')}><X /></button>{/if}
        </label>
        <button class="workbench-new" disabled={pendingActions.has('create-note')} on:click={() => void createNote()}><Plus /><span>{pendingActions.has('create-note') ? '创建中' : '新建'}</span></button>
      </div>

      <nav class="compact-status-tabs" aria-label="便签状态">
        <button class:active={view === 'notes'} on:click={() => selectView('notes')}>便签 <small>{statusCounts.notes}</small></button>
        <button class:active={view === 'pinned'} on:click={() => selectView('pinned')}>置顶 {#if statusCounts.pinned}<small>{statusCounts.pinned}</small>{/if}</button>
        <button class:active={view === 'archived'} on:click={() => selectView('archived')}>归档</button>
        <button class:active={view === 'trash'} on:click={() => selectView('trash')}>回收站</button>
      </nav>

      <div class="workbench-heading">
        <div><h1>{currentTitle}</h1><p role="status" aria-live="polite" aria-atomic="true">{#if query}找到 {visibleNotes.length} 条结果{:else}<span class="heading-context">{currentHint} · </span>{visibleNotes.length} 条<span class="heading-compact-label">便签</span>{/if}</p></div>
        <div class="workbench-heading-actions">
          <label class="sort-control" aria-label="便签排序">
            <Clock3 />
            <select bind:value={sortMode} aria-label="便签排序">
              <option value="updated">最近更新</option>
              <option value="created">最近创建</option>
              <option value="manual" disabled={!manualSortAvailable}>手动排序</option>
            </select>
            <ChevronDown />
          </label>
          {#if view !== 'trash'}
            <button aria-label="整理与筛选" class="compact-filter-trigger" class:active={filtersOpen || filterCount > 0} on:click|stopPropagation={() => { closeContextMenu(); filtersOpen = !filtersOpen }}>
              <SlidersHorizontal /><span>整理</span>{#if filterCount}<small>{filterCount}</small>{/if}
            </button>
          {:else if trashNotes.length}
            <button class="danger-text" on:click={() => void emptyTrash()}><Trash2 /><span>清空</span></button>
          {/if}
          <button aria-label={selectionMode ? '退出选择模式' : '选择便签'} class:active={selectionMode} on:click={() => selectionMode ? clearSelection() : (selectionMode = true)}><Check /><span>{selectionMode ? '取消' : '选择'}</span></button>
        </div>
      </div>

      {#if query || filterCount}
        <div class="active-filter-bar" aria-label="当前筛选">
          {#if query}<button on:click={() => (query = '')}><Search /><span>{query}</span><X /></button>{/if}
          {#if selectedFolderId !== undefined}<button on:click={() => (selectedFolderId = undefined)}><FolderIcon /><span>{folderFilterName}</span><X /></button>{/if}
          {#if selectedTag}<button on:click={() => (selectedTag = '')}><Hash /><span>{selectedTag}</span><X /></button>{/if}
        </div>
      {/if}

      <div class="workbench-note-list" aria-busy={loading || searching}>
        {#if loading || searching}<p class="workbench-progress" role="status">{searching ? '正在搜索…' : '正在读取便签…'}</p>{/if}
        {#if pinnedVisibleNotes.length}
          <div class="note-group-label"><Star />列表置顶</div>
          {#each pinnedVisibleNotes as note (note.id)}
            <HubNoteCard
              {note} {folders} {query} selectionMode={selectionMode || view === 'trash'}
              selected={selectedIds.has(note.id)} active={!selectionMode && workspace.activeNoteId === note.id}
              desktopVisible={visibleDesktopNotes.has(note.id)} dragEnabled={canDragNotes}
              dropBefore={dropBeforeId === note.id} formattedDate={formatNoteDate(note.trashedAt || note.updatedAt)}
              onActivate={handleCardClick} onTogglePin={togglePin}
              onOpenMenu={(event) => void showContextMenu(event, note)} onContextMenu={(event) => void showContextMenu(event, note)}
              onDragStart={handleDragStart} onDragOver={handleDragOver} onDrop={handleDrop} onDragEnd={handleDragEnd}
            />
          {/each}
          {#if regularVisibleNotes.length}<div class="note-group-label all-notes-label"><FileText />其他便签</div>{/if}
        {/if}
        {#each regularVisibleNotes as note (note.id)}
          <HubNoteCard
            {note} {folders} {query} selectionMode={selectionMode || view === 'trash'}
            selected={selectedIds.has(note.id)} active={!selectionMode && workspace.activeNoteId === note.id}
            desktopVisible={visibleDesktopNotes.has(note.id)} dragEnabled={canDragNotes}
            dropBefore={dropBeforeId === note.id} formattedDate={formatNoteDate(note.trashedAt || note.updatedAt)}
            onActivate={handleCardClick} onTogglePin={togglePin}
            onOpenMenu={(event) => void showContextMenu(event, note)} onContextMenu={(event) => void showContextMenu(event, note)}
            onDragStart={handleDragStart} onDragOver={handleDragOver} onDrop={handleDrop} onDragEnd={handleDragEnd}
          />
        {:else}
          {#if !pinnedVisibleNotes.length && !searching && !loading}
            <div class="workbench-empty">
              <span>{#if view === 'trash'}<Trash2 />{:else}<FileText />{/if}</span>
              <strong>{searchError ? '搜索未完成' : query ? '没有找到相关内容' : view === 'trash' ? '回收站是空的' : '这里还没有便签'}</strong>
              <p>{searchError || (query ? '换个关键词，或清除筛选再试试' : view === 'trash' ? '删除的便签会先保存在这里' : '记录一件现在不想忘记的事')}</p>
              {#if searchError}<button on:click={() => scheduleSearch(query)}>重试搜索</button>{/if}
              {#if view !== 'trash' && !query}<button on:click={() => void createNote()}><Plus />新建便签</button>{/if}
            </div>
          {/if}
        {/each}
      </div>
    </section>
  </div>

  {#if filtersOpen}
    <button class="filter-scrim" aria-label="关闭整理面板" on:click={() => (filtersOpen = false)}></button>
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
    <aside class="compact-organizer" on:click|stopPropagation aria-label="整理与筛选">
      <header><div><strong>整理与筛选</strong><small>文件夹决定位置，标签描述内容</small></div><button aria-label="关闭整理与筛选" on:click={() => (filtersOpen = false)}><X /></button></header>
      <section>
        <div class="organizer-title"><span>位置</span><button on:click={beginCreateFolder}><Plus />新建</button></div>
        <button class="organizer-row" class:active={selectedFolderId === undefined} on:click={() => selectFolder(undefined)}><FileText /><span>所有位置</span><small>{statusCounts.notes}</small></button>
        <button class="organizer-row" class:active={selectedFolderId === null} on:click={() => selectFolder(null)}><Inbox /><span>未分类</span><small>{folderCounts.get(null) || 0}</small></button>
        {#each folders as folder (folder.id)}
          {#if editingFolderId === folder.id}
            <form class="organizer-folder-editor" on:submit={(event) => { event.preventDefault(); void submitRenameFolder(folder) }}><FolderIcon /><input bind:value={folderDraft} maxlength="40" use:focusOnMount aria-label={`重命名 ${folder.name}`} /><button>保存</button></form>
          {:else}
            <div class="organizer-folder-row" class:active={selectedFolderId === folder.id}>
              <button class="organizer-row" on:click={() => selectFolder(folder.id)}><FolderIcon /><span>{folder.name}</span><small>{folderCounts.get(folder.id) || 0}</small></button>
              <button aria-label={`重命名 ${folder.name}`} on:click={() => beginRenameFolder(folder)}><Pencil /></button>
              <button class="danger" aria-label={`删除 ${folder.name}`} on:click={() => void removeFolder(folder)}><Trash2 /></button>
            </div>
          {/if}
        {/each}
        {#if creatingFolder}<form class="organizer-folder-editor" on:submit={(event) => { event.preventDefault(); void submitNewFolder() }}><Plus /><input bind:value={folderDraft} maxlength="40" placeholder="文件夹名称" use:focusOnMount aria-label="文件夹名称" /><button>创建</button></form>{/if}
      </section>
      <section>
        <div class="organizer-title"><span>标签</span>{#if selectedTag}<button on:click={() => (selectedTag = '')}>清除</button>{/if}</div>
        <div class="organizer-tags">
          {#each tags as tag (tag.id)}<button class:active={selectedTag === tag.name} on:click={() => selectTag(tag.name)}><i style={`--tag-color:${tag.color}`}></i>{tag.name}<small>{tag.count}</small></button>
          {:else}<p><Hash />标签会在这里成为筛选条件</p>{/each}
        </div>
      </section>
      {#if filterCount}<footer><button on:click={() => { selectedFolderId = undefined; selectedTag = '' }}>清除全部筛选</button></footer>{/if}
    </aside>
  {/if}

  {#if contextMenu}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
    <div bind:this={contextMenuElement} class="note-context-menu" style={`left:${contextMenu.x}px;top:${contextMenu.y}px`} on:click|stopPropagation on:keydown={handleContextMenuKeydown} role="menu" tabindex="-1">
      {#if contextMenu.note.trashedAt}
        <button on:click={() => void restoreMany([contextMenu!.note.id])}><Undo2 />恢复便签</button>
        <span></span><button class="danger" on:click={() => void permanentlyDeleteMany([contextMenu!.note.id])}><Trash2 />永久删除</button>
      {:else}
        <small>打开方式</small>
        <button on:click={() => { const id = contextMenu!.note.id; closeContextMenu(); void openNote(id, 'capsule') }}><MoreHorizontal />胶囊</button>
        <button on:click={() => { const id = contextMenu!.note.id; closeContextMenu(); void openNote(id, 'card') }}><FileText />卡片</button>
        <button on:click={() => { const id = contextMenu!.note.id; closeContextMenu(); void openNote(id, 'editor') }}><Pencil />完整编辑</button>
        <span></span>
        <button on:click={() => void togglePin(contextMenu!.note)}>{#if contextMenu.note.pinned}<PinOff />取消列表置顶{:else}<Pin />列表置顶{/if}</button>
        <button on:click={() => void archiveMany([contextMenu!.note.id], !contextMenu!.note.archived)}>{#if contextMenu.note.archived}<ArchiveRestore />移回全部便签{:else}<Archive />归档便签{/if}</button>
        <button on:click={() => { toggleSelection(contextMenu!.note.id); closeContextMenu() }}><Check />选择便签</button>
        <span></span><button class="danger" on:click={() => void trashMany([contextMenu!.note.id])}><Trash2 />移到回收站</button>
      {/if}
    </div>
  {/if}

  {#if selectionMode}
    <div class="bulk-action-bar">
      <strong>{selectedCount ? `已选 ${selectedCount} 条` : '请选择便签'}</strong>
      <div>
        {#if view === 'trash'}
          <button disabled={!selectedCount} on:click={() => void restoreMany([...selectedIds])}><Undo2 />恢复</button>
          <button class="danger" disabled={!selectedCount} on:click={() => void permanentlyDeleteMany([...selectedIds])}><Trash2 />永久删除</button>
        {:else}
          <button disabled={!selectedCount} on:click={() => void archiveMany([...selectedIds], view !== 'archived')}>{#if view === 'archived'}<ArchiveRestore />移回{:else}<Archive />归档{/if}</button>
          <button class="danger" disabled={!selectedCount} on:click={() => void trashMany([...selectedIds])}><Trash2 />删除</button>
        {/if}
      </div>
      <button class="bulk-close" aria-label="退出多选" on:click={clearSelection}><X /></button>
    </div>
  {/if}
</main>

{#if settingsOpen}
  <div class={`modal-theme ${themeClass}`}>
    <SettingsModal
      {settings}
      {appVersion}
      {checkingUpdate}
      onSave={saveAppSettings}
      onClose={() => (settingsOpen = false)}
      onBackup={() => void exportBackup()}
      onRestore={() => void restoreBackup()}
      onCheckUpdate={() => void runUpdateCheck(true)}
    />
  </div>
{/if}

{#if availableUpdate}
  <div class={`modal-theme ${themeClass}`}>
    <UpdateModal
      update={availableUpdate}
      installing={installingUpdate}
      progress={updateProgress}
      error={updateError}
      onInstall={() => void installAvailableUpdate()}
      onLater={() => {
        if (!installingUpdate) {
          availableUpdate = null
          updateError = ''
        }
      }}
    />
  </div>
{/if}

{#if toast}<div class="toast action-toast" role="status"><span>{toast}</span>{#if undoAction}<button on:click={() => void runUndo()}><RotateCcw />撤销</button>{/if}</div>{/if}
