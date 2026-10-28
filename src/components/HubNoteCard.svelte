<script lang="ts">
  import { Check, Folder, MoreHorizontal, Star } from 'lucide-svelte'
  import type { Folder as NoteFolder, NoteSummary } from '../lib/types'
  import { tooltip } from '../lib/tooltip'

  export let note: NoteSummary
  export let folders: NoteFolder[] = []
  export let query = ''
  export let active = false
  export let selected = false
  export let selectionMode = false
  export let desktopVisible = false
  export let dragEnabled = false
  export let dropBefore = false
  export let formattedDate = ''
  export let onActivate: (note: NoteSummary) => void
  export let onTogglePin: (note: NoteSummary) => void
  export let onOpenMenu: (event: MouseEvent, note: NoteSummary) => void
  export let onContextMenu: (event: MouseEvent, note: NoteSummary) => void
  export let onDragStart: (event: DragEvent, note: NoteSummary) => void
  export let onDragOver: (event: DragEvent, note: NoteSummary) => void
  export let onDrop: (event: DragEvent, note: NoteSummary) => void
  export let onDragEnd: () => void

  $: folderName = note.folderId
    ? folders.find((folder) => folder.id === note.folderId)?.name || '文件夹'
    : ''

  function highlighted(value: string) {
    const needle = query.trim()
    if (!needle) return [{ text: value, match: false }]
    const index = value.toLocaleLowerCase().indexOf(needle.toLocaleLowerCase())
    if (index < 0) return [{ text: value, match: false }]
    return [
      { text: value.slice(0, index), match: false },
      { text: value.slice(index, index + needle.length), match: true },
      { text: value.slice(index + needle.length), match: false },
    ].filter((part) => part.text)
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      onActivate(note)
    } else if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) {
      event.preventDefault()
      const target = event.currentTarget as HTMLElement
      const rect = target.getBoundingClientRect()
      onOpenMenu(new MouseEvent('click', { clientX: rect.right - 16, clientY: rect.top + 24 }), note)
    }
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="workbench-note-card"
  class:active
  class:selected
  class:drop-before={dropBefore}
  class:selection-mode={selectionMode}
  class:drag-enabled={dragEnabled}
  data-note-id={note.id}
  draggable={dragEnabled}
  on:contextmenu={(event) => onContextMenu(event, note)}
  on:dragstart={(event) => onDragStart(event, note)}
  on:dragover={(event) => onDragOver(event, note)}
  on:drop={(event) => onDrop(event, note)}
  on:dragend={onDragEnd}
>
  {#if selectionMode}
    <button
      class="note-check"
      class:checked={selected}
      aria-label={selected ? `取消勾选 ${note.title}` : `勾选 ${note.title}`}
      aria-pressed={selected}
      on:click={() => onActivate(note)}
    >{#if selected}<Check />{/if}</button>
  {/if}

  <button
    class="workbench-card-main"
    aria-label={selectionMode
      ? selected ? `取消选择 ${note.title}` : `选择 ${note.title}`
      : `打开 ${note.title}`}
    aria-pressed={selectionMode ? selected : undefined}
    on:click={() => onActivate(note)}
    on:keydown={handleKeydown}
  >
  <span class="workbench-card-copy">
    <span class="workbench-card-title">
      <strong>
        {#each highlighted(note.title || '新便签') as part}
          {#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}
        {/each}
      </strong>
      {#if desktopVisible}
        <span class="desktop-status" aria-label="已显示在桌面" use:tooltip={'已显示在桌面'}></span>
      {/if}
      {#if note.pinned && !note.trashedAt}<Star class="pinned-mark" aria-label="已置顶" />{/if}
      <time>{formattedDate}</time>
    </span>

    <span class="workbench-card-preview">
      {#each highlighted(note.preview || '空白便签') as part}
        {#if part.match}<mark>{part.text}</mark>{:else}{part.text}{/if}
      {/each}
    </span>

    {#if folderName || note.tags.length || note.trashedAt}
      <span class="workbench-card-meta">
        {#if folderName}<span class="meta-folder"><Folder />{folderName}</span>{/if}
        {#each note.tags.slice(0, 2) as tag}
          <span class="meta-tag"><i style={`--tag-color:${tag.color}`}></i>{tag.name}</span>
        {/each}
        {#if note.tags.length > 2}<span>+{note.tags.length - 2}</span>{/if}
        {#if note.trashedAt}<span>回收站</span>{/if}
      </span>
    {/if}
  </span>
  </button>

  {#if !note.trashedAt && !selectionMode}
    <div class="workbench-card-actions">
      <button
        class:active={note.pinned}
        aria-label={note.pinned ? '取消列表置顶' : '列表置顶'}
        use:tooltip={note.pinned ? '取消列表置顶' : '列表置顶'}
        on:click|stopPropagation={() => onTogglePin(note)}
      ><Star /></button>
      <button
        aria-label="更多操作"
        use:tooltip={'更多操作'}
        on:click|stopPropagation={(event) => onOpenMenu(event, note)}
      ><MoreHorizontal /></button>
    </div>
  {/if}
</div>
