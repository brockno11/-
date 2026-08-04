<script lang="ts">
  import {
    X, Keyboard, SunMoon, Rocket, Download, Upload, MousePointerClick, Pin, Type,
  } from 'lucide-svelte'
  import type { AppSettings, Theme } from '../lib/types'
  import { tooltip } from '../lib/tooltip'

  export let settings: AppSettings
  export let onSave: (settings: AppSettings) => Promise<void>
  export let onClose: () => void
  export let onBackup: () => void
  export let onRestore: () => void

  let draft = structuredClone(settings)
  let saving = false
  let capturing = false
  let error = ''

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
    if (!modifiers.length) {
      error = '快捷键至少需要一个 Ctrl、Alt、Shift 或 Win 修饰键'
      return
    }
    const key = event.key.length === 1 ? event.key.toUpperCase() : event.key
    draft.hotkey = [...modifiers, key].join('+')
    capturing = false
    error = ''
  }

  async function save() {
    saving = true
    error = ''
    try {
      await onSave(draft)
      onClose()
    } catch (reason) {
      error = reason instanceof Error ? reason.message : '设置保存失败'
    } finally {
      saving = false
    }
  }
</script>

<svelte:window on:keydown={captureHotkey} />

<div class="modal-backdrop" role="presentation" on:mousedown={(event) => event.target === event.currentTarget && onClose()}>
  <div class="settings-modal" role="dialog" aria-modal="true" aria-label="轻笺设置">
    <header>
      <div>
        <span class="eyebrow">QINGJIAN</span>
        <h2>设置</h2>
      </div>
      <button class="ghost icon-only" aria-label="关闭设置" use:tooltip={'关闭设置'} on:click={onClose}><X /></button>
    </header>

    <div class="settings-body">
      <label class="setting-row">
        <span class="setting-icon"><Keyboard /></span>
        <span class="setting-copy"><strong>全局快捷键</strong><small>随时唤起轻笺，默认打开核心工作台</small></span>
        <button
          class="hotkey-capture"
          class:capturing
          on:click={() => (capturing = true)}
        >{capturing ? '请按组合键…' : draft.hotkey}</button>
      </label>

      <label class="setting-row">
        <span class="setting-icon"><MousePointerClick /></span>
        <span class="setting-copy"><strong>快捷键动作</strong><small>选择按下全局快捷键时首先打开什么</small></span>
        <select bind:value={draft.hotkeyAction}>
          <option value="hub">核心工作台</option>
          <option value="last-note">上次便签</option>
          <option value="new-note">新建便签</option>
        </select>
      </label>

      <label class="setting-row">
        <span class="setting-icon"><SunMoon /></span>
        <span class="setting-copy"><strong>外观</strong><small>暖白主题会随系统切换深色模式</small></span>
        <select bind:value={draft.theme}>
          <option value="system">跟随系统</option>
          <option value="light">浅色</option>
          <option value="dark">深色</option>
        </select>
      </label>

      <label class="setting-row">
        <span class="setting-icon"><Pin /></span>
        <span class="setting-copy"><strong>窗口始终置顶</strong><small>工作台和打开的编辑器保持在其他窗口上方</small></span>
        <input class="switch" type="checkbox" bind:checked={draft.alwaysOnTop} />
      </label>

      <div class="setting-row">
        <span class="setting-icon"><Type /></span>
        <span class="setting-copy"><strong>默认文字</strong><small>新输入文字使用的字体与字号</small></span>
        <div class="default-type-controls">
          <select bind:value={draft.defaultFontFamily} aria-label="默认字体">
            <option value="Microsoft YaHei UI">微软雅黑</option>
            <option value="DengXian">等线</option>
            <option value="SimSun">宋体</option>
            <option value="SimHei">黑体</option>
            <option value="KaiTi">楷体</option>
            <option value="Segoe UI Variable">Segoe UI</option>
          </select>
          <input type="number" min="9" max="72" bind:value={draft.defaultFontSize} aria-label="默认字号" />
        </div>
      </div>

      <label class="setting-row">
        <span class="setting-icon"><Rocket /></span>
        <span class="setting-copy"><strong>开机自动启动</strong><small>静默驻留托盘，不主动弹出窗口</small></span>
        <input class="switch" type="checkbox" bind:checked={draft.startOnBoot} />
      </label>

      <div class="setting-row">
        <span class="setting-icon"><Download /></span>
        <span class="setting-copy"><strong>完整备份</strong><small>保存全部便签、标签、设置和图片</small></span>
        <button class="secondary" on:click={onBackup}>导出备份</button>
      </div>

      <div class="setting-row">
        <span class="setting-icon"><Upload /></span>
        <span class="setting-copy"><strong>恢复数据</strong><small>默认合并导入，不覆盖现有便签</small></span>
        <button class="secondary" on:click={onRestore}>选择备份</button>
      </div>
    </div>

    {#if error}<p class="settings-error">{error}</p>{/if}

    <footer>
      <span>数据仅保存在这台电脑上</span>
      <div>
        <button class="secondary" on:click={onClose}>取消</button>
        <button class="primary" disabled={saving} on:click={save}>{saving ? '保存中…' : '保存设置'}</button>
      </div>
    </footer>
  </div>
</div>
