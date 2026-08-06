<script lang="ts">
  import { onDestroy, onMount, tick } from 'svelte'
  import type { Editor as TiptapEditor } from '@tiptap/core'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { emit, listen } from '@tauri-apps/api/event'
  import { confirm, save } from '@tauri-apps/plugin-dialog'
  import {
    Archive, Code2, Copy, Download, EyeOff, FileDown, ImagePlus, LockKeyhole,
    Blend, MoreHorizontal, Pin, PinOff, Settings, Tags, Text, Trash2, Unlock, X,
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
  let editorScroll: HTMLDivElement
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
  let activePopover: 'opacity' | 'tags' | 'more' | 'format' | null = null
  let loading = true
  let toast = ''
  let toastTimer: ReturnType<typeof setTimeout> | undefined
  let collapsed = false
  let locking = false
  let cleanups: Array<() => void> = []

  $: mode = windowState?.mode ?? 'card'
  $: themeClass = settings.theme === 'system' ? '' : `theme-${settings.theme}`
  $: backgroundPercent = `${windowState?.backgroundOpacity ?? 100}%`
  $: textPercent = `${windowState?.textOpacity ?? 100}%`
  $: capsulePreview = note ? note.preview.split(/\r?\n/)
    .map((line) => line.trim()).filter(Boolean).filter((line, index) => index > 0 || line !== note?.title)
    .join(' · ') : ''

  function closeTransient() {
    activePopover = null
  }

  function togglePopover(name: Exclude<typeof activePopover, null>) {
    activePopover = activePopover === name ? null : name
  }

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
      title: documentTitle(pendingPlainText),
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
    if (note) note = { ...note, title: documentTitle(plainText), preview: plainText }
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

  async function saveTags() {
    updateMeta()
    closeTransient()
    await flushSave()
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
    if (next === 'capsule') editor = undefined
    await ensureEditorResources(next)
    windowState = await backend.setNoteWindowMode(noteId, next)
    closeTransient()
    await tick()
    if (next !== 'capsule') {
      if (next === 'card' && editor) {
        editor.chain().setTextSelection(1).focus().run()
        if (editorScroll) editorScroll.scrollTop = 0
      } else {
        editorView?.focus()
      }
    }
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
    closeTransient()
    if (!note) return
    note = { ...note, pinned: !note.pinned }
    updateMeta()
    await flushSave()
  }

  async function archiveCurrent() {
    closeTransient()
    if (!note) return
    await flushSave()
    await backend.setNoteArchived(note.id, true)
    await emitDesktop('qingjian://notes-changed')
    await backend.hideNoteWindow(note.id)
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
    closeTransient()
    const extension = kind === 'markdown' ? 'md' : kind
    const destination = backend.isDesktop ? await save({
      title: kind === 'markdown' ? '导出 Markdown' : kind === 'html' ? '导出 HTML' : '导出 PDF',
      defaultPath: `${note.title}.${extension}`,
      filters: [{ name: `${extension.toUpperCase()} 文档`, extensions: [extension] }],
    }) : null
    if (!destination) return
    if (kind === 'markdown') await backend.exportMarkdown(destination, serializeMarkdown(editor.getJSON()))
    else if (kind === 'html') await backend.exportHtml(note.id, destination, editor.getHTML())
    else await backend.exportPdf(note.id, destination)
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
    if (event.ctrlKey && !event.altKey && event.key.toLowerCase() === 'w') {
      event.preventDefault(); void hideWindow(); return
    }
    if (event.key === 'Escape') {
      if (activePopover) { event.preventDefault(); closeTransient(); return }
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
                if (event.payload.mode !== mode) {
                  await flushSave()
                  await ensureEditorResources(event.payload.mode)
                }
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

<svelte:window on:keydown={handleKeyboard} on:blur={handleWindowBlur} on:pointerdown={handleWindowPointerDown} />

<main
  class={`note-window-root mode-${mode} ${themeClass}`}
  style={`--note-bg-percent:${backgroundPercent};--note-text-percent:${textPercent}`}
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
            {#if capsulePreview}<span>{capsulePreview}</span>{/if}
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
            <div class="note-popover-anchor" data-transient-root>
              <button class:active={activePopover === 'opacity'} aria-label="调整背景与文字透明度" on:click={() => togglePopover('opacity')}>
                <Blend />
              </button>
              {#if activePopover === 'opacity'}
                <div class="opacity-popover">
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
              {/if}
            </div>
            <div class="note-popover-anchor" data-transient-root>
              <button class:active={activePopover === 'tags'} aria-label="编辑标签" on:click={() => togglePopover('tags')}>
                <Tags />{#if note.tags.length}<span class="tag-count">{note.tags.length}</span>{/if}
              </button>
              {#if activePopover === 'tags'}
                <div class="tag-popover">
                  <strong>便签标签</strong>
                  <input aria-label="便签标签" placeholder="输入标签，用逗号分隔" bind:value={tagDraft}
                    on:keydown={(event) => event.key === 'Enter' && void saveTags()} />
                  <small>最多 12 个，按 Enter 保存</small>
                  <button class="tag-save" on:click={() => void saveTags()}>保存标签</button>
                </div>
              {/if}
            </div>
            <button aria-label={windowState.locked ? '解锁' : '锁定并穿透'} disabled={locking} on:click={toggleLock}>
              {#if windowState.locked}<Unlock />{:else}<LockKeyhole />{/if}
            </button>
            <div class="note-popover-anchor" data-transient-root>
              <button class:active={activePopover === 'more'} aria-label="更多" on:click={() => togglePopover('more')}><MoreHorizontal /></button>
              {#if activePopover === 'more'}
                <div class="note-more-menu" role="menu">
                  <button on:click={toggleContentPin}>{#if note.pinned}<PinOff />取消内容置顶{:else}<Pin />内容置顶{/if}</button>
                  {#if mode === 'editor'}
                    <button on:click={() => { closeTransient(); fileInput.click() }}><ImagePlus />插入图片</button>
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
                  <button on:click={() => { closeTransient(); void backend.showHubWindow().then(() => emitDesktop('qingjian://settings')) }}><Settings />打开设置</button>
                  <button class="danger" on:click={() => void deleteCurrent()}><Trash2 />移到回收站</button>
                </div>
              {/if}
            </div>
            <button aria-label="隐藏便签" on:click={() => void hideWindow()}><EyeOff /></button>
            <button class="note-close" aria-label="隐藏便签" on:click={() => void hideWindow()}><X /></button>
          </div>
        </header>

        {#if mode === 'editor' && Toolbar}
          <svelte:component this={Toolbar} {editor} {fonts} version={toolbarVersion}
            menuOpen={activePopover === 'format'}
            onMenuOpenChange={(open: boolean) => (activePopover = open ? 'format' : null)}
            onPickImage={() => fileInput.click()} />
        {/if}
        <div bind:this={editorScroll} class="note-window-editor" class:card-editor={mode === 'card'}>
          {#if EditorView}
            <svelte:component
              this={EditorView} bind:this={editorView} noteId={note.id} content={note.content}
              defaultFontFamily={settings.defaultFontFamily} defaultFontSize={settings.defaultFontSize}
              onChange={scheduleSave}
              onReady={(instance: TiptapEditor) => {
                editor = instance
                if (mode === 'card') {
                  instance.chain().setTextSelection(1).focus().run()
                  void tick().then(() => { if (editorScroll) editorScroll.scrollTop = 0 })
                }
              }}
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
  .note-window-root { display:block; width:100%; height:100%; padding:7px; box-sizing:border-box; color:var(--text, #27251f); background:transparent; }
  .note-window-root.collapsed { padding: 0; }
  .dock-strip { width: 100%; height: 100%; background: linear-gradient(135deg, #f4d36e, #e9b83f); box-shadow: inset 0 0 0 1px rgba(91,68,16,.18); }
  .note-loading { display:grid; place-items:center; height:100%; border-radius:16px; background:#fff8dc; color:#9b7218; font-weight:800; }
  .capsule-shell, .desktop-note-shell { width:100%; height:100%; box-sizing:border-box; background:color-mix(in srgb, var(--paper, #fff8dc) var(--note-bg-percent), transparent); border:1px solid color-mix(in srgb, var(--line), var(--text) 12%); box-shadow:0 8px 26px rgba(48,39,18,.18); overflow:hidden; }
  .capsule-shell { border-radius:999px; display:flex; align-items:center; padding:6px 8px 6px 14px; gap:8px; }
  .capsule-drag { min-width:0; flex:1; display:flex; align-items:center; gap:10px; cursor:grab; }
  .capsule-drag:active { cursor:grabbing; }
  .status-dot { width:8px; height:8px; flex:none; border-radius:50%; background:#5dbe79; box-shadow:0 0 0 3px rgba(93,190,121,.14); }
  .status-dot.dirty { background:#e1a72f; }
  .capsule-copy { min-width:0; display:flex; align-items:baseline; gap:9px; white-space:nowrap; overflow:hidden; color:color-mix(in srgb, var(--text, #27251f) var(--note-text-percent), transparent); }
  .capsule-copy strong { max-width:48%; overflow:hidden; text-overflow:ellipsis; font-size:13px; }
  .capsule-copy span { flex:1; overflow:hidden; text-overflow:ellipsis; color:var(--muted); font-size:12px; }
  .capsule-expand { flex:none; border:0; border-radius:999px; padding:7px 11px; background:var(--accent-soft); color:var(--text); font-size:11px; }
  .desktop-note-shell { border-radius:16px; display:flex; flex-direction:column; }
  .note-window-titlebar { min-height:39px; display:flex; align-items:center; justify-content:space-between; padding:0 6px 0 9px; border-bottom:1px solid var(--line); background:color-mix(in srgb, var(--paper, #fff8dc) 78%, transparent); cursor:grab; }
  .note-window-titlebar:active { cursor:grabbing; }
  .mode-switcher { display:flex; padding:2px; border-radius:9px; background:color-mix(in srgb, var(--sidebar), transparent 20%); }
  .mode-switcher button { border:0; background:transparent; padding:5px 8px; border-radius:7px; color:var(--muted); font-size:11px; }
  .mode-switcher button.active { background:var(--paper); color:var(--text); box-shadow:0 1px 4px rgba(57,44,12,.12); }
  .window-note-actions { display:flex; align-items:center; gap:1px; }
  .window-note-actions > button, .note-popover-anchor > button { width:28px; height:28px; border:0; border-radius:8px; display:grid; place-items:center; background:transparent; color:var(--muted); }
  .window-note-actions button:hover, .window-note-actions button.active { background:var(--accent-soft); color:var(--text); }
  .window-note-actions :global(svg) { width:14px; height:14px; }
  .tag-count { position:absolute; right:2px; top:2px; min-width:11px; height:11px; padding:0 2px; box-sizing:border-box; border-radius:999px; background:#dcae35; color:#fff; font-size:7px; line-height:11px; }
  .compact-save-state { min-width:34px; text-align:right; margin-right:3px; font-size:9px; color:var(--muted); }
  .compact-save-state.error { color:#c95545; }
  .note-popover-anchor { position:relative; }
  .opacity-popover, .tag-popover, .note-more-menu { position:fixed; z-index:30; right:8px; top:44px; max-width:calc(100vw - 16px); border:1px solid rgba(91,70,19,.14); border-radius:12px; background:var(--paper, #fffdf5); color:var(--text, #27251f); box-shadow:0 12px 32px rgba(53,40,10,.2); }
  .opacity-popover { width:220px; padding:12px; display:grid; gap:13px; }
  .opacity-popover label { display:grid; gap:7px; }
  .opacity-popover label > span { display:flex; justify-content:space-between; align-items:center; font-size:11px; }
  .opacity-popover label > span b { color:var(--muted, #887a59); font-size:10px; }
  .opacity-popover input { width:100%; margin:0; accent-color:#d2a62e; }
  .opacity-popover i { display:flex; gap:5px; font-style:normal; }
  .opacity-popover i button { flex:1; min-height:26px; border:1px solid var(--line); border-radius:7px; background:var(--sidebar); color:var(--text); padding:4px; font-size:10px; }
  .tag-popover { width:220px; padding:12px; display:grid; gap:8px; }
  .tag-popover strong { font-size:12px; }
  .tag-popover input { width:100%; box-sizing:border-box; border:1px solid var(--line); border-radius:8px; background:var(--sidebar); color:inherit; padding:8px 9px; outline:0; }
  .tag-popover input:focus { border-color:#c69a27; box-shadow:0 0 0 3px rgba(198,154,39,.12); }
  .tag-popover small { color:var(--muted, #887a59); font-size:9px; }
  .tag-popover .tag-save { justify-self:end; min-height:28px; border:0; border-radius:7px; background:#e6bd4e; color:#43330d; padding:5px 10px; }
  .note-more-menu { width:180px; max-height:min(480px, calc(100vh - 48px)); overflow:auto; padding:6px; }
  .note-more-menu button { width:100%; display:flex; align-items:center; gap:8px; border:0; border-radius:8px; background:transparent; padding:8px; color:var(--text); font-size:12px; text-align:left; }
  .note-more-menu button:hover { background:var(--sidebar); }
  .note-more-menu button.danger { color:#b94b42; }
  .note-more-menu small { display:block; padding:7px 8px 3px; color:var(--muted); font-size:9px; text-transform:uppercase; }
  .note-window-editor { min-height:0; flex:1; overflow:auto; }
  .note-window-root :global(.format-toolbar) { background:color-mix(in srgb, var(--paper) 84%, transparent); }
  .note-window-editor :global(.editor-host) { min-height:100%; }
  .note-window-editor :global(.tiptap) { min-height:100%; box-sizing:border-box; padding:14px 16px 24px; outline:0; color:color-mix(in srgb, var(--text, #27251f) var(--note-text-percent), transparent); caret-color:var(--text, #27251f); }
  .note-window-editor :global(.tiptap img) { opacity:1; }
  .card-editor :global(.tiptap) { padding-top:12px; font-size:14px; }
  .mode-card .note-window-titlebar { min-height:36px; }
  @media (max-width: 330px), (max-height: 210px) {
    .note-window-root { padding:4px; }
    .note-window-titlebar { padding-left:5px; }
    .mode-switcher button { padding:4px 5px; font-size:9px; }
    .window-note-actions > button, .note-popover-anchor > button { width:24px; height:24px; }
    .compact-save-state, .note-close { display:none !important; }
    .card-editor :global(.tiptap) { padding:9px 10px 14px; }
  }
  @media (max-height: 210px) {
    .opacity-popover, .tag-popover, .note-more-menu { top:8px; max-height:calc(100vh - 16px); overflow:auto; }
  }
</style>
