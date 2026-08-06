<script lang="ts">
  import { Download, RefreshCw, X } from 'lucide-svelte'
  import type { AppUpdateInfo, UpdateDownloadProgress } from '../lib/updater'

  export let update: AppUpdateInfo
  export let installing = false
  export let progress: UpdateDownloadProgress = { downloadedBytes: 0, finished: false }
  export let error = ''
  export let onInstall: () => void
  export let onLater: () => void

  $: percent = progress.totalBytes
    ? Math.min(100, Math.round(progress.downloadedBytes / progress.totalBytes * 100))
    : 0

  function formatBytes(value: number) {
    if (value < 1024 * 1024) return `${Math.max(1, Math.round(value / 1024))} KB`
    return `${(value / 1024 / 1024).toFixed(1)} MB`
  }
</script>

<div class="modal-backdrop update-backdrop" role="presentation" on:mousedown={(event) => {
  if (!installing && event.target === event.currentTarget) onLater()
}}>
  <div class="update-modal" role="dialog" aria-modal="true" aria-label="发现轻笺新版本">
    <header>
      <span class="update-icon"><RefreshCw /></span>
      <div>
        <span class="eyebrow">QINGJIAN UPDATE</span>
        <h2>轻笺 {update.version} 已准备好</h2>
        <p>当前版本 {update.currentVersion}</p>
      </div>
      <button class="ghost icon-only" disabled={installing} aria-label="稍后提醒" on:click={onLater}><X /></button>
    </header>

    <div class="update-body">
      <strong>本次更新</strong>
      <div class="release-notes">{update.body?.trim() || '包含新的功能改进、体验优化和问题修复。'}</div>
      {#if update.date}<small>发布于 {new Date(update.date).toLocaleDateString('zh-CN')}</small>{/if}
    </div>

    {#if installing}
      <div class="update-progress" aria-live="polite">
        <div><span style={`width:${progress.totalBytes ? percent : 18}%`}></span></div>
        <p>
          {progress.finished ? '正在安装并准备重启…' : '正在下载更新…'}
          {#if progress.totalBytes}
            <b>{formatBytes(progress.downloadedBytes)} / {formatBytes(progress.totalBytes)}</b>
          {/if}
        </p>
      </div>
    {/if}

    {#if error}<p class="update-error" role="alert">{error}</p>{/if}

    <footer>
      <button class="secondary" disabled={installing} on:click={onLater}>稍后提醒</button>
      <button class="primary update-install" disabled={installing} on:click={onInstall}>
        {#if installing}<RefreshCw class="spin" />安装中…{:else}<Download />立即更新并重启{/if}
      </button>
    </footer>
  </div>
</div>
