<script lang="ts">
  import { Search, Plus, Pin, Archive, Settings, ChevronLeft, Minus, X } from 'lucide-svelte'
  import type { NoteSummary } from '../lib/types'
  import { tooltip } from '../lib/tooltip'

  export let notes: NoteSummary[] = []
  export let activeId = ''
  export let query = ''
  export let onQuery: (value: string) => void
  export let onSelect: (id: string) => void
  export let onNew: () => void
  export let onSettings: () => void
  export let onCollapse: () => void
  export let onDragStart: (event: MouseEvent) => void
  export let standalone = false
  export let onMinimize: () => void = () => {}
  export let onClose: () => void = () => {}

  const dateFormat = new Intl.DateTimeFormat('zh-CN', {
    month: 'numeric',
    day: 'numeric',
    hour: '2-digit',
    minute: '2-digit',
  })
</script>

<aside class="sidebar" class:standalone>
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="sidebar-drag" on:mousedown={onDragStart}>
    {#if standalone}
      <span class="navigator-title">便签目录</span>
      <div class="navigator-window-actions">
        <button class="ghost icon-only window-action" aria-label="最小化目录" use:tooltip={'最小化目录'} on:click={onMinimize}><Minus /></button>
        <button class="ghost icon-only window-action close-action" aria-label="关闭目录" use:tooltip={'关闭目录'} on:click={onClose}><X /></button>
      </div>
    {:else}
      <button class="ghost icon-only collapse-button" aria-label="折叠侧栏" use:tooltip={'折叠侧栏'} on:click={onCollapse}><ChevronLeft /></button>
    {/if}
  </div>

  <div class="sidebar-actions">
    <div class="search-field">
      <Search />
      <input
        aria-label="搜索便签"
        placeholder="搜索便签"
        value={query}
        on:input={(event) => onQuery(event.currentTarget.value)}
      />
    </div>
    <button class="new-note" on:click={onNew}><Plus /> 新建便签</button>
  </div>

  <div class="note-list">
    {#each notes as note (note.id)}
      <button
        class="note-card"
        class:active={note.id === activeId}
        on:click={() => onSelect(note.id)}
      >
        <div class="note-card-title">
          <strong>{note.title || '新便签'}</strong>
          {#if note.pinned}<Pin class="pin-icon" />{/if}
        </div>
        <p>{note.preview || '空白便签'}</p>
        <div class="note-meta">
          <time>{dateFormat.format(new Date(note.updatedAt))}</time>
          {#if note.archived}<Archive />{/if}
          {#each note.tags.slice(0, 2) as tag}
            <span class="tag-dot" style={`--tag-color:${tag.color}`}>{tag.name}</span>
          {/each}
        </div>
      </button>
    {:else}
      <div class="empty-list">
        <span>没有找到便签</span>
        <button class="link-button" on:click={onNew}>新建一张</button>
      </div>
    {/each}
  </div>

  <div class="sidebar-footer">
    <span>{notes.length} 张便签</span>
    <button class="ghost icon-only" aria-label="设置" use:tooltip={'设置 · Ctrl+,'} on:click={onSettings}><Settings /></button>
  </div>
</aside>
