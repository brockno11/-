<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte'
  import type { Editor as TiptapEditor } from '@tiptap/core'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { emit, listen } from '@tauri-apps/api/event'
  import { confirm, save } from '@tauri-apps/plugin-dialog'
  import {
    Archive, Code2, Copy, Download, FileDown, ImagePlus, LockKeyhole,
    Blend, Bold, CircleCheckBig, Italic, List, ListOrdered, MoreHorizontal,
    Pilcrow, Pin, PinOff, Star, Settings, Strikethrough, Tags, Text, Trash2, Underline,
    Unlock, X, Plus, Maximize2, Minimize2,
  } from 'lucide-svelte'
  import { backend } from './lib/backend'
  import { documentTitle } from './lib/note'
  import { SaveCoordinator } from './lib/save-coordinator'
  import type { SaveBarrierFailure, SaveBarrierRequest } from './lib/save-barrier'
  import { serializeMarkdown } from './lib/markdown'
  import { richHtml } from './lib/rich-html'
  import { INHERITED_APPEARANCE, resolveAppearance } from './lib/appearance'
  import { menu, popover } from './lib/menu'
  import { tooltip, dismissTooltip } from './lib/tooltip'
  import { quietScroll } from './lib/quiet-scroll'
  import { progressSummary, progressPlainText } from './lib/progress'
  import { themeClass as resolveThemeClass } from './lib/theme'
  import { DEFAULT_SETTINGS } from './lib/types'
  import type {
    AppSettings, Note, NoteWindowMode, NoteWindowState, SaveNoteInput, SaveState, NoteAppearance, NotePreferences, NoteViewState, NoteChange,
  } from './lib/types'

  type EditorComponent = typeof import('./components/Editor.svelte').default
  type ToolbarComponent = typeof import('./components/Toolbar.svelte').default

  const query = new URLSearchParams(window.location.search)
  const noteId = query.get('noteId') || getCurrentWindow().label.replace(/^note-/, '')
  let preferences: NotePreferences = { appearance: { ...INHERITED_APPEARANCE }, view: null }
  let editorBusy = false
  let pickingImage = false
  let idleTimer: ReturnType<typeof setTimeout> | undefined
  let barrierOperations = new Set<string>()
  let appearanceSaving = false
  let appearanceSequence = 0
  let appearanceWrites: Promise<void> = Promise.resolve()
  let confirmedAppearance: NoteAppearance = { ...INHERITED_APPEARANCE }
  let rememberedView: NoteViewState | null = null
  let note: Note | undefined
  let windowState: NoteWindowState | undefined
  let settings: AppSettings = { ...DEFAULT_SETTINGS }
  let fonts: string[] = [DEFAULT_SETTINGS.defaultFontFamily]
  let EditorView: EditorComponent | undefined
  let Toolbar: ToolbarComponent | undefined
  let editorView: InstanceType<EditorComponent> | undefined
  let editorScroll: HTMLDivElement
  let editor: TiptapEditor | undefined
  let moreButton: HTMLButtonElement
  let fileInput: HTMLInputElement
  let saveState: SaveState = 'saved'
  let saveCoordinator: SaveCoordinator<SaveNoteInput, Note> | undefined
  let geometryTimer: ReturnType<typeof setTimeout> | undefined
  let moveTimer: ReturnType<typeof setTimeout> | undefined
  let dockTimer: ReturnType<typeof setTimeout> | undefined
  let pendingContent: Record<string, unknown> | undefined
  let pendingPlainText = ''
  let toolbarVersion = 0
  let tagDraft = ''
  let activePopover: 'opacity' | 'tags' | 'more' | 'format' | 'text' | 'appearance' | null = null
  let textMenu = { x: 0, y: 0 }
  let loading = true
  let toast = ''
  let toastTimer: ReturnType<typeof setTimeout> | undefined
  let collapsed = false
  let locking = false
  let pendingActions = new Set<string>()
  let cleanups: Array<() => void> = []

  $: appearance = resolveAppearance(settings, preferences.appearance)
  $: progress = progressSummary(pendingContent ?? note?.content ?? {})
  $: mode = windowState?.mode ?? 'card'
  $: themeClass = resolveThemeClass(settings.themeId)
  $: backgroundPercent = `${windowState?.backgroundOpacity ?? 100}%`
  $: textPercent = `${windowState?.textOpacity ?? 100}%`
  $: capsulePreview = note ? note.preview.split(/\r?\n/)
    .map((line) => line.trim()).filter(Boolean).filter((line, index) => index > 0 || line !== note?.title)
    .join('\n') : ''
  $: capsuleCompact = [note?.title || '新便签', capsulePreview.split('\n')[0]].filter(Boolean).join(' · ')

  function closeTransient() {
    activePopover = null
  }

  function toggleProgress() {
    if (!editor?.isEditable) return
    editor.view.dispatch(editor.state.tr.setDocAttribute('progressEnabled', !progress.enabled))
    closeTransient()
  }

  function addFirstStep() {
    if (!editor?.isEditable) return
    editor.chain().focus('end').insertContentAt(editor.state.doc.content.size, {
      type: 'taskList', content: [{ type: 'taskItem', attrs: { checked: false }, content: [{ type: 'paragraph' }] }],
    }).focus('end').run()
  }

  function newDesktopNote() {
    void emitDesktop('qingjian://new-note', { sourceNoteId: noteId })
  }

  function showTextContextMenu(event: MouseEvent) {
    if (mode !== 'card' || !editor || editor.state.selection.empty) return
    textMenu = {
      x: Math.max(8, Math.min(event.clientX, window.innerWidth - 188)),
      y: Math.max(8, Math.min(event.clientY, window.innerHeight - 222)),
    }
    activePopover = 'text'
  }

  function formatSelectedText(action: 'bold' | 'italic' | 'underline' | 'strike' | 'paragraph' | 'task' | 'bullet' | 'ordered') {
    if (!editor) return
    const chain = editor.chain().focus()
    if (action === 'bold') chain.toggleBold()
    else if (action === 'italic') chain.toggleItalic()
    else if (action === 'underline') chain.toggleUnderline()
    else if (action === 'strike') chain.toggleStrike()
    else if (action === 'paragraph') chain.setParagraph()
    else if (action === 'task') chain.toggleTaskList()
    else if (action === 'bullet') chain.toggleBulletList()
    else chain.toggleOrderedList()
    chain.run()
    closeTransient()
  }

  function togglePopover(name: Exclude<typeof activePopover, null>) {
    activePopover = activePopover === name ? null : name
    if (name === 'appearance' && fonts.length === 1) void backend.listSystemFonts().then(value => (fonts = value))
  }

  function showToast(message: string, duration = 2400) {
    toast = message
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => (toast = ''), duration)
  }

  async function runNoteAction<T>(key: string, action: () => Promise<T>): Promise<T | undefined> {
    if (pendingActions.has(key) || barrierOperations.size) return
    const freezesEditor = ['destructive-action', 'hide-window', 'switch-mode'].includes(key)
    if (freezesEditor && (editorBusy || pickingImage)) { showToast('请先完成当前输入或图片处理'); return }
    pendingActions = new Set(pendingActions).add(key)
    if (freezesEditor) editor?.setEditable(false, false)
    try {
      return await action()
    } catch (error) {
      showToast(error instanceof Error ? error.message : '操作失败，请重试', 5000)
      return undefined
    } finally {
      const next = new Set(pendingActions)
      next.delete(key)
      pendingActions = next
      if (freezesEditor) editor?.setEditable(!barrierOperations.size, false)
    }
  }

  async function emitDesktop(event: string, payload?: unknown) {
    if (backend.isDesktop) await emit(event, payload)
  }

  async function ensureEditorResources(targetMode: NoteWindowMode) {
    if (targetMode === 'capsule') return
    if (import.meta.env.MODE === 'qa') performance.mark('qingjian:editor-import-start')
    if (!EditorView) EditorView = (await import('./components/Editor.svelte')).default
    if (import.meta.env.MODE === 'qa') performance.mark('qingjian:editor-import-end')
    if (targetMode === 'editor' && !Toolbar) {
      Toolbar = (await import('./components/Toolbar.svelte')).default
      if (fonts.length === 1) fonts = await backend.listSystemFonts()
    }
    await tick()
  }

  function noteInput(): SaveNoteInput | undefined {
    if (!note || !pendingContent) return
    return {
      id: note.id,
      title: documentTitle(pendingPlainText),
      content: pendingContent,
      plainText: pendingPlainText,

    }
  }

  async function flushSave() {
    const input = noteInput()
    if (!input || !saveCoordinator) return
    if (!saveCoordinator.hasPendingChanges) saveCoordinator.queue(input)
    await saveCoordinator.flush()
  }

  function captureView(): NoteViewState | null {
    if (!editor) return rememberedView
    return { anchor: editor.state.selection.anchor, head: editor.state.selection.head, scrollTop: editorScroll?.scrollTop ?? 0 }
  }

  async function rememberView() {
    const view = captureView()
    if (!view) return
    rememberedView = view
    await backend.saveNoteViewState(noteId, view)
  }

  async function restoreView(instance = editor) {
    await tick()
    const view = rememberedView
    if (!instance || !view) return
    const end = instance.state.doc.content.size
    instance.commands.setTextSelection({ from: Math.min(view.anchor, end), to: Math.min(view.head, end) })
    if (editorScroll) editorScroll.scrollTop = view.scrollTop
  }

  async function reportBarrierSave(request: SaveBarrierRequest) {
    if (!request.noteIds.includes(noteId)) return
    try {
      if (loading || !note) throw new Error('便签仍在载入，请稍后重试')
      if (activePopover === 'tags' && tagDraft !== note.tags.map(tag => tag.name).join('、')) throw new Error('请先保存标签或关闭标签编辑')
      if (editorBusy || pickingImage || appearanceSaving || pendingActions.size) throw new Error('便签正在编辑或处理操作，请稍后重试')
      barrierOperations = new Set(barrierOperations).add(request.operationId)
      editor?.setEditable(false, false)
      clearTimeout(idleTimer); idleTimer = undefined
      await flushSave()
      await rememberView()
      await emitDesktop('qingjian://note-save-ready', { operationId: request.operationId, noteId })
    } catch (error) {
      const failure: SaveBarrierFailure = {
        operationId: request.operationId, noteId,
        message: error instanceof Error ? error.message : '未知保存错误',
      }
      await emitDesktop('qingjian://note-save-failed', failure)
    }
  }

  function scheduleIdleRelease() {
    if (!backend.isDesktop || windowState?.visible) { clearTimeout(idleTimer); idleTimer = undefined; return }
    if (idleTimer) return
    idleTimer = setTimeout(() => { idleTimer = undefined; void releaseIdle() }, 60_000)
  }

  async function releaseIdle() {
    if (windowState?.visible || editorBusy || pickingImage || appearanceSaving || pendingActions.size || barrierOperations.size || saveState === 'error') {
      scheduleIdleRelease(); return
    }
    try {
      await flushSave()
      await rememberView()
      if (!saveCoordinator?.hasPendingChanges && !windowState?.visible && !barrierOperations.size && !editorBusy) {
        const released = await backend.releaseIdleNoteWindow(noteId)
        if (!released) scheduleIdleRelease()
      }
    } catch { scheduleIdleRelease() /* Keep the session and retry after a transient failure. */ }
  }

  function scheduleSave(content: Record<string, unknown>, plainText: string) {
    pendingContent = content
    pendingPlainText = plainText
    if (note) note = { ...note, title: documentTitle(plainText), preview: plainText }
    const input = noteInput()
    if (input) saveCoordinator?.queue(input)
  }

  async function saveTags() {
    await runNoteAction('tags', async () => {
      const names = tagDraft.split(/[,，、]/).map(tag => tag.trim().replace(/^#/, '')).filter(Boolean)
      const tags = await backend.setNoteTags(noteId, names)
      if (note) note = { ...note, tags }
      await emitDesktop('qingjian://notes-changed', { noteIds: [noteId], kind: 'metadata' })
      closeTransient()
    })
  }

  function changeAppearance(next: NoteAppearance) {
    if (barrierOperations.size) return
    const sequence = ++appearanceSequence
    preferences = { ...preferences, appearance: next }
    appearanceSaving = true
    appearanceWrites = appearanceWrites.then(async () => {
      try {
        const saved = await backend.setNoteAppearance(noteId, next)
        confirmedAppearance = saved
        if (sequence === appearanceSequence) preferences = { ...preferences, appearance: saved }
        await emitDesktop('qingjian://notes-changed', { noteIds: [noteId], kind: 'appearance' })
      } catch (error) {
        if (sequence === appearanceSequence) {
          preferences = { ...preferences, appearance: confirmedAppearance }
          showToast(error instanceof Error ? error.message : '外观保存失败，已恢复上次设置', 5000)
        }
      } finally { if (sequence === appearanceSequence) appearanceSaving = false }
    })
  }

  function startWindowDrag(event: MouseEvent) {
    if (!backend.isDesktop || event.button !== 0 || windowState?.locked) return
    const target = event.target
    if (target instanceof Element && target.closest('button,input,select,textarea,a,[role="button"],[role="menu"]')) return
    void getCurrentWindow().startDragging()
  }

  function pickImage() {
    if (barrierOperations.size) return
    closeTransient()
    pickingImage = true
    fileInput.click()
  }

  async function switchMode(next: NoteWindowMode) {
    if (!windowState || next === mode) return
    await runNoteAction('switch-mode', async () => {
      if (editorBusy) throw new Error('请先完成当前输入或图片处理')
      await flushSave()
      await rememberView()
      if (next === 'capsule') editor = undefined
      await ensureEditorResources(next)
      windowState = await backend.setNoteWindowMode(noteId, next)
      closeTransient()
      await tick()
      if (next !== 'capsule') {
        await restoreView()
        editorView?.focus()
      }
    })
  }

  async function hideWindow() {
    await runNoteAction('hide-window', async () => {
      if (editorBusy) throw new Error('请先完成当前输入或图片处理')
      await flushSave()
      await rememberView()
      await backend.saveNoteWindowGeometry(noteId)
      await backend.hideNoteWindow(noteId)
      if (windowState) windowState = { ...windowState, visible: false }
      scheduleIdleRelease()
    })
  }

  async function openAppSettings() {
    closeTransient()
    try {
      await backend.showHubWindow()
      await emitDesktop('qingjian://settings')
    } catch (error) {
      showToast(error instanceof Error ? error.message : '设置打开失败')
    }
  }

  async function toggleAlwaysOnTop() {
    if (!windowState) return
    windowState = await backend.setNoteWindowAlwaysOnTop(noteId, !windowState.alwaysOnTop)
  }

  async function setOpacity(layer: 'background' | 'text', value: number) {
    if (!windowState) return
    windowState = await backend.setNoteWindowOpacity(noteId, layer, value)
  }

  async function toggleLock() {
    if (!windowState || locking) return
    if (windowState.locked) {
      windowState = await backend.setNoteWindowLocked(noteId, false)
      showToast('便签已解锁')
      return
    }
    locking = true
    await flushSave()
    showToast('即将锁定并启用鼠标穿透 · Ctrl+Alt+L 可解锁', 2100)
    await new Promise((resolve) => setTimeout(resolve, 2000))
    windowState = await backend.setNoteWindowLocked(noteId, true)
    locking = false
  }

  async function toggleContentPin() {
    if (!note) return
    await runNoteAction('pin', async () => {
      const pinned = !note!.pinned
      await backend.setNotePinned(noteId, pinned)
      if (note) note = { ...note, pinned }
      closeTransient()
      await emitDesktop('qingjian://notes-changed', { noteIds: [noteId], kind: 'metadata' })
    })
  }

  async function archiveCurrent() {
    closeTransient()
    if (!note) return
    await runNoteAction('destructive-action', async () => {
      await flushSave()
      await rememberView()
      await backend.setNoteArchived(note!.id, true)
    })
  }

  async function deleteCurrent() {
    closeTransient()
    if (!note) return
    const approved = backend.isDesktop
      ? await confirm(`将“${note.title}”移到回收站？之后仍可恢复。`, {
          title: '移到回收站', kind: 'warning', okLabel: '移到回收站', cancelLabel: '取消',
        })
      : window.confirm(`将“${note.title}”移到回收站？`)
    if (!approved) return
    await runNoteAction('destructive-action', async () => {
      await flushSave()
      await rememberView()
      await backend.deleteNote(note!.id)
    })
  }

  async function clipboardContent() {
    if (!editor) return { text: '', html: '', markdown: '' }
    const { DOMSerializer } = await import('@tiptap/pm/model')
    const { selection, doc, schema } = editor.state
    const fragment = selection.empty ? doc.content : selection.content().content
    const wrapper = document.createElement('div')
    wrapper.appendChild(DOMSerializer.fromSchema(schema).serializeFragment(fragment))
    const exportDocument = selection.empty ? editor.getJSON() : {
      type: 'doc', attrs: { progressEnabled: progress.enabled }, content: fragment.toJSON(),
    }
    return {
      text: progress.enabled ? progressPlainText(exportDocument) : selection.empty ? editor.getText() : doc.textBetween(selection.from, selection.to, '\n'),
      html: richHtml(wrapper.innerHTML, appearance, progress.enabled),
      markdown: serializeMarkdown(exportDocument),
    }
  }

  async function copyContent(kind: 'rich' | 'text' | 'markdown' | 'html') {
    closeTransient()
    const content = await clipboardContent()
    try {
      if (kind === 'rich' && 'ClipboardItem' in window) {
        await navigator.clipboard.write([new ClipboardItem({
          'text/plain': new Blob([content.text], { type: 'text/plain' }),
          'text/html': new Blob([content.html], { type: 'text/html' }),
        })])
      } else {
        await navigator.clipboard.writeText(kind === 'html' ? content.html : kind === 'markdown' ? content.markdown : content.text)
      }
      showToast(kind === 'rich' ? '已复制并保留格式' : kind === 'markdown' ? '已复制 Markdown' : kind === 'html' ? '已复制 HTML' : '已复制纯文本')
    } catch { showToast('复制失败，请检查剪贴板权限') }
  }

  async function exportCurrent(kind: 'markdown' | 'html' | 'pdf') {
    if (!note || !editor) return
    if (editorBusy) { showToast('请先完成当前输入或图片处理'); return }
    await flushSave()
    closeTransient()
    const extension = kind === 'markdown' ? 'md' : kind
    const destination = backend.isDesktop ? await save({
      title: kind === 'markdown' ? '导出 Markdown' : kind === 'html' ? '导出 HTML' : '导出 PDF',
      defaultPath: `${note.title}.${extension}`,
      filters: [{ name: `${extension.toUpperCase()} 文档`, extensions: [extension] }],
    }) : null
    if (!destination) return
    await runNoteAction('export', async () => {
      if (kind === 'markdown') await backend.exportMarkdown(destination, serializeMarkdown(editor!.getJSON()))
      else if (kind === 'html') await backend.exportHtml(note!.id, destination, richHtml(editor!.getHTML(), appearance, progress.enabled))
      else await backend.exportPdf(note!.id, destination)
      showToast('导出完成')
    })
  }

  async function collapseDocked() {
    if (!settings.autoHideAtScreenEdge || !windowState?.dockEdge || windowState.locked || collapsed || editorBusy || barrierOperations.size || activePopover) return
    await backend.collapseDockedNoteWindow(noteId)
    collapsed = true
  }

  async function expandDocked() {
    clearTimeout(dockTimer)
    if (!collapsed) return
    await backend.expandDockedNoteWindow(noteId)
    collapsed = false
  }

  function scheduleDockCollapse() {
    clearTimeout(dockTimer)
    if (!settings.autoHideAtScreenEdge) return
    dockTimer = setTimeout(() => void collapseDocked(), 350)
  }

  function handleWindowBlur() {
    closeTransient()
    scheduleDockCollapse()
  }

  function handleWindowPointerDown(event: PointerEvent) {
    if (!activePopover) return
    const target = event.target
    if (!(target instanceof Element) || !target.closest('[data-transient-root]')) closeTransient()
  }

  function handleKeyboard(event: KeyboardEvent) {
    if (event.isComposing || barrierOperations.size) return
    if (event.ctrlKey && !event.altKey) {
      const key = event.key.toLowerCase()
      if (key === 'w') { event.preventDefault(); void hideWindow(); return }
      if (key === 's') { event.preventDefault(); void runNoteAction('save', flushSave); return }
      if (key === 'n') { event.preventDefault(); newDesktopNote(); return }
      if (key === ',') { event.preventDefault(); void openAppSettings(); return }
    }
    if (event.key === 'Escape') {
      if (activePopover) { event.preventDefault(); closeTransient(); return }
      if (dismissTooltip()) { event.preventDefault(); return }
      if (mode === 'editor') { event.preventDefault(); void switchMode('card') }
    }
  }

  onMount(() => {
    let disposed = false
    void (async () => {
      try {
        ;[note, settings, windowState, preferences] = await Promise.all([
          backend.getNote(noteId), backend.getSettings(), backend.getNoteWindowState(noteId), backend.getNotePreferences(noteId),
        ])
        confirmedAppearance = preferences.appearance
        rememberedView = preferences.view
        pendingPlainText = note.preview
        tagDraft = note.tags.map((tag) => tag.name).join('、')
        saveCoordinator = new SaveCoordinator<SaveNoteInput, Note>({
          delay: 500,
          save: (input) => backend.saveNote(input),
          onStateChange: (state, error) => {
            saveState = state
            if (state === 'error') {
              showToast(error instanceof Error ? error.message : '保存失败', 5000)
              if (backend.isDesktop && windowState && !windowState.visible) void backend.openNoteWindow(noteId).catch(() => undefined)
            }
          },
          onSaved: (saved, _revision, isLatest) => {
            if (isLatest) {
              if (note) note = { ...note, title: saved.title, preview: saved.preview, updatedAt: saved.updatedAt, content: saved.content }
              pendingContent = undefined
              void emitDesktop('qingjian://notes-changed', { noteIds: [noteId], kind: 'content' })
            }
          },
        })
        if (import.meta.env.MODE === 'qa') performance.mark('qingjian:note-data-ready')
        loading = false
        await ensureEditorResources(windowState.mode)
        if (windowState.mode !== 'capsule') await tick()
        if (backend.isDesktop) {
          const current = getCurrentWindow()
          cleanups.push(
            await current.onMoved(() => {
              clearTimeout(moveTimer)
              moveTimer = setTimeout(async () => {
                if (disposed || collapsed) return
                windowState = await backend.finalizeNoteWindowMove(noteId)
              }, 150)
            }),
            await current.onResized(() => {
              clearTimeout(geometryTimer)
              geometryTimer = setTimeout(() => {
                if (!disposed && !collapsed) void backend.saveNoteWindowGeometry(noteId)
              }, 180)
            }),
            await listen('qingjian://hide-requested', () => void hideWindow()),
            await listen<AppSettings>('qingjian://settings-changed', (event) => {
              settings = event.payload
              if (!settings.autoHideAtScreenEdge) void expandDocked()
            }),
            await listen<NoteWindowState>('qingjian://window-state-changed', (event) => {
              void (async () => {
                if (event.payload.mode !== mode) {
                  await flushSave()
                  await ensureEditorResources(event.payload.mode)
                }
                windowState = event.payload
              })()
            }),
            await listen<SaveBarrierRequest>('qingjian://prepare-save', event => void reportBarrierSave(event.payload)),
            await listen<{ operationId: string }>('qingjian://release-save', event => {
              const next = new Set(barrierOperations); next.delete(event.payload.operationId); barrierOperations = next
              editor?.setEditable(!next.size, false)
              if (saveState === 'error' && windowState && !windowState.visible) void backend.openNoteWindow(noteId).catch(() => undefined)
              scheduleIdleRelease()
            }),
            await listen('qingjian://note-windows-changed', () => {
              void backend.getNoteWindowState(noteId).then(state => { windowState = state; scheduleIdleRelease() }).catch(() => undefined)
            }),
            await listen<NoteChange>('qingjian://notes-changed', event => {
              const change = event.payload
              if (!change || !change.noteIds?.includes(noteId) || change.kind !== 'metadata') return
              void backend.getNote(noteId).then(latest => {
                if (note) note = { ...note, pinned: latest.pinned, tags: latest.tags, archived: latest.archived, folderId: latest.folderId }
              }).catch(() => undefined)
            }),
          )
        }
        windowState = await backend.noteWindowReady(noteId)
        scheduleIdleRelease()
      } catch (error) {
        showToast(error instanceof Error ? error.message : '便签打开失败')
      } finally {
        loading = false
      }
    })()
    return () => { disposed = true }
  })

  onDestroy(() => {
    saveCoordinator?.dispose()
    clearTimeout(geometryTimer); clearTimeout(moveTimer); clearTimeout(dockTimer); clearTimeout(toastTimer); clearTimeout(idleTimer)
    cleanups.forEach((cleanup) => cleanup())
  })
</script>

<svelte:window on:keydown={handleKeyboard} on:blur={handleWindowBlur} on:pointerdown={handleWindowPointerDown} />

<main
  class={`note-window-root mode-${mode} ${themeClass}`}
  style={`--note-bg-percent:${backgroundPercent};--note-text-percent:${textPercent}`}
  class:collapsed
  on:mouseenter={() => void expandDocked()}
  on:mouseleave={scheduleDockCollapse}
>
  {#if collapsed}
    <button class={`dock-strip edge-${windowState?.dockEdge || 'left'}`} aria-label="展开停靠便签" on:click={() => void expandDocked()}><span></span></button>
  {:else if loading}
    <div class="note-loading">轻</div>
  {:else if note && windowState}
    {#if mode === 'capsule'}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <section class="capsule-shell" inert={barrierOperations.size > 0} on:dblclick={() => void switchMode('card')}>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="capsule-drag" on:mousedown={startWindowDrag}>
          <span class:error={saveState === 'error'} class="capsule-save-state" aria-live="polite">
            {saveState === 'saving' ? '保存中' : saveState === 'dirty' ? '未保存' : saveState === 'error' ? '保存失败' : '已保存'}
          </span>
          <div class:compact={settings.capsuleDisplay === 'compact'} class="capsule-copy">
            {#if settings.capsuleDisplay === 'compact'}
              <strong>{capsuleCompact}</strong>
            {:else}
              <strong>{note.title || '新便签'}</strong>
              {#if capsulePreview}<span>{capsulePreview}</span>{/if}
            {/if}
          </div>
        </div>
        <button class="capsule-expand" aria-label="展开为卡片" use:tooltip={'展开为卡片'} on:click={() => void switchMode('card')}><Maximize2 /></button>
        <button class="capsule-hide" aria-label="隐藏便签" use:tooltip={'隐藏便签'} on:click={() => void hideWindow()}><X /></button>
      </section>
    {:else}
      <section class="desktop-note-shell" inert={barrierOperations.size > 0}>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <header class="note-window-titlebar" on:mousedown={startWindowDrag}>
          <button class="note-mode-label" aria-label={mode === 'editor' ? '收起编辑工具' : '展开完整编辑'} use:tooltip={mode === 'editor' ? '收起编辑工具' : '展开完整编辑'} on:click={() => void switchMode(mode === 'editor' ? 'card' : 'editor')}>{#if mode === 'editor'}<Minimize2 />{:else}<Maximize2 />{/if}</button>
          <div class="window-note-actions">
            <button aria-label="新建桌面便签" use:tooltip={'新建桌面便签（Ctrl+N）'} on:click={newDesktopNote}><Plus /></button>
            <span class:error={saveState === 'error'} class="compact-save-state">
              {saveState === 'saving' ? '保存中' : saveState === 'dirty' ? '未保存' : saveState === 'error' ? '保存失败' : '已保存'}
            </span>
            {#if saveState === 'error'}<button class="save-retry" on:click={() => void flushSave()}>重试</button>{/if}
            <button class:active={windowState.alwaysOnTop} aria-label={windowState.alwaysOnTop ? '取消始终置于最前' : '始终置于最前'} aria-pressed={windowState.alwaysOnTop} use:tooltip={windowState.alwaysOnTop ? '取消始终置于最前' : '始终置于最前'} on:click={toggleAlwaysOnTop}>
              {#if windowState.alwaysOnTop}<PinOff />{:else}<Pin />{/if}
            </button>
            <div class="note-popover-anchor" data-transient-root>
              <button bind:this={moreButton} class:active={Boolean(activePopover && activePopover !== 'format')} aria-label="更多" aria-haspopup="menu" aria-expanded={Boolean(activePopover)} on:click={() => togglePopover('more')}><MoreHorizontal />{#if note.tags.length}<span class="tag-count">{note.tags.length}</span>{/if}</button>
              {#if activePopover === 'more'}
                <div class="note-more-menu" role="menu" aria-label="便签操作" use:menu={{ onClose: closeTransient, returnFocus: moreButton }}>
                  <small>显示方式</small>
                  <button on:click={() => void switchMode('capsule')}>胶囊浏览</button>
                  <button on:click={() => void switchMode('card')}>卡片编辑</button>
                  <button on:click={() => void switchMode('editor')}>完整编辑</button>
                  <small>整理与窗口</small>
                  <button on:click={() => formatSelectedText('task')}><CircleCheckBig />待办清单</button>
                  <button aria-pressed={progress.enabled} on:click={toggleProgress}><CircleCheckBig />{progress.enabled ? '收起进展' : '记录进展'}</button>
                  <button on:click={toggleContentPin}><Star />{note.pinned ? '取消列表置顶' : '列表置顶'}</button>
                  <button on:click={() => togglePopover('appearance')}><Text />文字与待办外观</button>
                  <button on:click={() => (activePopover = 'tags')}><Tags />编辑标签</button>
                  <button on:click={() => (activePopover = 'opacity')}><Blend />背景与文字透明度</button>
                  <button disabled={locking} on:click={toggleLock}>{#if windowState.locked}<Unlock />解锁鼠标穿透{:else}<LockKeyhole />锁定并启用鼠标穿透{/if}</button>
                  <button on:click={pickImage}><ImagePlus />插入图片</button>
                    <small>复制内容</small>
                    <button on:click={() => void copyContent('rich')}><Copy />保留源格式</button>
                    <button on:click={() => void copyContent('text')}><Text />纯文本</button>
                    <button on:click={() => void copyContent('markdown')}><Code2 />Markdown</button>
                    <button on:click={() => void copyContent('html')}><Code2 />HTML 源码</button>
                    <small>导出</small>
                    <button on:click={() => void exportCurrent('markdown')}><FileDown />Markdown</button>
                    <button on:click={() => void exportCurrent('html')}><FileDown />HTML</button>
                    <button on:click={() => void exportCurrent('pdf')}><Download />PDF</button>
                  <button on:click={() => void archiveCurrent()}><Archive />归档便签</button>
                  <button on:click={() => void openAppSettings()}><Settings />打开设置</button>
                  <button class="danger" on:click={() => void deleteCurrent()}><Trash2 />移到回收站</button>
                </div>
              {/if}
              {#if activePopover === 'appearance'}
                <div class="appearance-popover" role="dialog" aria-label="文字与待办外观" use:popover={{ onClose: closeTransient, returnFocus: moreButton }}>
                  <strong>文字与待办</strong>
                  <p>未自定义的项目跟随全局默认</p>
                  <fieldset aria-busy={appearanceSaving}>
                    <label>字体<select value={preferences.appearance.fontFamily ?? ''} on:change={event => void changeAppearance({ ...preferences.appearance, fontFamily: event.currentTarget.value || null })}>
                      <option value="">跟随默认</option>{#each fonts as font}<option value={font}>{font}</option>{/each}
                    </select></label>
                    <label>字号<select value={preferences.appearance.fontSize ?? ''} on:change={event => void changeAppearance({ ...preferences.appearance, fontSize: event.currentTarget.value ? Number(event.currentTarget.value) : null })}>
                      <option value="">跟随默认</option>{#each [12,14,16,18,20,24,28,32,48,72] as size}<option value={size}>{size}px</option>{/each}
                    </select></label>
                    <label>行距<select value={preferences.appearance.lineHeight ?? ''} on:change={event => void changeAppearance({ ...preferences.appearance, lineHeight: event.currentTarget.value ? Number(event.currentTarget.value) : null })}>
                      <option value="">跟随默认</option><option value="1.4">紧凑</option><option value="1.55">舒适</option><option value="1.7">宽松</option>
                    </select></label>
                    <label>待办圆圈<select value={preferences.appearance.taskMarkerSize ?? ''} on:change={event => void changeAppearance({ ...preferences.appearance, taskMarkerSize: event.currentTarget.value === '' ? null : Number(event.currentTarget.value) })}>
                      <option value="">跟随默认</option><option value="0">随字号缩放</option>{#each Array.from({ length: 25 }, (_, i) => i + 12) as size}<option value={size}>{size}px</option>{/each}
                    </select></label>
                    <button class="appearance-reset" on:click={() => void changeAppearance({ ...INHERITED_APPEARANCE })}>全部恢复跟随默认</button>
                  </fieldset>
                </div>
              {:else if activePopover === 'opacity'}
                <div class="opacity-popover" role="dialog" aria-label="透明度" use:popover={{ onClose: closeTransient, returnFocus: moreButton }}>
                  <label>
                    <span><strong>背景</strong><b>{windowState.backgroundOpacity}%</b></span>
                    <input type="range" min="0" max="100" step="5" value={windowState.backgroundOpacity}
                      on:input={(event) => void setOpacity('background', Number(event.currentTarget.value))} />
                    <i>{#each [0, 40, 70, 100] as value}<button on:click={() => void setOpacity('background', value)}>{value}</button>{/each}</i>
                  </label>
                  <label>
                    <span><strong>正文文字</strong><b>{windowState.textOpacity}%</b></span>
                    <input type="range" min="20" max="100" step="5" value={windowState.textOpacity}
                      on:input={(event) => void setOpacity('text', Number(event.currentTarget.value))} />
                    <i>{#each [20, 60, 80, 100] as value}<button on:click={() => void setOpacity('text', value)}>{value}</button>{/each}</i>
                  </label>
                </div>
              {:else if activePopover === 'tags'}
                <div class="tag-popover" role="dialog" aria-label="便签标签" use:popover={{ onClose: closeTransient, returnFocus: moreButton }}>
                  <strong>便签标签</strong>
                  <input aria-label="便签标签" placeholder="输入标签，用逗号分隔" bind:value={tagDraft}
                    on:keydown={(event) => event.key === 'Enter' && void saveTags()} />
                  <small>最多 12 个，按 Enter 保存</small>
                  <button class="tag-save" on:click={() => void saveTags()}>保存标签</button>
                </div>
              {/if}
            </div>
            <button class="note-close" aria-label="隐藏便签" disabled={pendingActions.has('hide-window')} use:tooltip={'隐藏便签'} on:click={() => void hideWindow()}><X /></button>
          </div>
        </header>

        {#if mode === 'editor' && Toolbar}
          <svelte:component this={Toolbar} {editor} {fonts} defaultFontSize={appearance.fontSize} version={toolbarVersion}
            menuOpen={activePopover === 'format'}
            onMenuOpenChange={(open: boolean) => (activePopover = open ? 'format' : null)}
            onPickImage={pickImage} />
        {/if}
        {#if progress.enabled}
          <div class="progress-summary" role="status" aria-live="polite" aria-atomic="true">
            <div><strong>已完成 {progress.done}／{progress.total} 步</strong>{#if progress.blocked}<span>{progress.blocked} 项受阻</span>{/if}</div>
            {#if !progress.total}<button on:click={addFirstStep}>添加第一步</button>
            {:else if progress.done === progress.total}<p>全部完成</p>
            {:else if progress.current}<p title={progress.current}><span>{progress.doing ? '进行中' : '下一步'} · {progress.current}</span>{#if progress.doing > 1}<span class="parallel-count">另 {progress.doing - 1} 项进行中</span>{/if}</p>
            {:else}<p>等待解除受阻</p>{/if}
          </div>
        {/if}
        <div bind:this={editorScroll} class="note-window-editor" class:card-editor={mode === 'card'} use:quietScroll>
          {#if EditorView}
            <svelte:component
              this={EditorView} bind:this={editorView} noteId={note.id} content={note.content}
              defaultFontFamily={appearance.fontFamily} defaultFontSize={appearance.fontSize}
              defaultLineHeight={appearance.lineHeight} taskMarkerSize={appearance.taskMarkerSize}
              onChange={scheduleSave}
              onReady={(instance: TiptapEditor) => {
                editor = instance
                void restoreView(instance).then(() => {
                  if (!windowState?.locked) instance.commands.focus(undefined, { scrollIntoView: false })
                })
              }}
              onBusyChange={(busy: boolean) => (editorBusy = busy)}
              onSelectionChange={() => (toolbarVersion += 1)} onError={showToast}
              onTextContextMenu={showTextContextMenu}
            />
          {/if}
        </div>
        {#if activePopover === 'text'}
          <div role="menu" aria-label="文字格式" use:menu={{ onClose: closeTransient }} class="text-context-menu" data-transient-root tabindex="-1"
            style={`left:${textMenu.x}px;top:${textMenu.y}px`} on:mousedown|preventDefault|stopPropagation>
            <small>文字格式</small>
            <div>
              <button class:active={editor?.isActive('bold')} on:click={() => formatSelectedText('bold')}><Bold />粗体</button>
              <button class:active={editor?.isActive('italic')} on:click={() => formatSelectedText('italic')}><Italic />斜体</button>
              <button class:active={editor?.isActive('underline')} on:click={() => formatSelectedText('underline')}><Underline />下划线</button>
              <button class:active={editor?.isActive('strike')} on:click={() => formatSelectedText('strike')}><Strikethrough />删除线</button>
            </div>
            <small>文本类型</small>
            <div>
              <button on:click={() => formatSelectedText('paragraph')}><Pilcrow />正文</button>
              <button on:click={() => formatSelectedText('task')}><CircleCheckBig />复选框</button>
              <button on:click={() => formatSelectedText('bullet')}><List />项目符号</button>
              <button on:click={() => formatSelectedText('ordered')}><ListOrdered />编号列表</button>
            </div>
          </div>
        {/if}
      </section>
    {/if}
  {/if}
</main>

<input bind:this={fileInput} class="hidden-file-input" type="file"
  accept="image/png,image/jpeg,image/webp,image/gif,image/bmp" multiple
  on:cancel={() => (pickingImage = false)}
  on:change={(event) => {
    pickingImage = false
    const files = Array.from(event.currentTarget.files ?? [])
    event.currentTarget.value = ''
    if (barrierOperations.size) { showToast('正在安全保存便签，请稍后插入图片'); return }
    if (files.length) void editorView?.insertFiles(files)
  }} />

{#if barrierOperations.size}<div class="note-save-overlay" role="status">正在保存便签…</div>{/if}
{#if toast}<div class="toast note-window-toast" role="status">{toast}</div>{/if}

<style>
  :global(html), :global(body), :global(#app) { width: 100%; height: 100%; overflow: hidden; }
  :global(#app) { padding: 0; }
  .note-window-root { display:block; width:100%; height:100%; padding:7px; box-sizing:border-box; color:var(--text, #18181b); background:transparent; }
  .note-window-root.collapsed { padding: 0; }
  .dock-strip { position:relative; width:100%; height:100%; padding:0; border:1px solid color-mix(in srgb, var(--text, #18181b), transparent 82%); background:color-mix(in srgb, var(--paper, #ffffff) 88%, transparent); box-shadow:0 2px 10px rgba(15,23,42,.12); backdrop-filter:blur(10px); cursor:pointer; }
  .dock-strip span { position:absolute; border-radius:999px; background:color-mix(in srgb, var(--text, #18181b), transparent 58%); }
  .dock-strip.edge-left span, .dock-strip.edge-right span { width:3px; height:34px; top:50%; left:50%; transform:translate(-50%,-50%); }
  .dock-strip.edge-top span, .dock-strip.edge-bottom span { width:34px; height:3px; top:50%; left:50%; transform:translate(-50%,-50%); }
  .note-loading { display:grid; place-items:center; height:100%; border:1px solid var(--line); border-radius:var(--radius-window); background:var(--paper, #ffffff); color:var(--accent, #52657d); font-weight:700; }
  .capsule-shell, .desktop-note-shell { width:100%; height:100%; box-sizing:border-box; background:color-mix(in srgb, var(--paper, #ffffff) var(--note-bg-percent), transparent); border:1px solid color-mix(in srgb, var(--line), var(--text) 8%); box-shadow:var(--shadow-note); overflow:hidden; }
  .capsule-shell { border-radius:999px; display:flex; align-items:center; padding:6px 8px 6px 14px; gap:9px; backdrop-filter:blur(14px); }
  .capsule-drag { min-width:0; flex:1; display:flex; align-items:center; gap:10px; cursor:grab; }
  .capsule-drag:active { cursor:grabbing; }
  .capsule-save-state { flex:none; color:var(--muted); font-size:9px; line-height:14px; }
  .capsule-save-state.error { color:var(--danger); font-weight:650; }
  .capsule-copy { min-width:0; flex:1; display:grid; align-content:center; gap:1px; overflow:hidden; color:color-mix(in srgb, var(--text, #18181b) var(--note-text-percent), transparent); }
  .capsule-copy strong { min-width:0; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; font-size:12px; line-height:16px; font-weight:650; }
  .capsule-copy span { display:-webkit-box; overflow:hidden; color:color-mix(in srgb, var(--muted) var(--note-text-percent), transparent); -webkit-box-orient:vertical; -webkit-line-clamp:2; line-clamp:2; white-space:pre-wrap; font-size:11px; line-height:14px; }
  .capsule-copy.compact strong { font-size:12px; font-weight:560; }
  .capsule-expand { flex:none; width:30px; height:30px; display:grid; place-items:center; border:0; border-radius:999px; background:color-mix(in srgb, var(--accent-soft), var(--paper) 32%); color:var(--text); cursor:pointer; }
  .capsule-expand :global(svg), .note-mode-label :global(svg) { width:15px; height:15px; }
  .capsule-hide { flex:none; display:grid; place-items:center; width:28px; height:28px; padding:4px; border-radius:50%; background:transparent; color:var(--muted); cursor:pointer; }
  .capsule-hide :global(svg) { width:14px; height:14px; }
  .capsule-hide:hover { background:var(--accent-soft); color:var(--text); }
  .capsule-expand:hover { background:var(--accent-soft); }
  .desktop-note-shell { border-radius:var(--radius-window); display:flex; flex-direction:column; }
  .note-window-titlebar { min-height:39px; display:flex; align-items:center; justify-content:space-between; padding:0 6px 0 9px; border-bottom:1px solid var(--line); background:color-mix(in srgb, var(--paper, #ffffff) 78%, transparent); cursor:grab; }
  .note-mode-label { color:var(--muted); width:30px; height:30px; display:grid; place-items:center; border-radius:var(--radius-control); background:transparent; cursor:pointer; }
  .progress-summary { flex:none; margin:0 14px; padding:7px 0; border-bottom:1px solid var(--line); font-size:11px; color:var(--muted); }
  .progress-summary > div { display:flex; justify-content:space-between; gap:8px; }
  .progress-summary strong { font-weight:600; color:var(--text); }
  .progress-summary p { display:flex; gap:6px; margin:4px 0 0; }
  .progress-summary p > span { min-width:0; overflow:hidden; white-space:nowrap; text-overflow:ellipsis; }
  .progress-summary p > .parallel-count { flex:none; }
  .progress-summary button { min-height:24px; padding:2px 0; margin-top:2px; background:transparent; color:var(--text); text-decoration:underline; cursor:pointer; }
  .note-mode-label:hover { background:var(--accent-soft); color:var(--text); }
  .note-window-titlebar:active { cursor:grabbing; }
  .mode-switcher { display:flex; padding:2px; border-radius:9px; background:color-mix(in srgb, var(--sidebar), transparent 20%); }
  .mode-switcher button { border:0; background:transparent; min-height:28px; padding:5px 8px; border-radius:7px; color:var(--muted); font-size:12px; }
  .mode-switcher button.active { background:var(--paper); color:var(--text); box-shadow:0 1px 4px rgba(57,44,12,.12); }
  .window-note-actions { display:flex; align-items:center; gap:1px; }
  .window-note-actions > button, .note-popover-anchor > button { width:30px; height:30px; border:0; border-radius:var(--radius-control); display:grid; place-items:center; background:transparent; color:var(--muted); }
  .window-note-actions button:hover, .window-note-actions button.active { background:var(--accent-soft); color:var(--text); }
  .window-note-actions :global(svg) { width:14px; height:14px; }
  .tag-count { position:absolute; right:2px; top:2px; min-width:11px; height:11px; padding:0 2px; box-sizing:border-box; border-radius:999px; background:var(--accent); color:var(--paper); font-size:7px; line-height:11px; }
  .compact-save-state { min-width:38px; text-align:right; margin-right:3px; font-size:11px; color:var(--muted); }
  .compact-save-state.error { color:var(--danger); }
  .save-retry { width:auto !important; padding:0 6px !important; color:var(--danger) !important; font-size:10px !important; }
  .note-popover-anchor { position:relative; }
  .opacity-popover, .tag-popover, .appearance-popover, .note-more-menu { position:fixed; z-index:30; right:8px; top:44px; max-width:calc(100vw - 16px); border:1px solid var(--line); border-radius:var(--radius-panel); background:var(--paper, #ffffff); color:var(--text, #18181b); box-shadow:var(--shadow-menu); }
  .opacity-popover { width:220px; padding:12px; display:grid; gap:13px; }
  .opacity-popover label { display:grid; gap:7px; }
  .opacity-popover label > span { display:flex; justify-content:space-between; align-items:center; font-size:12px; }
  .opacity-popover label > span b { color:var(--muted, #6b7280); font-size:12px; }
  .opacity-popover input { width:100%; margin:0; accent-color:var(--accent); }
  .opacity-popover i { display:flex; gap:5px; font-style:normal; }
  .opacity-popover i button { flex:1; min-height:28px; border:1px solid var(--line); border-radius:7px; background:var(--sidebar); color:var(--text); padding:4px; font-size:12px; }
  .tag-popover { width:220px; padding:12px; display:grid; gap:8px; }
  .tag-popover strong { font-size:12px; }
  .tag-popover input { width:100%; box-sizing:border-box; border:1px solid var(--line); border-radius:var(--radius-control); background:var(--sidebar); color:inherit; padding:8px 9px; outline:0; }
  .tag-popover input:focus { border-color:var(--accent); box-shadow:0 0 0 3px color-mix(in srgb, var(--accent), transparent 86%); }
  .tag-popover small { color:var(--muted, #6b7280); font-size:11px; }
  .tag-popover .tag-save { justify-self:end; min-height:28px; border:0; border-radius:7px; background:var(--accent); color:var(--paper); padding:5px 10px; }
  .note-more-menu { width:180px; max-height:min(480px, calc(100vh - 48px)); overflow:auto; padding:6px; }
  .note-more-menu button { width:100%; display:flex; align-items:center; gap:8px; border:0; border-radius:var(--radius-control); background:transparent; padding:8px; color:var(--text); font-size:12px; text-align:left; }
  .note-more-menu button:hover { background:var(--sidebar); }
  .note-more-menu button.danger { color:var(--danger); }
  .note-more-menu small { display:block; padding:7px 8px 3px; color:var(--muted); font-size:9px; text-transform:uppercase; }
  .note-window-editor { min-height:0; flex:1; overflow:auto; }
  .note-window-root :global(.format-toolbar) { background:color-mix(in srgb, var(--paper) 84%, transparent); }
  .note-window-editor :global(.editor-host) { min-height:100%; }
  .note-window-editor :global(.tiptap) { min-height:100%; box-sizing:border-box; padding:14px 16px 24px; outline:0; color:color-mix(in srgb, var(--text, #18181b) var(--note-text-percent), transparent); caret-color:var(--text, #18181b); }
  .note-window-editor :global(.tiptap img) { opacity:1; }
  .card-editor :global(.tiptap) { padding-top:12px; }
  .text-context-menu { position:fixed; z-index:42; width:180px; padding:6px; border:1px solid var(--window-edge); border-radius:var(--radius-panel); background:var(--surface-raised, var(--paper)); color:var(--text); box-shadow:var(--shadow-menu); }
  .text-context-menu small { display:block; padding:5px 7px 3px; color:var(--muted); font-size:10px; font-weight:650; }
  .text-context-menu > div { display:grid; grid-template-columns:1fr 1fr; gap:2px; }
  .text-context-menu button { min-height:32px; display:flex; align-items:center; gap:6px; padding:0 7px; border:0; border-radius:7px; background:transparent; color:var(--text); cursor:pointer; font-size:11px; text-align:left; }
  .text-context-menu button:hover, .text-context-menu button.active { background:var(--accent-soft); }
  .text-context-menu :global(svg) { width:13px; height:13px; flex:none; }
  .mode-card .note-window-titlebar { min-height:36px; }
  @media (max-width: 330px), (max-height: 210px) {
    .note-window-root { padding:4px; }
    .note-window-titlebar { padding-left:5px; }
    .mode-switcher button { padding:4px 5px; font-size:9px; }
    .window-note-actions > button, .note-popover-anchor > button { width:24px; height:24px; }
    .compact-save-state:not(.error) { display:none; }
    .card-editor :global(.tiptap) { padding:9px 10px 14px; }
  }
  @media (max-height: 210px) {
    .opacity-popover, .tag-popover, .appearance-popover, .note-more-menu { top:8px; max-height:calc(100vh - 16px); overflow:auto; }
  }

  .appearance-popover { width:252px; padding:14px; max-height:calc(100vh - 54px); overflow:auto; }
  .appearance-popover > strong { font-size:14px; }
  .appearance-popover p { margin:5px 0 12px; color:var(--muted); font-size:12px; line-height:1.5; }
  .appearance-popover fieldset { display:grid; gap:10px; border:0; margin:0; padding:0; }
  .appearance-popover label { display:grid; grid-template-columns:64px minmax(0,1fr); align-items:center; gap:8px; font-size:12px; }
  .appearance-popover select { min-width:0; width:100%; min-height:32px; padding:4px 6px; border:1px solid var(--line); border-radius:var(--radius-control); background:var(--paper); color:var(--text); }
  .appearance-reset { min-height:32px; border:1px solid var(--line); border-radius:var(--radius-control); background:var(--sidebar); color:var(--text); font-size:12px; cursor:pointer; }
  .note-save-overlay { position:fixed; inset:8px; z-index:60; display:grid; place-items:center; border-radius:var(--radius-window); background:color-mix(in srgb,var(--paper) 94%,transparent); color:var(--text); font-size:13px; }

</style>
