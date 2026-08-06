<script lang="ts">
  import { onMount, tick } from 'svelte'
  import type { Editor as TiptapEditor } from '@tiptap/core'
  import { DOMSerializer } from '@tiptap/pm/model'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { emit, listen } from '@tauri-apps/api/event'
  import { confirm, save } from '@tauri-apps/plugin-dialog'
  import {
    Archive, ArchiveRestore, ChevronDown, Code2, Copy, Download, FileDown, ImagePlus,
    Minus, MoreHorizontal, Pin, PinOff, Settings, Text, Trash2, X,
  } from 'lucide-svelte'
  import Editor from './components/Editor.svelte'
  import Toolbar from './components/Toolbar.svelte'
  import { backend } from './lib/backend'
  import { documentTitle } from './lib/editor'
  import { serializeMarkdown } from './lib/markdown'
  import { tooltip } from './lib/tooltip'
  import { DEFAULT_SETTINGS } from './lib/types'
  import type {
    AppSettings, Note, NoteSummary, SaveNoteInput, SaveState, WorkspaceState,
  } from './lib/types'

  let workspace: WorkspaceState = { openNoteIds: [], activeNoteId: null }
  let tabs: NoteSummary[] = []
  let activeNote: Note | undefined
  let settings: AppSettings = { ...DEFAULT_SETTINGS }
  let fonts: string[] = [DEFAULT_SETTINGS.defaultFontFamily]
  let editor: TiptapEditor | undefined
  let editorView: Editor
  let fileInput: HTMLInputElement
  let saveState: SaveState = 'saved'
  let saveTimer: ReturnType<typeof setTimeout> | undefined
  let pendingContent: Record<string, unknown> | undefined
  let pendingPlainText = ''
  let toolbarVersion = 0
  let menuOpen = false
  let tabsExpanded = false
  let pagesMenuOpen = false
  let collapseTimer: ReturnType<typeof setTimeout> | undefined
  let toast = ''
  let toastTimer: ReturnType<typeof setTimeout> | undefined
  let tagDraft = ''
  let loading = true
  let closing = false

  $: themeClass = settings.theme === 'system' ? '' : `theme-${settings.theme}`

  function showToast(message: string) {
    toast = message
    clearTimeout(toastTimer)
    toastTimer = setTimeout(() => (toast = ''), 2400)
  }

  async function emitDesktop(event: string, payload?: unknown) {
    if (backend.isDesktop) await emit(event, payload)
  }

  function startWindowDrag(event: MouseEvent) {
    if (!backend.isDesktop || event.button !== 0) return
    const target = event.target
    if (target instanceof Element && target.closest('button,input,select,textarea,a,[role="button"],[role="menu"]')) return
    void getCurrentWindow().startDragging()
  }

  async function refreshTabs() {
    const all = await backend.listNotes('', true)
    const map = new Map(all.map((note) => [note.id, note]))
    tabs = workspace.openNoteIds.map((id) => map.get(id)).filter(Boolean) as NoteSummary[]
    const validIds = tabs.map((tab) => tab.id)
    if (validIds.length !== workspace.openNoteIds.length) {
      workspace = await backend.saveWorkspaceState({
        openNoteIds: validIds,
        activeNoteId: validIds.includes(workspace.activeNoteId || '')
          ? workspace.activeNoteId
          : validIds[0] ?? null,
      })
    }
  }

  async function persistWorkspace() {
    workspace = await backend.saveWorkspaceState(workspace)
    await emitDesktop('qingjian://workspace-changed', workspace)
  }

  async function selectNote(id: string) {
    if (activeNote?.id === id) return
    await flushSave()
    activeNote = await backend.getNote(id)
    if (!workspace.openNoteIds.includes(id)) workspace.openNoteIds = [...workspace.openNoteIds, id]
    workspace.activeNoteId = id
    pendingContent = undefined
    pendingPlainText = activeNote.preview
    tagDraft = activeNote.tags.map((tag) => tag.name).join('、')
    saveState = 'saved'
    await persistWorkspace()
    await refreshTabs()
    await tick()
    editorView?.focus()
  }

  async function acceptOpenRequest(id: string) {
    workspace = await backend.getWorkspaceState()
    await refreshTabs()
    await selectNote(id)
    tabsExpanded = true
    scheduleCollapse()
  }

  function noteInput(): SaveNoteInput | undefined {
    if (!activeNote || !pendingContent) return
    return {
      id: activeNote.id,
      title: activeNote.title.trim() || documentTitle(pendingPlainText),
      content: pendingContent,
      plainText: pendingPlainText,
      pinned: activeNote.pinned,
      archived: activeNote.archived,
      tags: tagDraft.split(/[，,、#]/).map((tag) => tag.trim()).filter(Boolean).slice(0, 12),
    }
  }

  async function flushSave() {
    clearTimeout(saveTimer)
    const input = noteInput()
    if (!input) return
    saveState = 'saving'
    try {
      activeNote = await backend.saveNote(input)
      pendingContent = undefined
      saveState = 'saved'
      await refreshTabs()
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
    if (activeNote && (!activeNote.title || activeNote.title === '新便签')) {
      activeNote = { ...activeNote, title: documentTitle(plainText) }
      tabs = tabs.map((tab) => tab.id === activeNote?.id ? { ...tab, title: activeNote.title } : tab)
    }
    saveState = 'dirty'
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => void flushSave(), 500)
  }

  function updateMeta() {
    if (!activeNote || !editor) return
    pendingContent = editor.getJSON() as Record<string, unknown>
    pendingPlainText = editor.getText()
    saveState = 'dirty'
    clearTimeout(saveTimer)
    saveTimer = setTimeout(() => void flushSave(), 500)
  }

  async function closeTab(id: string) {
    if (activeNote?.id === id) await flushSave()
    const index = workspace.openNoteIds.indexOf(id)
    const remaining = workspace.openNoteIds.filter((noteId) => noteId !== id)
    workspace = {
      openNoteIds: remaining,
      activeNoteId: workspace.activeNoteId === id
        ? remaining[Math.min(index, remaining.length - 1)] ?? null
        : workspace.activeNoteId,
    }
    await persistWorkspace()
    await refreshTabs()
    if (!workspace.activeNoteId) {
      activeNote = undefined
      await destroyEditor()
      return
    }
    if (activeNote?.id === id) {
      activeNote = undefined
      await selectNote(workspace.activeNoteId)
    }
  }

  async function destroyEditor() {
    if (closing) return
    closing = true
    try {
      await flushSave()
      await persistWorkspace()
      await getCurrentWindow().hide()
    } finally {
      closing = false
    }
  }

  async function deleteCurrent() {
    if (!activeNote) return
    const approved = backend.isDesktop
      ? await confirm(`将“${activeNote.title}”移到回收站？之后仍可恢复。`, {
          title: '移到回收站', kind: 'warning', okLabel: '移到回收站', cancelLabel: '取消',
        })
      : window.confirm(`将“${activeNote.title}”移到回收站？`)
    if (!approved) return
    await flushSave()
    const id = activeNote.id
    await backend.deleteNote(id)
    await emitDesktop('qingjian://notes-changed')
    const index = workspace.openNoteIds.indexOf(id)
    const remaining = workspace.openNoteIds.filter((noteId) => noteId !== id)
    workspace = {
      openNoteIds: remaining,
      activeNoteId: remaining[Math.min(index, remaining.length - 1)] ?? null,
    }
    activeNote = undefined
    pendingContent = undefined
    await persistWorkspace()
    await refreshTabs()
    if (workspace.activeNoteId) await selectNote(workspace.activeNoteId)
    else await destroyEditor()
  }

  function selectedClipboardContent() {
    if (!editor) return { text: '', html: '', markdown: '' }
    const { selection, doc, schema } = editor.state
    const fragment = selection.empty ? doc.content : selection.content().content
    const wrapper = document.createElement('div')
    wrapper.appendChild(DOMSerializer.fromSchema(schema).serializeFragment(fragment))
    const markdown = serializeMarkdown({
      type: 'doc',
      content: fragment.toJSON() as Record<string, unknown>[],
    }, {
      assetUrl: (assetPath, source) => assetPath || source,
    })
    return {
      text: selection.empty ? editor.getText() : doc.textBetween(selection.from, selection.to, '\n'),
      html: wrapper.innerHTML,
      markdown,
    }
  }

  async function copyContent(mode: 'rich' | 'text' | 'html' | 'markdown') {
    if (!editor) return
    const content = selectedClipboardContent()
    try {
      if (mode === 'rich' && typeof ClipboardItem !== 'undefined') {
        await navigator.clipboard.write([new ClipboardItem({
          'text/plain': new Blob([content.text], { type: 'text/plain' }),
          'text/html': new Blob([content.html], { type: 'text/html' }),
        })])
      } else {
        await navigator.clipboard.writeText(
          mode === 'html' ? content.html : mode === 'markdown' ? content.markdown : content.text,
        )
      }
      menuOpen = false
      showToast(
        mode === 'rich' ? '已复制并保留源格式'
          : mode === 'html' ? '已复制 HTML 源码'
            : mode === 'markdown' ? '已复制 Markdown'
              : '已复制纯文本',
      )
    } catch {
      showToast('复制失败，请检查系统剪贴板权限')
    }
  }

  async function toggleArchive() {
    if (!activeNote) return
    activeNote = { ...activeNote, archived: !activeNote.archived }
    updateMeta()
    await flushSave()
  }

  async function togglePin() {
    if (!activeNote) return
    activeNote = { ...activeNote, pinned: !activeNote.pinned }
    updateMeta()
    await flushSave()
  }

  async function exportHtml() {
    if (!activeNote || !editor) return
    const destination = backend.isDesktop ? await save({
      title: '导出 HTML', defaultPath: `${activeNote.title}.html`,
      filters: [{ name: 'HTML 文档', extensions: ['html'] }],
    }) : null
    if (!destination) return
    await backend.exportHtml(activeNote.id, destination, editor.getHTML())
    showToast('HTML 已导出')
  }

  async function exportMarkdown() {
    if (!activeNote || !editor) return
    menuOpen = false
    const destination = backend.isDesktop ? await save({
      title: '导出 Markdown', defaultPath: `${activeNote.title}.md`,
      filters: [{ name: 'Markdown 文档', extensions: ['md'] }],
    }) : null
    if (!destination) return
    const markdown = serializeMarkdown(editor.getJSON(), { title: activeNote.title })
    await backend.exportMarkdown(destination, markdown)
    showToast('Markdown 已导出')
  }

  async function exportPdf() {
    if (!activeNote) return
    menuOpen = false
    const destination = backend.isDesktop ? await save({
      title: '导出 PDF', defaultPath: `${activeNote.title}.pdf`,
      filters: [{ name: 'PDF 文档', extensions: ['pdf'] }],
    }) : null
    if (backend.isDesktop && !destination) return
    document.body.classList.add('print-note')
    await tick()
    try {
      if (destination) await backend.exportPdf(destination)
      else window.print()
      showToast('PDF 已导出')
    } finally {
      document.body.classList.remove('print-note')
    }
  }

  function expandTabs() {
    clearTimeout(collapseTimer)
    tabsExpanded = true
  }

  function scheduleCollapse() {
    clearTimeout(collapseTimer)
    if (pagesMenuOpen) return
    collapseTimer = setTimeout(() => (tabsExpanded = false), 450)
  }

  async function cycleTab(direction: number) {
    if (tabs.length < 2 || !activeNote) return
    const current = tabs.findIndex((tab) => tab.id === activeNote?.id)
    const next = (current + direction + tabs.length) % tabs.length
    await selectNote(tabs[next].id)
  }

  function handleKeyboard(event: KeyboardEvent) {
    if (event.ctrlKey && event.key === 'Tab') {
      event.preventDefault()
      void cycleTab(event.shiftKey ? -1 : 1)
    } else if (event.ctrlKey && event.key.toLowerCase() === 'w') {
      event.preventDefault()
      if (activeNote) void closeTab(activeNote.id)
    } else if (event.ctrlKey && event.key.toLowerCase() === 's') {
      event.preventDefault()
      void flushSave().then(() => showToast('已保存'))
    } else if (event.ctrlKey && event.key.toLowerCase() === 'n') {
      event.preventDefault()
      void backend.createNote(null).then(async (note) => {
        await emitDesktop('qingjian://notes-changed')
        await backend.openNoteWindow(note.id, 'card')
      })
    } else if (event.ctrlKey && event.key === ',') {
      event.preventDefault()
      void backend.showHubWindow().then(() => emitDesktop('qingjian://settings'))
    } else if (event.key === 'Escape' && !menuOpen && !pagesMenuOpen) {
      void destroyEditor()
    }
  }

  onMount(() => {
    const cleanups: Array<() => void> = []
    let disposed = false
    void (async () => {
      try {
        ;[settings, fonts, workspace] = await Promise.all([
          backend.getSettings(), backend.listSystemFonts(), backend.getWorkspaceState(),
        ])
        fonts = Array.from(new Set([
          'Microsoft YaHei UI', 'DengXian', 'SimSun', 'SimHei', 'KaiTi',
          'Segoe UI Variable', ...fonts,
        ])).sort((a, b) => a.localeCompare(b, 'zh-CN'))
        await refreshTabs()
        const id = workspace.activeNoteId ?? tabs[0]?.id
        if (id) await selectNote(id)
        else await destroyEditor()

        if (!backend.isDesktop || disposed) return
        await getCurrentWindow().setAlwaysOnTop(settings.defaultNoteAlwaysOnTop)
        cleanups.push(await getCurrentWindow().onCloseRequested((event) => {
          event.preventDefault()
          void destroyEditor()
        }))
        cleanups.push(...await Promise.all([
          listen<string>('qingjian://open-note', (event) => void acceptOpenRequest(event.payload)),
          listen<AppSettings>('qingjian://settings-changed', (event) => {
            settings = event.payload
            void getCurrentWindow().setAlwaysOnTop(settings.defaultNoteAlwaysOnTop)
          }),
          listen('qingjian://flush-and-quit', () => {
            void flushSave().finally(() => emitDesktop('qingjian://ready-to-quit'))
          }),
        ]))
      } catch (error) {
        showToast(error instanceof Error ? error.message : '编辑器初始化失败')
      } finally {
        loading = false
      }
    })()
    return () => {
      disposed = true
      cleanups.forEach((cleanup) => cleanup())
      clearTimeout(saveTimer)
      clearTimeout(collapseTimer)
    }
  })
</script>

<svelte:window on:keydown={handleKeyboard} />

<main class={`app-window editor-window ${themeClass}`}>
  <div
    class="tab-hot-zone"
    role="region"
    aria-label="已打开便签"
    on:mouseenter={expandTabs}
    on:mouseleave={scheduleCollapse}
    on:focusin={expandTabs}
    on:focusout={scheduleCollapse}
  >
    <button class="tab-peek" class:expanded={tabsExpanded} aria-label="显示已打开便签" on:click={expandTabs}>
      <span class="peek-line"></span>
      <span class="peek-dots">
        {#each tabs.slice(0, 4) as tab}
          <i class:active={tab.id === activeNote?.id}></i>
        {/each}
      </span>
    </button>
    <div class="floating-tabs" class:visible={tabsExpanded}>
      <div class="tab-scroll">
        {#each tabs as tab (tab.id)}
          <button class="floating-tab" class:active={tab.id === activeNote?.id} on:click={() => void selectNote(tab.id)}>
            <span>{tab.title || '新便签'}</span>
            <i
              role="button"
              tabindex="0"
              aria-label={`关闭 ${tab.title}`}
              on:click={(event) => { event.stopPropagation(); void closeTab(tab.id) }}
              on:keydown={(event) => {
                if (event.key === 'Enter' || event.key === ' ') {
                  event.stopPropagation()
                  void closeTab(tab.id)
                }
              }}
            ><X /></i>
          </button>
        {/each}
      </div>
      <div class="pages-anchor">
        <button class="all-pages" aria-label="全部页面" use:tooltip={'全部页面'} on:click={() => (pagesMenuOpen = !pagesMenuOpen)}><ChevronDown /></button>
        {#if pagesMenuOpen}
          <div class="all-pages-menu" role="menu">
            {#each tabs as tab (tab.id)}
              <button class:active={tab.id === activeNote?.id} on:click={() => { pagesMenuOpen = false; void selectNote(tab.id) }}>
                <span>{tab.title || '新便签'}</span>
                {#if tab.id === activeNote?.id}<i></i>{/if}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  </div>

  <section class="note-shell">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <header class="titlebar editor-titlebar" on:mousedown={startWindowDrag}>
      <div class="titlebar-leading"><span class="app-mark">轻笺</span></div>
      <div class="titlebar-actions">
        <span class:visible={saveState !== 'saved'} class:error={saveState === 'error'} class="save-status">
          {saveState === 'saving' ? '保存中…' : saveState === 'error' ? '保存失败' : saveState === 'dirty' ? '未保存' : '已保存'}
        </span>
        <button class="ghost icon-only" aria-label={activeNote?.pinned ? '取消置顶' : '置顶'} use:tooltip={activeNote?.pinned ? '取消置顶便签' : '置顶便签'} on:click={() => void togglePin()}>
          {#if activeNote?.pinned}<PinOff />{:else}<Pin />{/if}
        </button>
        <button class="ghost icon-only" aria-label="插入图片" use:tooltip={'插入图片'} on:click={() => fileInput.click()}><ImagePlus /></button>
        <div class="menu-anchor">
          <button class="ghost icon-only" aria-label="更多" use:tooltip={'更多操作'} on:click={() => (menuOpen = !menuOpen)}><MoreHorizontal /></button>
          {#if menuOpen}
            <div class="more-menu">
              <small class="menu-group-label">复制内容</small>
              <button on:click={() => void copyContent('rich')}><Copy />保留源格式</button>
              <button on:click={() => void copyContent('text')}><Text />纯文本</button>
              <button on:click={() => void copyContent('markdown')}><Code2 />Markdown</button>
              <button on:click={() => void copyContent('html')}><Code2 />HTML 源码</button>
              <span class="menu-separator"></span>
              <button on:click={() => void exportMarkdown()}><FileDown />导出 Markdown</button>
              <button on:click={() => { menuOpen = false; void exportHtml() }}><FileDown />导出 HTML</button>
              <button on:click={() => void exportPdf()}><Download />导出 PDF</button>
              <button on:click={() => { menuOpen = false; void toggleArchive() }}>
                {#if activeNote?.archived}<ArchiveRestore />恢复便签{:else}<Archive />归档便签{/if}
              </button>
              <button on:click={() => { menuOpen = false; void backend.showHubWindow().then(() => emitDesktop('qingjian://settings')) }}><Settings />打开设置</button>
              <span class="menu-separator"></span>
              <button class="danger" on:click={() => { menuOpen = false; void deleteCurrent() }}><Trash2 />移到回收站</button>
            </div>
          {/if}
        </div>
        <span class="window-divider"></span>
        <button class="ghost icon-only window-action" aria-label="最小化" use:tooltip={'最小化'} on:click={() => getCurrentWindow().minimize()}><Minus /></button>
        <button class="ghost icon-only window-action close-action" aria-label="关闭编辑器" use:tooltip={'保存并关闭编辑器'} on:click={() => void destroyEditor()}><X /></button>
      </div>
    </header>

    {#if loading}
      <div class="loading-state"><span class="loading-mark">轻</span><p>正在打开便签…</p></div>
    {:else if activeNote}
      <div class="note-heading">
        <input
          class="note-title"
          aria-label="便签标题"
          value={activeNote.title}
          on:input={(event) => {
            activeNote = { ...activeNote!, title: event.currentTarget.value }
            tabs = tabs.map((tab) => tab.id === activeNote?.id ? { ...tab, title: activeNote.title } : tab)
            updateMeta()
          }}
        />
        <input class="tag-input" aria-label="便签标签" placeholder="# 添加标签" bind:value={tagDraft} on:change={updateMeta} />
      </div>
      <Toolbar {editor} {fonts} version={toolbarVersion} onPickImage={() => fileInput.click()} />
      <div class="editor-scroll">
        <Editor
          bind:this={editorView}
          noteId={activeNote.id}
          content={activeNote.content}
          defaultFontFamily={settings.defaultFontFamily}
          defaultFontSize={settings.defaultFontSize}
          onChange={scheduleSave}
          onReady={(instance) => (editor = instance)}
          onSelectionChange={() => (toolbarVersion += 1)}
          onError={showToast}
        />
      </div>
    {/if}
  </section>
</main>

<input
  bind:this={fileInput}
  class="hidden-file-input"
  type="file"
  accept="image/png,image/jpeg,image/webp,image/gif,image/bmp"
  multiple
  on:change={(event) => {
    if (event.currentTarget.files) void editorView.insertFiles(event.currentTarget.files)
    event.currentTarget.value = ''
  }}
/>

{#if toast}<div class="toast" role="status">{toast}</div>{/if}
