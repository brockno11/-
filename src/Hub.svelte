<script lang="ts">
  import { onMount } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { emit, listen } from '@tauri-apps/api/event'
  import { confirm, open, save } from '@tauri-apps/plugin-dialog'
  import { disable, enable } from '@tauri-apps/plugin-autostart'
  import { register, unregister } from '@tauri-apps/plugin-global-shortcut'
  import {
    Archive, ArchiveRestore, Check, ChevronDown, FileText, Folder as FolderIcon,
    Hash, Inbox, Minus, Pencil, Pin, PinOff, Plus, Search, Settings, Star,
    Trash2, Undo2, X,
  } from 'lucide-svelte'
  import SettingsModal from './components/SettingsModal.svelte'
  import UpdateModal from './components/UpdateModal.svelte'
  import { backend } from './lib/backend'
  import { tooltip } from './lib/tooltip'
  import { DEFAULT_SETTINGS } from './lib/types'
  import type { AppSettings, Folder, NoteSummary, NoteWindowMode, Tag, WorkspaceState } from './lib/types'
  import {
    checkForAppUpdate, downloadAndInstallAppUpdate, getCurrentAppVersion,
    type AppUpdateInfo, type UpdateDownloadProgress,
  } from './lib/updater'

  type HubView = 'notes' | 'pinned' | 'archived' | 'trash'
  type ContextMenu = { note: NoteSummary; x: number; y: number } | null

  let notes: NoteSummary[] = []
  let folders: Folder[] = []
  let workspace: WorkspaceState = { openNoteIds: [], activeNoteId: null }
  let settings: AppSettings = { ...DEFAULT_SETTINGS }
  let view: HubView = 'notes'
  let selectedFolderId: string | null | undefined = undefined
  let selectedTag = ''
  let query = ''
  let loading = true
  let settingsOpen = false
  let filtersOpen = false
  let selectionMode = false
  let selectedIds = new Set<string>()
  let contextMenu: ContextMenu = null
  let searchInput: HTMLInputElement
  let registeredHotkey = ''
  let visibleDesktopNotes = new Set<string>()
  let toast = ''
  let creatingFolder = false
  let editingFolderId = ''
  let folderDraft = ''
  let toastTimer: ReturnType<typeof setTimeout> | undefined
  let quitFallback: ReturnType<typeof setTimeout> | undefined
  let pendingQuitIds = new Set<string>()
  let appVersion = '0.6.0'
  let checkingUpdate = false
  let availableUpdate: AppUpdateInfo | null = null
  let installingUpdate = false
  let updateError = ''
  let updateProgress: UpdateDownloadProgress = { downloadedBytes: 0, finished: false }
  let startupUpdateTimer: ReturnType<typeof setTimeout> | undefined
  const compactPreview = !backend.isDesktop
    && new URLSearchParams(window.location.search).get('preview') === 'compact'

  $: themeClass = settings.theme === 'system' ? '' : `theme-${settings.theme}`
  $: activeNotes = notes.filter((note) => !note.trashedAt)
  $: trashNotes = notes.filter((note) => note.trashedAt)
  $: tags = collectTags(activeNotes)
  $: visibleNotes = notes
    .filter((note) => matchesCurrentView(note, view, query, selectedFolderId, selectedTag))
    .sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.updatedAt.localeCompare(a.updatedAt))
  $: selectedCount = selectedIds.size
  $: filterCount = Number(selectedFolderId !== undefined) + Number(Boolean(selectedTag))
  $: currentTitle = view === 'notes' ? '全部便签'
    : view === 'pinned' ? '置顶便签'
      : view === 'archived' ? '归档'
        : '回收站'
  $: currentHint = view === 'notes' ? '最近更新的内容'
    : view === 'pinned' ? '需要持续关注的内容'
      : view === 'archived' ? '暂时收起，但仍然保留'
        : '清空后才会永久删除'
  $: folderFilterName = selectedFolderId === undefined ? '所有位置'
    : selectedFolderId === null ? '未分类'
      : folders.find((folder) => folder.id === selectedFolderId)?.name || '文件夹'

  function showToast(message: string) {
    toast = message
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => (toast = ''), 2400)
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
    settingsOpen = false
    if (!installingUpdate) {
      availableUpdate = null
      updateError = ''
    }
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

  function matchesCurrentView(
    note: NoteSummary,
    currentView: HubView,
    currentQuery: string,
    folderId: string | null | undefined,
    tagName: string,
  ) {
    const search = currentQuery.trim().toLocaleLowerCase()
    if (search && !(
      note.title.toLocaleLowerCase().includes(search)
      || note.preview.toLocaleLowerCase().includes(search)
      || note.tags.some((tag) => tag.name.toLocaleLowerCase().includes(search))
    )) return false
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

  async function refresh() {
    const [nextNotes, nextFolders, nextWorkspace, visibleWindows] = await Promise.all([
      backend.listNotes('', true, true),
      backend.listFolders(),
      backend.getWorkspaceState(),
      backend.listVisibleNoteWindows(),
    ])
    notes = nextNotes
    folders = nextFolders
    workspace = nextWorkspace
    visibleDesktopNotes = new Set(visibleWindows.map((state) => state.noteId))
  }

  function startWindowDrag(event: MouseEvent) {
    if (!backend.isDesktop || event.button !== 0) return
    const target = event.target
    if (target instanceof Element && target.closest('button,input,select,a,[role="button"]')) return
    void getCurrentWindow().startDragging()
  }

  async function openNote(id: string, preferredMode?: NoteWindowMode) {
    const note = notes.find((item) => item.id === id)
    if (!note || note.trashedAt) return
    workspace = {
      openNoteIds: workspace.openNoteIds.includes(id)
        ? workspace.openNoteIds : [...workspace.openNoteIds, id],
      activeNoteId: id,
    }
    await backend.saveWorkspaceState(workspace)
    await backend.openNoteWindow(id, preferredMode)
    visibleDesktopNotes = new Set(visibleDesktopNotes).add(id)
  }

  async function createNote() {
    const folderId = selectedFolderId !== undefined && view !== 'trash' ? selectedFolderId : null
    const note = await backend.createNote(folderId)
    await refresh()
    await emitDesktop('qingjian://notes-changed')
    await openNote(note.id, 'card')
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

  function showContextMenu(event: MouseEvent, note: NoteSummary) {
    event.preventDefault()
    filtersOpen = false
    contextMenu = {
      note,
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - 196)),
      y: Math.max(60, Math.min(event.clientY, window.innerHeight - 198)),
    }
  }

  async function trashMany(ids: string[]) {
    for (const id of ids) await backend.deleteNote(id)
    clearSelection()
    contextMenu = null
    await refresh()
    await emitDesktop('qingjian://notes-changed')
    showToast(ids.length > 1 ? `已将 ${ids.length} 条便签移到回收站` : '已移到回收站，可随时恢复')
  }

  async function restoreMany(ids: string[]) {
    for (const id of ids) await backend.restoreNote(id)
    clearSelection()
    contextMenu = null
    await refresh()
    await emitDesktop('qingjian://notes-changed')
    showToast(ids.length > 1 ? `已恢复 ${ids.length} 条便签` : '便签已恢复')
  }

  async function archiveMany(ids: string[], archived: boolean) {
    for (const id of ids) await backend.setNoteArchived(id, archived)
    clearSelection()
    contextMenu = null
    await refresh()
    await emitDesktop('qingjian://notes-changed')
    showToast(archived ? '已归档' : '已移回全部便签')
  }

  async function togglePin(note: NoteSummary) {
    await backend.setNotePinned(note.id, !note.pinned)
    contextMenu = null
    await refresh()
    await emitDesktop('qingjian://notes-changed')
  }

  async function permanentlyDeleteMany(ids: string[]) {
    const approved = backend.isDesktop
      ? await confirm(`将永久删除 ${ids.length} 条便签，且无法恢复。`, {
          title: '永久删除', kind: 'warning', okLabel: '永久删除', cancelLabel: '取消',
        })
      : window.confirm(`永久删除 ${ids.length} 条便签？`)
    if (!approved) return
    for (const id of ids) await backend.permanentlyDeleteNote(id)
    clearSelection()
    contextMenu = null
    await refresh()
    await emitDesktop('qingjian://notes-changed')
    showToast('便签已永久删除')
  }

  async function emptyTrash() {
    if (!trashNotes.length) return
    const approved = backend.isDesktop
      ? await confirm(`回收站中的 ${trashNotes.length} 条便签将永久删除。`, {
          title: '清空回收站', kind: 'warning', okLabel: '清空', cancelLabel: '取消',
        })
      : window.confirm('清空回收站？')
    if (!approved) return
    await backend.emptyTrash()
    clearSelection()
    await refresh()
    await emitDesktop('qingjian://notes-changed')
    showToast('回收站已清空')
  }

  function beginCreateFolder() {
    editingFolderId = ''
    folderDraft = ''
    creatingFolder = true
    filtersOpen = true
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
    if (next.hotkey !== settings.hotkey) await registerHotkey(next.hotkey)
    if (backend.isDesktop && next.startOnBoot !== settings.startOnBoot) {
      if (next.startOnBoot) await enable()
      else await disable()
    }
    settings = await backend.saveSettings(next)
    await emitDesktop('qingjian://settings-changed', settings)
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

  async function flushNotesForUpdate() {
    const visible = await backend.listVisibleNoteWindows()
    const pending = new Set(visible.map((state) => state.noteId))
    if (!pending.size) return
    let resolveReady: () => void = () => {}
    const ready = new Promise<void>((resolve) => { resolveReady = resolve })
    const unlisten = await listen<string>('qingjian://note-update-save-ready', (event) => {
      pending.delete(event.payload)
      if (!pending.size) resolveReady()
    })
    await emitDesktop('qingjian://flush-for-update')
    await Promise.race([
      ready,
      new Promise<void>((resolve) => setTimeout(resolve, 2000)),
    ])
    unlisten()
  }

  async function installAvailableUpdate() {
    if (!availableUpdate || installingUpdate) return
    installingUpdate = true
    updateError = ''
    updateProgress = { downloadedBytes: 0, finished: false }
    try {
      await flushNotesForUpdate()
      await downloadAndInstallAppUpdate((progress) => (updateProgress = progress))
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
    await backend.exportBackup(destination)
    showToast('完整备份已导出')
  }

  async function restoreBackup() {
    const source = backend.isDesktop ? await open({
      title: '选择轻笺备份', multiple: false,
      filters: [{ name: '轻笺备份', extensions: ['qingjian-backup'] }],
    }) : null
    if (!source || Array.isArray(source)) return
    await backend.restoreBackup(source, false)
    await refresh()
    await emitDesktop('qingjian://notes-changed')
    showToast('备份已合并导入')
  }

  function handleKeyboard(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      if (availableUpdate && !installingUpdate) { availableUpdate = null; updateError = ''; return }
      if (settingsOpen) { settingsOpen = false; return }
      if (filtersOpen) { filtersOpen = false; return }
      if (contextMenu) { contextMenu = null; return }
      if (selectionMode) clearSelection()
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
    let disposed = false
    void (async () => {
      try {
        settings = await backend.getSettings()
        appVersion = await getCurrentAppVersion()
        await refresh()
        if (!backend.isDesktop || disposed) return
        await registerHotkey(settings.hotkey)
        await register('Ctrl+Alt+L', (event) => {
          if (event.state === 'Pressed') void backend.unlockAllNoteWindows()
        })
        await register('Ctrl+Alt+O', (event) => {
          if (event.state === 'Pressed') void backend.restoreAllNoteVisibility()
        })
        cleanups.push(await getCurrentWindow().onCloseRequested((event) => {
          event.preventDefault(); void getCurrentWindow().hide()
        }))
        cleanups.push(...await Promise.all([
          listen('qingjian://notes-changed', () => void refresh()),
          listen('qingjian://note-windows-changed', () => void refresh()),
          listen<WorkspaceState>('qingjian://workspace-changed', (event) => (workspace = event.payload)),
          listen('qingjian://new-note', () => void createNote()),
          listen('qingjian://settings', openSettings),
          listen('qingjian://check-update', () => void runUpdateCheck(true)),
          listen('qingjian://quit-requested', () => {
            void (async () => {
              const visible = await backend.listVisibleNoteWindows()
              pendingQuitIds = new Set(visible.map((state) => state.noteId))
              await emitDesktop('qingjian://flush-and-quit')
              if (!pendingQuitIds.size) { await backend.quit(); return }
              clearTimeout(quitFallback)
              quitFallback = setTimeout(() => void backend.quit(), 2000)
            })()
          }),
          listen<string>('qingjian://note-save-ready', (event) => {
            pendingQuitIds.delete(event.payload)
            pendingQuitIds = new Set(pendingQuitIds)
            if (!pendingQuitIds.size) { clearTimeout(quitFallback); void backend.quit() }
          }),
        ]))
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
      if (backend.isDesktop) void unregister('Ctrl+Alt+L')
      if (backend.isDesktop) void unregister('Ctrl+Alt+O')
      clearTimeout(quitFallback)
      clearTimeout(startupUpdateTimer)
    }
  })
</script>

<svelte:window on:keydown={handleKeyboard} on:click={() => (contextMenu = null)} on:blur={handleHubBlur} />

<main class={`hub-window hub-redesign ${compactPreview ? 'compact-preview' : ''} ${themeClass}`}>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <header class="hub-titlebar hub-redesign-titlebar" on:mousedown={startWindowDrag}>
    <div class="hub-brand">
      <span class="hub-mark">轻</span>
      <div><strong>轻笺</strong><small>所有内容只在本机</small></div>
    </div>
    <div class="hub-window-actions">
      <button class="ghost icon-only" aria-label="设置" use:tooltip={'设置 · Ctrl+,'} on:click={openSettings}><Settings /></button>
      <button class="ghost icon-only" aria-label="最小化" use:tooltip={'最小化'} on:click={() => getCurrentWindow().minimize()}><Minus /></button>
      <button class="ghost icon-only close-action" aria-label="隐藏到托盘" use:tooltip={'隐藏到系统托盘'} on:click={() => getCurrentWindow().hide()}><X /></button>
    </div>
  </header>

  <section class="hub-redesign-content">
    <div class="hub-command-row">
      <label class="hub-search redesign-search">
        <Search />
        <input bind:this={searchInput} bind:value={query} placeholder="搜索全部内容" aria-label="搜索便签" />
        {#if !query}<kbd>Ctrl K</kbd>{/if}
        {#if query}<button aria-label="清空搜索" on:click={() => (query = '')}><X /></button>{/if}
      </label>
      <button class="hub-new-note redesign-new" on:click={() => void createNote()}><Plus /><span>新建</span></button>
    </div>

    <nav class="hub-status-nav" aria-label="便签状态">
      <button class:active={view === 'notes'} on:click={() => selectView('notes')}>
        <FileText /><span>便签</span><i>{activeNotes.filter((note) => !note.archived).length}</i>
      </button>
      <button class:active={view === 'pinned'} on:click={() => selectView('pinned')}>
        <Star /><span>置顶</span><i>{activeNotes.filter((note) => note.pinned && !note.archived).length}</i>
      </button>
      <button class:active={view === 'archived'} on:click={() => selectView('archived')}>
        <Archive /><span>归档</span><i>{activeNotes.filter((note) => note.archived).length}</i>
      </button>
      <button class:active={view === 'trash'} on:click={() => selectView('trash')}>
        <Trash2 /><span>回收站</span><i>{trashNotes.length}</i>
      </button>
    </nav>

    <div class="hub-list-heading">
      <div>
        <h1>{currentTitle}</h1>
        <p>{query ? `搜索“${query}”` : currentHint} · {visibleNotes.length} 条</p>
      </div>
      <div class="hub-heading-actions">
        {#if view !== 'trash'}
          <button class:active={filtersOpen || filterCount > 0} on:click|stopPropagation={() => { contextMenu = null; filtersOpen = !filtersOpen }}>
            <FolderIcon /><span>{filterCount ? `${folderFilterName}${selectedTag ? ` · #${selectedTag}` : ''}` : '筛选'}</span><ChevronDown />
          </button>
        {:else if trashNotes.length}
          <button class="danger-text" on:click={() => void emptyTrash()}><Trash2 /><span>清空</span></button>
        {/if}
        <button class:active={selectionMode} on:click={() => selectionMode ? clearSelection() : (selectionMode = true)}>
          <Check /><span>{selectionMode ? '取消' : '选择'}</span>
        </button>
      </div>
    </div>

    <div class="hub-note-list redesign-list" aria-busy={loading}>
      {#each visibleNotes as note (note.id)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="hub-note-card redesign-card"
          data-note-id={note.id}
          class:active={!selectionMode && workspace.activeNoteId === note.id}
          class:selected={selectedIds.has(note.id)}
          role="button"
          tabindex="0"
          aria-pressed={selectedIds.has(note.id)}
          on:click={(event) => { event.stopPropagation(); handleCardClick(note) }}
          on:keydown={(event) => {
            if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); handleCardClick(note) }
          }}
          on:contextmenu={(event) => showContextMenu(event, note)}
        >
          {#if selectionMode || view === 'trash'}
            <span class="note-select-indicator" class:checked={selectedIds.has(note.id)}>
              {#if selectedIds.has(note.id)}<Check />{/if}
            </span>
          {/if}
          <div class="redesign-card-body">
            <div class="hub-note-title">
              <strong>{note.title || '新便签'}</strong>
              {#if visibleDesktopNotes.has(note.id)}<span class="desktop-visible-badge">桌面已显示</span>{/if}
              {#if note.pinned && !note.trashedAt}<Pin />{/if}
              <time>{formatNoteDate(note.trashedAt || note.updatedAt)}</time>
            </div>
            <p>{note.preview || '空白便签'}</p>
            <div class="hub-note-meta redesign-meta">
              {#if note.folderId}<span class="meta-location"><FolderIcon />{folders.find((folder) => folder.id === note.folderId)?.name || '文件夹'}</span>{/if}
              {#each note.tags.slice(0, 2) as tag}
                <span style={`--tag-color:${tag.color}`}>{tag.name}</span>
              {/each}
              {#if note.trashedAt}<span>已移到回收站</span>{/if}
            </div>
          </div>
        </div>
      {:else}
        <div class="hub-empty redesign-empty">
          <span class="empty-mark">{#if view === 'trash'}<Trash2 />{:else}<FileText />{/if}</span>
          <strong>{query ? '没有找到相关内容' : view === 'trash' ? '回收站是空的' : '这里还没有便签'}</strong>
          <p>{view === 'trash' ? '删除的便签会先保存在这里' : '记录一件现在不想忘记的事'}</p>
          {#if view !== 'trash'}<button on:click={() => void createNote()}><Plus />新建便签</button>{/if}
        </div>
      {/each}
    </div>
  </section>

  {#if filtersOpen}
    <button class="filter-scrim" aria-label="关闭筛选" on:click={() => (filtersOpen = false)}></button>
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
    <aside class="hub-filter-popover" on:click|stopPropagation aria-label="筛选与文件夹">
      <header><div><strong>整理与筛选</strong><small>文件夹是位置，标签是特征</small></div><button aria-label="关闭整理与筛选" on:click={() => (filtersOpen = false)}><X /></button></header>
      <section>
        <div class="filter-section-title"><span>位置</span><button on:click={beginCreateFolder}><Plus />新建</button></div>
        <button class="filter-row" class:active={selectedFolderId === undefined} on:click={() => (selectedFolderId = undefined)}><FileText /><span>所有位置</span><small>{activeNotes.length}</small></button>
        <button class="filter-row" class:active={selectedFolderId === null} on:click={() => (selectedFolderId = null)}><Inbox /><span>未分类</span><small>{activeNotes.filter((note) => note.folderId === null).length}</small></button>
        {#each folders as folder (folder.id)}
          {#if editingFolderId === folder.id}
            <form class="folder-editor" on:submit={(event) => { event.preventDefault(); void submitRenameFolder(folder) }}>
              <FolderIcon /><input bind:value={folderDraft} maxlength="40" use:focusOnMount aria-label={`重命名 ${folder.name}`} /><button>保存</button>
            </form>
          {:else}
            <div class="filter-folder-row" class:active={selectedFolderId === folder.id}>
              <button class="filter-row" on:click={() => (selectedFolderId = folder.id)}><FolderIcon /><span>{folder.name}</span><small>{activeNotes.filter((note) => note.folderId === folder.id).length}</small></button>
              <button aria-label={`重命名 ${folder.name}`} on:click={() => beginRenameFolder(folder)}><Pencil /></button>
              <button class="danger" aria-label={`删除 ${folder.name}`} on:click={() => void removeFolder(folder)}><Trash2 /></button>
            </div>
          {/if}
        {/each}
        {#if creatingFolder}
          <form class="folder-editor new" on:submit={(event) => { event.preventDefault(); void submitNewFolder() }}>
            <Plus /><input bind:value={folderDraft} maxlength="40" placeholder="文件夹名称" use:focusOnMount aria-label="文件夹名称" /><button>创建</button>
          </form>
        {/if}
      </section>
      <section class="filter-tags">
        <div class="filter-section-title"><span>标签</span>{#if selectedTag}<button on:click={() => (selectedTag = '')}>清除</button>{/if}</div>
        <div>
          {#each tags as tag (tag.id)}
            <button class:active={selectedTag === tag.name} on:click={() => (selectedTag = selectedTag === tag.name ? '' : tag.name)}><i style={`--tag-color:${tag.color}`}></i>{tag.name}<small>{tag.count}</small></button>
          {:else}<p><Hash />标签会在这里成为筛选条件</p>{/each}
        </div>
      </section>
      {#if filterCount}<footer><button on:click={() => { selectedFolderId = undefined; selectedTag = '' }}>清除全部筛选</button></footer>{/if}
    </aside>
  {/if}

  {#if contextMenu}
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_noninteractive_element_interactions -->
    <div class="note-context-menu" style={`left:${contextMenu.x}px;top:${contextMenu.y}px`} on:click|stopPropagation role="menu" tabindex="-1">
      {#if contextMenu.note.trashedAt}
        <button on:click={() => void restoreMany([contextMenu!.note.id])}><Undo2 />恢复便签</button>
        <span></span>
        <button class="danger" on:click={() => void permanentlyDeleteMany([contextMenu!.note.id])}><Trash2 />永久删除</button>
      {:else}
        <button on:click={() => { const id = contextMenu!.note.id; contextMenu = null; void openNote(id, 'capsule') }}>胶囊形态打开</button>
        <button on:click={() => { const id = contextMenu!.note.id; contextMenu = null; void openNote(id, 'card') }}>卡片形态打开</button>
        <button on:click={() => { const id = contextMenu!.note.id; contextMenu = null; void openNote(id, 'editor') }}>编辑形态打开</button>
        <span></span>
        <button on:click={() => void togglePin(contextMenu!.note)}>{#if contextMenu.note.pinned}<PinOff />取消置顶{:else}<Pin />置顶便签{/if}</button>
        <button on:click={() => void archiveMany([contextMenu!.note.id], !contextMenu!.note.archived)}>{#if contextMenu.note.archived}<ArchiveRestore />移回全部便签{:else}<Archive />归档便签{/if}</button>
        <button on:click={() => { toggleSelection(contextMenu!.note.id); contextMenu = null }}><Check />选择便签</button>
        <span></span>
        <button class="danger" on:click={() => void trashMany([contextMenu!.note.id])}><Trash2 />移到回收站</button>
      {/if}
    </div>
  {/if}

  {#if selectionMode}
    <div class="bulk-action-bar">
      <strong>{selectedCount ? `已选 ${selectedCount} 条` : '选择便签'}</strong>
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

{#if toast}<div class="toast" role="status">{toast}</div>{/if}
