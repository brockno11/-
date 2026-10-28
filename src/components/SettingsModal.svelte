<script lang="ts">
  import { onMount, tick } from 'svelte'
  import { ArrowLeft, ChevronRight, X } from 'lucide-svelte'
  import type { AppSettings } from '../lib/types'
  import { isReservedGlobalHotkey } from '../lib/types'
  import { tooltip } from '../lib/tooltip'

  export let settings: AppSettings
  export let onSave: (settings: AppSettings) => Promise<void>
  export let onClose: () => void
  export let onBackup: () => void
  export let onRestore: () => void
  export let appVersion = '0.8.0'
  export let checkingUpdate = false
  export let onCheckUpdate: () => void

  type Section = 'general' | 'note' | 'shortcut' | 'data' | 'about' | 'advanced'
  const sections: Array<{ id: Section; label: string; hint: string }> = [
    { id: 'general', label: '常规', hint: '主题与启动行为' },
    { id: 'note', label: '便签外观', hint: '文字、待办与默认样式' },
    { id: 'shortcut', label: '快捷键', hint: '唤起与救援操作' },
    { id: 'data', label: '数据与备份', hint: '导出和恢复本地数据' },
    { id: 'about', label: '关于与更新', hint: '版本和联网说明' },
    { id: 'advanced', label: '高级', hint: '窗口停靠与可见性' },
  ]

  let narrow = window.innerWidth <= 520
  let active: Section | null = narrow ? null : 'general'
  let draft = structuredClone(settings)
  let persisted = structuredClone(settings)
  let saving = false
  let pendingCommit = false
  let saved = false
  let capturing = false
  let error = ''
  let savedTimer: ReturnType<typeof setTimeout> | undefined
  let dialogElement: HTMLDivElement
  let closeButton: HTMLButtonElement

  function focusableElements() {
    return Array.from(dialogElement.querySelectorAll<HTMLElement>(
      'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])',
    )).filter((element) => element.getAttribute('aria-hidden') !== 'true')
  }

  function trapDialogFocus(event: KeyboardEvent) {
    if (event.key !== 'Tab') return
    const focusable = focusableElements()
    if (!focusable.length) { event.preventDefault(); dialogElement.focus(); return }
    const first = focusable[0]
    const last = focusable[focusable.length - 1]
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault(); last.focus()
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault(); first.focus()
    }
  }

  async function commit() {
    if (saving) { pendingCommit = true; return }
    if (isReservedGlobalHotkey(draft.hotkey)) {
      error = `${draft.hotkey} 是轻笺的固定救援快捷键，请换一个组合`
      draft = structuredClone(persisted)
      return
    }
    const candidate = structuredClone(draft)
    saving = true
    error = ''
    try {
      await onSave(candidate)
      persisted = candidate
      saved = true
      clearTimeout(savedTimer)
      savedTimer = setTimeout(() => (saved = false), 1600)
    } catch (reason) {
      draft = structuredClone(persisted)
      error = reason instanceof Error ? reason.message : '设置保存失败，已恢复原值'
    } finally {
      saving = false
      if (pendingCommit) { pendingCommit = false; void commit() }
    }
  }

  function captureHotkey(event: KeyboardEvent) {
    if (!capturing) return
    event.preventDefault()
    event.stopPropagation()
    if (['Control', 'Alt', 'Shift', 'Meta'].includes(event.key)) return
    const modifiers: string[] = []
    if (event.ctrlKey) modifiers.push('Ctrl')
    if (event.altKey) modifiers.push('Alt')
    if (event.shiftKey) modifiers.push('Shift')
    if (event.metaKey) modifiers.push('Super')
    if (!modifiers.length) { error = '快捷键至少需要一个 Ctrl、Alt、Shift 或 Win 修饰键'; return }
    const key = event.key.length === 1 ? event.key.toUpperCase() : event.key
    const hotkey = [...modifiers, key].join('+')
    if (isReservedGlobalHotkey(hotkey)) { error = `${hotkey} 是轻笺的固定救援快捷键，请换一个组合`; return }
    draft.hotkey = hotkey
    capturing = false
    error = ''
    void commit()
  }

  function updateLayout() {
    const nextNarrow = window.innerWidth <= 520
    if (!nextNarrow && active === null) active = 'general'
    narrow = nextNarrow
  }

  async function selectSection(section: Section) {
    active = section
    await tick()
    if (narrow) dialogElement.querySelector<HTMLButtonElement>('.settings-back')?.focus()
  }

  async function showCategories() {
    const previous = active
    active = null
    await tick()
    dialogElement.querySelector<HTMLButtonElement>(`[data-section="${previous}"]`)?.focus()
  }

  onMount(() => {
    const returnFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    const background = document.querySelector<HTMLElement>('main.qj-workbench')
    const previousInert = background?.inert ?? false
    const previousAriaHidden = background ? background.getAttribute('aria-hidden') : null
    if (background) {
      background.inert = true
      background.setAttribute('aria-hidden', 'true')
    }
    queueMicrotask(() => closeButton?.focus())

    return () => {
      clearTimeout(savedTimer)
      if (background) {
        background.inert = previousInert
        if (previousAriaHidden === null) background.removeAttribute('aria-hidden')
        else background.setAttribute('aria-hidden', previousAriaHidden)
      }
      queueMicrotask(() => {
        if (returnFocus?.isConnected) returnFocus.focus()
      })
    }
  })
</script>

<svelte:window on:keydown={captureHotkey} on:resize={updateLayout} />

<div class="modal-backdrop settings-backdrop" role="presentation">
  <div bind:this={dialogElement} class="settings-modal settings-page" role="dialog" aria-modal="true" aria-label="轻笺设置" tabindex="-1" on:keydown={trapDialogFocus}>
    <header class="settings-page-header">
      <div><h2>设置</h2><p>更改会自动保存在本机</p></div>
      <div class="settings-save-state" aria-live="polite">{saving ? '保存中…' : saved ? '已保存' : ''}</div>
      <button bind:this={closeButton} class="ghost icon-only" aria-label="关闭设置" use:tooltip={'关闭设置'} on:click={onClose}><X /></button>
    </header>

    <div class:narrow class="settings-layout">
      {#if !narrow || active === null}
      <nav class:category-home={narrow} class="settings-nav" aria-label="设置分类">
        {#each sections as section}
          <button data-section={section.id} class:active={active === section.id} aria-current={active === section.id ? 'page' : undefined} on:click={() => void selectSection(section.id)}><span><strong>{section.label}</strong><small>{section.hint}</small></span><ChevronRight /></button>
        {/each}
      </nav>
      {/if}

      {#if !narrow || active !== null}
      <div class="settings-content">
        {#if narrow}<button class="settings-back" on:click={() => void showCategories()}><ArrowLeft />所有设置</button>{/if}
        {#if active === 'general'}
          <div class="settings-section-heading"><h3>常规</h3><p>控制轻笺的基础外观和启动方式。</p></div>
          <label class="setting-item"><span><strong>主题</strong><small>默认白色是主要设计基底</small></span><select bind:value={draft.themeId} on:change={() => void commit()}><option value="default-light">默认白色</option><option value="system">跟随系统</option><option value="dark">深色</option></select></label>
          <label class="setting-item"><span><strong>开机自动启动</strong><small>静默驻留托盘，不主动弹出工作台</small></span><input class="switch" type="checkbox" bind:checked={draft.startOnBoot} on:change={() => void commit()} /></label>
          <label class="setting-item"><span><strong>恢复桌面便签</strong><small>开机后恢复上次可见便签</small></span><input class="switch" type="checkbox" bind:checked={draft.restoreNoteWindowsOnBoot} on:change={() => void commit()} /></label>
        {:else if active === 'note'}
          <div class="settings-section-heading"><h3>便签外观</h3><p>未单独设置的文字与待办会跟随默认；窗口默认仅影响新建便签。</p></div>
          <label class="setting-item"><span><strong>默认置顶</strong><small>新便签保持在其他窗口上方</small></span><input class="switch" type="checkbox" bind:checked={draft.defaultNoteAlwaysOnTop} on:change={() => void commit()} /></label>
          <div class="setting-item setting-item-stack"><span><strong>默认可见度</strong><small>分别控制纸张和正文文字</small></span><div class="settings-range"><label>背景 <input type="range" min="0" max="100" step="5" bind:value={draft.defaultNoteBackgroundOpacity} on:change={() => void commit()} /><b>{draft.defaultNoteBackgroundOpacity}%</b></label><label>文字 <input type="range" min="20" max="100" step="5" bind:value={draft.defaultNoteTextOpacity} on:change={() => void commit()} /><b>{draft.defaultNoteTextOpacity}%</b></label></div></div>
          <div class="setting-item"><span><strong>默认文字</strong><small>字体和字号</small></span><div class="default-type-controls"><select aria-label="默认字体" bind:value={draft.defaultFontFamily} on:change={() => void commit()}><option value="Microsoft YaHei UI">微软雅黑</option><option value="DengXian">等线</option><option value="SimSun">宋体</option><option value="SimHei">黑体</option><option value="KaiTi">楷体</option><option value="Segoe UI Variable">Segoe UI</option></select><input type="number" min="9" max="72" bind:value={draft.defaultFontSize} on:change={() => void commit()} aria-label="默认字号" /></div></div>
          <label class="setting-item"><span><strong>正文行距</strong><small>未单独设置行距的便签会跟随</small></span><select bind:value={draft.defaultLineHeight} on:change={() => void commit()}><option value={1.4}>紧凑</option><option value={1.55}>舒适</option><option value={1.7}>宽松</option></select></label>
          <label class="setting-item"><span><strong>待办圆圈</strong><small>单张便签可单独设置，完成后留在原处</small></span><select bind:value={draft.defaultTaskMarkerSize} on:change={() => void commit()}><option value={0}>随字号缩放</option>{#each Array.from({ length: 25 }, (_, i) => i + 12) as size}<option value={size}>{size}px</option>{/each}</select></label>
          <label class="setting-item"><span><strong>胶囊内容</strong><small>选择摘要信息密度</small></span><select bind:value={draft.capsuleDisplay} on:change={() => void commit()}><option value="comfortable">标题 + 两行正文</option><option value="compact">单行摘要</option></select></label>
        {:else if active === 'shortcut'}
          <div class="settings-section-heading"><h3>快捷键</h3><p>快捷键失败不会影响轻笺正常启动。</p></div>
          <div class="setting-item"><span><strong>全局快捷键</strong><small>随时打开上次便签或新建便签</small></span><button class:capturing class="hotkey-capture" on:click={() => (capturing = true)}>{capturing ? '请按组合键…' : draft.hotkey}</button></div>
          <label class="setting-item"><span><strong>快捷键动作</strong><small>决定按下快捷键后的行为</small></span><select bind:value={draft.hotkeyAction} on:change={() => void commit()}><option value="last-note">打开上次便签</option><option value="new-note">新建便签</option></select></label>
          <div class="shortcut-reference"><strong>救援快捷键</strong><p><kbd>Ctrl+Alt+H</kbd> 显示或隐藏桌面便签</p><p><kbd>Ctrl+Alt+L</kbd> 解锁鼠标穿透便签</p><p><kbd>Ctrl+Alt+O</kbd> 恢复便签可见度</p></div>
        {:else if active === 'data'}
          <div class="settings-section-heading"><h3>数据与备份</h3><p>便签正文、设置和图片均保存在本机。</p></div>
          <div class="setting-item"><span><strong>完整备份</strong><small>导出全部便签、标签、设置和图片</small></span><button class="secondary" on:click={onBackup}>导出备份</button></div>
          <div class="setting-item"><span><strong>恢复数据</strong><small>默认合并导入，不覆盖已有便签</small></span><button class="secondary" on:click={onRestore}>选择备份</button></div>
          <div class="local-data-note"><strong>本地优先</strong><p>日常编辑、搜索、保存、导出和恢复都不需要网络。</p></div>
        {:else if active === 'about'}
          <div class="settings-section-heading"><h3>关于与更新</h3><p>当前版本 {appVersion}</p></div>
          <label class="setting-item"><span><strong>自动检查更新</strong><small>仅此功能会访问网络；失败不影响本地使用</small></span><input class="switch" type="checkbox" bind:checked={draft.checkUpdatesOnStartup} on:change={() => void commit()} /></label>
          <div class="setting-item"><span><strong>软件更新</strong><small>手动检查 GitHub 正式版本</small></span><button class="secondary" disabled={checkingUpdate} on:click={onCheckUpdate}>{checkingUpdate ? '检查中…' : '检查更新'}</button></div>
        {:else}
          <div class="settings-section-heading"><h3>高级</h3><p>窗口停靠和可见性相关设置。</p></div>
          <label class="setting-item"><span><strong>贴边自动收起</strong><small>关闭后仍可贴边，但不会缩成边缘标记</small></span><input class="switch" type="checkbox" bind:checked={draft.autoHideAtScreenEdge} on:change={() => void commit()} /></label>
          <div class="advanced-note"><strong>鼠标穿透</strong><p>锁定属于单张便签操作。锁定后使用 Ctrl+Alt+L 解锁。</p></div>
        {/if}

        {#if error}<p class="settings-error" role="alert">{error}</p>{/if}
      </div>
      {/if}
    </div>
  </div>
</div>
