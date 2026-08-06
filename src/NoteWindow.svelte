<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte'
  import type { Editor as TiptapEditor } from '@tiptap/core'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { emit, listen } from '@tauri-apps/api/event'
  import { confirm, save } from '@tauri-apps/plugin-dialog'
  import {
    Archive, Code2, Copy, Download, EyeOff, FileDown, ImagePlus, LockKeyhole,
    MoreHorizontal, Pin, PinOff, Settings, Text, Trash2, Unlock, X,
  } from 'lucide-svelte'
  import { backend } from './lib/backend'
  import { documentTitle } from './lib/note'
  import { serializeMarkdown } from './lib/markdown'
  import { DEFAULT_SETTINGS } from './lib/types'
  import type {
    AppSettings, Note, NoteWindowMode, NoteWindowState, SaveNoteInput, SaveState,
  } from './lib/types'

  type EditorComponent = typeof import('./components/Editor.svelte').default
  type ToolbarComponent = typeof import('./components/Toolbar.svelte').default

  const query = new URLSearchParams(window.location.search)
  const noteId = query.get('noteId') || getCurrentWindow().label.replace(/^note-/, '')
  let note: Note | undefined
  let windowState: NoteWindowState | undefined
  let settings: AppSettings = { ...DEFAULT_SETTINGS }
  let fonts: string[] = [DEFAULT_SETTINGS.defaultFontFamily]
  let EditorView: EditorComponent | undefined
  let Toolbar: ToolbarComponent | undefined
  let editorView: InstanceType<EditorComponent> | undefined
  let editor: TiptapEditor | undefined
  let fileInput: HTMLInputElement
  let saveState: SaveState = 'saved'
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let geometryTimer: ReturnType<typeof setTimeout> | undefined
  let moveTimer: ReturnType<typeof setTimeout> | undefined
  let dockTimer: ReturnType<typeof setTimeout> | undefined
  let pendingContent: Record<string, unknown> | undefined
  let pendingPlainText = ''
  let toolbarVersion = 0
  let tagDraft = ''
  let menuOpen = false
  let opacityOpen = false
  let loading = true
  let toast = ''
  let toastTimer: ReturnType<typeof setTimeout> | undefined
  let collapsed = false
  let locking = false
  let cleanups: Array<() => void> = []

  $: mode = windowState?.mode ?? 'card'
  $: themeClass = settings.theme === 'system' ? '' : `theme-${settings.theme}`

  function showToast(message: string, duration = 2400) {
    toast = message
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => (toast = ''), duration)
  }

  async function emitDesktop(event: string, payload?: unknown) {
    if (backend.isDesktop) await emit(event, payload)
  }

  async function ensureEditorResources(targetMode: NoteWindowMode) {
    if (targetMode === 'capsule') return
    if (!EditorView) EditorView = (await import('./components/Editor.svelte')).default
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
      title: note.title.trim() || documentTitle(pendingPlainText),
      content: pendingContent,
      plainText: pendingPlainText,
      pinned: note.pinned,
      archived: note.archived,
      tags: tagDraft.split(/[,，、]/).map((tag) => tag.trim().replace(/^#/, '')).filter(Boolean).slice(0, 12),
    }
  }

  async function flushSave() {
    clearTimeout(saveTimer)
    const input = noteInput()
    if (!input) return
    saveState = 'saving'
    try {
      note = await backend.saveNote(input)
      pendingContent = undefined
      saveState = 'saved'
      await emitDesktop('qingjian://notes-changed')
    } catch (error) {
      saveState = 'error'
      showToast(error instanceof Error ? error.message : '保存失败')
      throw error
    }
  }

  function scheduleSave(content: Record<string, unknown>, plainText: string) {
    pendingContent = content
    pendingPlainText = plainText
    if (note && (!note.title || note.title === '新便签')) note = { ...note, title: documentTitle(plainText) }
    saveState = 'dirty'
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => void flushSave(), 500)
  }

  function updateMeta() {
    if (!note || !editor) return
    pendingContent = editor.getJSON() as Record<string, unknown>
    pendingPlainText = editor.getText()
    saveState = 'dirty'
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => void flushSave(), 500)
  }

  function startWindowDrag(event: MouseEvent) {
    if (!backend.isDesktop || event.button !== 0 || windowState?.locked) return
    const target = event.target
    if (target instanceof Element && target.closest('button,input,select,textarea,a,[role="button"],[role="menu"]')) return
    void getCurrentWindow().startDragging()
  }

  async function switchMode(next: NoteWindowMode) {
    if (!windowState || next === mode) return
    await flushSave()
    editor = undefined
    await ensureEditorResources(next)
    windowState = await backend.setNoteWindowMode(noteId, next)
    menuOpen = false
    opacityOpen = false
    await tick()
    if (next !== 'capsule') editorView?.focus()
  }

  async function hideWindow() {
    await flushSave()
    await backend.saveNoteWindowGeometry(noteId)
    await backend.hideNoteWindow(noteId)
  }

  async function toggleAlwaysOnTop() {
    if (!windowState) return
    windowState = await backend.setNoteWindowAlwaysOnTop(noteId, !windowState.alwaysOnTop)
  }

  async function setOpacity(value: number) {
    if (!windowState) return
    windowState = await backend.setNoteWindowOpacity(noteId, value)
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
    note = { ...note, pinned: !note.pinned }
    updateMeta()
    await flushSave()
  }

  async function archiveCurrent() {
    if (!note) return
    await flushSave()
    await backend.setNoteArchived(note.id, true)
    await emitDesktop('qingjian://notes-changed')
    await backend.hideNoteWindow(note.id)
  }

  async function deleteCurrent() {
    if (!note) return
    const approved = backend.isDesktop
      ? await confirm(`将“${note.title}”移到回收站？之后仍可恢复。`, {
          title: '移到回收站', kind: 'warning', okLabel: '移到回收站', cancelLabel: '取消',
        })
      : window.confirm(`将“${note.title}”移到回收站？`)
    if (!approved) return
    await flushSave()
    await backend.deleteNote(note.id)
    await emitDesktop('qingjian://notes-changed')
    await backend.hideNoteWindow(note.id)
  }

  async function clipboardContent() {
    if (!editor) return { text: '', html: '', markdown: '' }
    const { DOMSerializer } = await import('@tiptap/pm/model')
    const { selection, doc, schema } = editor.state
    const fragment = selection.empty ? doc.content : selection.content().content
    const wrapper = document.createElement('div')
    wrapper.appendChild(DOMSerializer.fromSchema(schema).serializeFragment(fragment))
    return {
      text: selection.empty ? editor.getText() : doc.textBetween(selection.from, selection.to, '\n'),
      html: wrapper.innerHTML,
      markdown: serializeMarkdown(selection.empty ? editor.getJSON() : {
        type: 'doc', content: fragment.toJSON() as Record<string, unknown>[],
      }),
    }
  }

  async function copyContent(kind: 'rich' | 'text' | 'markdown' | 'html') {
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
      menuOpen = false
      showToast(kind === 'rich' ? '已复制并保留格式' : kind === 'markdown' ? '已复制 Markdown' : kind === 'html' ? '已复制 HTML' : '已复制纯文本')
    } catch { showToast('复制失败，请检查剪贴板权限') }
  }

  async function exportCurrent(kind: 'markdown' | 'html' | 'pdf') {
    if (!note || !editor) return
    menuOpen = false
    const extension = kind === 'markdown' ? 'md' : kind
    const destination = backend.isDesktop ? await save({
      title: kind === 'markdown' ? '导出 Markdown' : kind === 'html' ? '导出 HTML' : '导出 PDF',
      defaultPath: `${note.title}.${extension}`,
      filters: [{ name: `${extension.toUpperCase()} 文档`, extensions: [extension] }],
    }) : null
    if (!destination) return
    if (kind === 'markdown') await backend.exportMarkdown(destination, serializeMarkdown(editor.getJSON(), { title: note.title }))
    else if (kind === 'html') await backend.exportHtml(note.id, destination, editor.getHTML())
    else await backend.exportPdf(destination)
    showToast('导出完成')
  }

  async function collapseDocked() {
    if (!windowState?.dockEdge || windowState.locked || collapsed) return
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
    dockTimer = setTimeout(() => void collapseDocked(), 350)
  }

  function handleKeyboard(event: KeyboardEvent) {
    if (event.ctrlKey && !event.altKey && event.key.toLowerCase() === 'w') {
      event.preventDefault(); void hideWindow(); return
    }
    if (event.key === 'Escape') {
      if (menuOpen || opacityOpen) { menuOpen = false; opacityOpen = false; return }
      if (mode === 'editor') { event.preventDefault(); void switchMode('card') }
    }
  }

  onMount(() => {
    let disposed = false
    void (async () => {
      try {
        ;[note, settings, windowState] = await Promise.all([
          backend.getNote(noteId), backend.getSettings(), backend.noteWindowReady(noteId),
        ])
        pendingPlainText = note.preview
        tagDraft = note.tags.map((tag) => tag.name).join('、')
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
            await listen<NoteWindowState>('qingjian://window-state-changed', (event) => {
              void (async () => {
                await flushSave()
                await ensureEditorResources(event.payload.mode)
                windowState = event.payload
              })()
            }),
            await listen('qingjian://flush-and-quit', () => {
              void flushSave().then(() => emitDesktop('qingjian://note-save-ready', noteId)).catch(() => undefined)
            }),
            await listen('qingjian://flush-for-update', () => {
              void flushSave().then(() => emitDesktop('qingjian://note-update-save-ready', noteId)).catch(() => undefined)
            }),
          )
        }
      } catch (error) {
        showToast(error instanceof Error ? error.message : '便签打开失败')
      } finally {
        loading = false
      }
    })()
    return () => { disposed = true }
  })

  onDestroy(() => {
    clearTimeout(saveTimer); clearTimeout(geometryTimer); clearTimeout(moveTimer); clearTimeout(dockTimer); clearTimeout(toastTimer)
    cleanups.forEach((cleanup) => cleanup())
  })
</script>

<svelte:window on:keydown={handleKeyboard} on:blur={scheduleDockCollapse} />

<main
  class={`note-window-root mode-${mode} ${themeClass}`}
  class:collapsed
  on:mouseenter={() => void expandDocked()}
  on:mouseleave={scheduleDockCollapse}
>
  {#if collapsed}
    <div class="dock-strip" aria-label="展开停靠便签"></div>
  {:else if loading}
    <div class="note-loading">轻</div>
  {:else if note && windowState}
    {#if mode === 'capsule'}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <section class="capsule-shell" on:dblclick={() => void switchMode('card')}>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div class="capsule-drag" on:mousedown={startWindowDrag}>
          <i class="status-dot" class:dirty={saveState !== 'saved'}></i>
          <div class="capsule-copy">
            <strong>{note.title || '新便签'}</strong>
            {#if note.preview}<span>{note.preview}</span>{/if}
          </div>
        </div>
        <button class="capsule-expand" aria-label="展开为卡片" on:click={() => void switchMode('card')}>展开</button>
      </section>
    {:else}
      <section class="desktop-note-shell">
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <header class="note-window-titlebar" on:mousedown={startWindowDrag}>
          <div class="mode-switcher" aria-label="便签形态">
            <button on:click={() => void switchMode('capsule')}>胶囊</button>
            <button class:active={mode === 'card'} on:click={() => void switchMode('card')}>卡片</button>
            <button class:active={mode === 'editor'} on:click={() => void switchMode('editor')}>编辑</button>
          </div>
          <div class="window-note-actions">
            <span class:error={saveState === 'error'} class="compact-save-state">
              {saveState === 'saving' ? '保存中' : saveState === 'dirty' ? '未保存' : saveState === 'error' ? '失败' : ''}
            </span>
            <button class:active={windowState.alwaysOnTop} aria-label="窗口置顶" on:click={toggleAlwaysOnTop}>
              {#if windowState.alwaysOnTop}<PinOff />{:else}<Pin />{/if}
            </button>
            <div class="note-popover-anchor">
              <button aria-label="透明度" on:click={() => { opacityOpen = !opacityOpen; menuOpen = false }}>
                <span class="opacity-icon">{windowState.opacity}</span>
              </button>
              {#if opacityOpen}
                <div class="opacity-popover">
                  <strong>透明度 {windowState.opacity}%</strong>
                  <input type="range" min="60" max="100" step="5" value={windowState.opacity}
                    on:input={(event) => void setOpacity(Number(event.currentTarget.value))} />
                  <div>{#each [70, 85, 100] as value}<button on:click={() => void setOpacity(value)}>{value}%</button>{/each}</div>
                </div>
              {/if}
            </div>
            <button aria-label={windowState.locked ? '解锁' : '锁定并穿透'} disabled={locking} on:click={toggleLock}>
              {#if windowState.locked}<Unlock />{:else}<LockKeyhole />{/if}
            </button>
            <div class="note-popover-anchor">
              <button aria-label="更多" on:click={() => { menuOpen = !menuOpen; opacityOpen = false }}><MoreHorizontal /></button>
              {#if menuOpen}
                <div class="note-more-menu" role="menu">
                  <button on:click={toggleContentPin}>{#if note.pinned}<PinOff />取消内容置顶{:else}<Pin />内容置顶{/if}</button>
                  {#if mode === 'editor'}
                    <button on:click={() => fileInput.click()}><ImagePlus />插入图片</button>
                    <small>复制内容</small>
                    <button on:click={() => void copyContent('rich')}><Copy />保留源格式</button>
                    <button on:click={() => void copyContent('text')}><Text />纯文本</button>
                    <button on:click={() => void copyContent('markdown')}><Code2 />Markdown</button>
                    <button on:click={() => void copyContent('html')}><Code2 />HTML 源码</button>
                    <small>导出</small>
                    <button on:click={() => void exportCurrent('markdown')}><FileDown />Markdown</button>
                    <button on:click={() => void exportCurrent('html')}><FileDown />HTML</button>
                    <button on:click={() => void exportCurrent('pdf')}><Download />PDF</button>
                  {/if}
                  <button on:click={() => void archiveCurrent()}><Archive />归档便签</button>
                  <button on:click={() => { menuOpen = false; void backend.showHubWindow().then(() => emitDesktop('qingjian://settings')) }}><Settings />打开设置</button>
                  <button class="danger" on:click={() => void deleteCurrent()}><Trash2 />移到回收站</button>
                </div>
              {/if}
            </div>
            <button aria-label="隐藏便签" on:click={() => void hideWindow()}><EyeOff /></button>
            <button class="note-close" aria-label="隐藏便签" on:click={() => void hideWindow()}><X /></button>
          </div>
        </header>

        <div class="note-window-heading">
          <input
            class="note-window-title" aria-label="便签标题" value={note.title}
            on:input={(event) => { note = { ...note!, title: event.currentTarget.value }; updateMeta() }}
          />
          {#if mode === 'editor'}
            <input class="note-window-tags" aria-label="便签标签" placeholder="# 添加标签" bind:value={tagDraft} on:change={updateMeta} />
          {/if}
        </div>

        {#if mode === 'editor' && Toolbar}
          <svelte:component this={Toolbar} {editor} {fonts} version={toolbarVersion} onPickImage={() => fileInput.click()} />
        {/if}
        <div class="note-window-editor" class:card-editor={mode === 'card'}>
          {#if EditorView}
            <svelte:component
              this={EditorView} bind:this={editorView} noteId={note.id} content={note.content}
              defaultFontFamily={settings.defaultFontFamily} defaultFontSize={settings.defaultFontSize}
              onChange={scheduleSave}
              onReady={(instance: TiptapEditor) => { editor = instance; if (mode === 'card') instance.commands.focus() }}
              onSelectionChange={() => (toolbarVersion += 1)} onError={showToast}
            />
          {/if}
        </div>
      </section>
    {/if}
  {/if}
</main>

<input bind:this={fileInput} class="hidden-file-input" type="file"
  accept="image/png,image/jpeg,image/webp,image/gif,image/bmp" multiple
  on:change={(event) => {
    if (event.currentTarget.files) void editorView?.insertFiles(event.currentTarget.files)
    event.currentTarget.value = ''
  }} />

{#if toast}<div class="toast note-window-toast" role="status">{toast}</div>{/if}

<style>
  :global(html), :global(body), :global(#app) { width: 100%; height: 100%; overflow: hidden; }
  :global(#app) { padding: 0; }
  .note-window-root { display:block; width:100%; height:100%; padding:7px; box-sizing:border-box; color:var(--ink, #27251f); background:transparent; }
  .note-window-root.collapsed { padding: 0; }
  .dock-strip { width: 100%; height: 100%; background: linear-gradient(135deg, #f4d36e, #e9b83f); box-shadow: inset 0 0 0 1px rgba(91,68,16,.18); }
  .note-loading { display:grid; place-items:center; height:100%; border-radius:16px; background:#fff8dc; color:#9b7218; font-weight:800; }
  .capsule-shell, .desktop-note-shell { width:100%; height:100%; box-sizing:border-box; background:rgba(255,250,226,.97); border:1px solid rgba(105,82,25,.16); box-shadow:0 8px 26px rgba(48,39,18,.18); overflow:hidden; }
  .capsule-shell { border-radius:999px; display:flex; align-items:center; padding:6px 8px 6px 14px; gap:8px; }
  .capsule-drag { min-width:0; flex:1; display:flex; align-items:center; gap:10px; cursor:grab; }
  .capsule-drag:active { cursor:grabbing; }
  .status-dot { width:8px; height:8px; flex:none; border-radius:50%; background:#5dbe79; box-shadow:0 0 0 3px rgba(93,190,121,.14); }
  .status-dot.dirty { background:#e1a72f; }
  .capsule-copy { min-width:0; display:flex; align-items:baseline; gap:9px; white-space:nowrap; overflow:hidden; }
  .capsule-copy strong { max-width:48%; overflow:hidden; text-overflow:ellipsis; font-size:13px; }
  .capsule-copy span { flex:1; overflow:hidden; text-overflow:ellipsis; color:#7a725f; font-size:12px; }
  .capsule-expand { flex:none; border:0; border-radius:999px; padding:7px 11px; background:rgba(121,91,22,.09); color:#6b521d; font-size:11px; }
  .desktop-note-shell { border-radius:16px; display:flex; flex-direction:column; }
  .note-window-titlebar { min-height:39px; display:flex; align-items:center; justify-content:space-between; padding:0 6px 0 9px; border-bottom:1px solid rgba(95,76,28,.09); cursor:grab; }
  .note-window-titlebar:active { cursor:grabbing; }
  .mode-switcher { display:flex; padding:2px; border-radius:9px; background:rgba(102,82,33,.07); }
  .mode-switcher button { border:0; background:transparent; padding:5px 8px; border-radius:7px; color:#766b50; font-size:11px; }
  .mode-switcher button.active { background:#fffdf2; color:#4e421f; box-shadow:0 1px 4px rgba(57,44,12,.12); }
  .window-note-actions { display:flex; align-items:center; gap:1px; }
  .window-note-actions > button, .note-popover-anchor > button { width:28px; height:28px; border:0; border-radius:8px; display:grid; place-items:center; background:transparent; color:#766b50; }
  .window-note-actions button:hover, .window-note-actions button.active { background:rgba(105,82,25,.09); color:#463811; }
  .window-note-actions :global(svg) { width:14px; height:14px; }
  .opacity-icon { font-size:9px; font-weight:750; }
  .compact-save-state { min-width:34px; text-align:right; margin-right:3px; font-size:9px; color:#99855c; }
  .compact-save-state.error { color:#c95545; }
  .note-popover-anchor { position:relative; }
  .opacity-popover, .note-more-menu { position:absolute; z-index:20; right:0; top:32px; border:1px solid rgba(91,70,19,.14); border-radius:12px; background:#fffdf5; box-shadow:0 12px 32px rgba(53,40,10,.2); }
  .opacity-popover { width:190px; padding:12px; }
  .opacity-popover strong { font-size:12px; }
  .opacity-popover input { width:100%; margin:10px 0; }
  .opacity-popover div { display:flex; gap:5px; }
  .opacity-popover div button { flex:1; border:1px solid #e6d8af; border-radius:7px; background:#fff; padding:5px; font-size:10px; }
  .note-more-menu { width:180px; max-height:min(480px, calc(100vh - 48px)); overflow:auto; padding:6px; }
  .note-more-menu button { width:100%; display:flex; align-items:center; gap:8px; border:0; border-radius:8px; background:transparent; padding:8px; color:#554a31; font-size:12px; text-align:left; }
  .note-more-menu button:hover { background:#f7efd6; }
  .note-more-menu button.danger { color:#b94b42; }
  .note-more-menu small { display:block; padding:7px 8px 3px; color:#a08d63; font-size:9px; text-transform:uppercase; }
  .note-window-heading { display:flex; align-items:center; gap:8px; padding:10px 14px 6px; }
  .note-window-title { min-width:0; flex:1; border:0; outline:0; background:transparent; color:inherit; font-size:18px; font-weight:740; }
  .note-window-tags { width:120px; border:0; border-bottom:1px solid rgba(90,70,22,.12); outline:0; background:transparent; color:#8a7953; font-size:11px; }
  .note-window-editor { min-height:0; flex:1; overflow:auto; }
  .note-window-editor :global(.editor-host) { min-height:100%; }
  .note-window-editor :global(.tiptap) { min-height:100%; box-sizing:border-box; padding:10px 16px 24px; outline:0; }
  .card-editor :global(.tiptap) { padding-top:4px; font-size:14px; }
  .mode-card .note-window-titlebar { min-height:36px; }
  .mode-card .note-window-heading { padding-top:7px; }
  .mode-card .note-window-title { font-size:16px; }
  @media (max-width: 330px), (max-height: 210px) {
    .note-window-root { padding:4px; }
    .note-window-titlebar { padding-left:5px; }
    .mode-switcher button { padding:4px 5px; font-size:9px; }
    .window-note-actions > button, .note-popover-anchor > button { width:24px; height:24px; }
    .compact-save-state, .note-close { display:none !important; }
    .note-window-heading { padding:5px 10px 2px; }
    .note-window-title { font-size:14px; }
    .card-editor :global(.tiptap) { padding:4px 10px 14px; }
  }
</style>
