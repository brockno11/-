<script lang="ts">
  import type { Editor } from '@tiptap/core'
  import { NodeSelection } from '@tiptap/pm/state'
  import {
    Bold, Italic, Underline, Strikethrough, List, ListOrdered, CheckSquare,
    AlignLeft, AlignCenter, AlignRight, ImagePlus, Table2, Rows3, Columns3,
    Trash2, Minus, Plus, Image as ImageIcon, RotateCcw, WrapText,
  } from 'lucide-svelte'
  import { normalizeImageLayout, type ImageLayout } from '../lib/editor'
  import { tooltip } from '../lib/tooltip'

  export let editor: Editor | undefined
  export let fonts: string[] = []
  export let version = 0
  export let onPickImage: () => void

  const sizes = [9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 48, 64, 72]
  const preferredFonts = [
    'Microsoft YaHei UI', 'Microsoft YaHei', 'DengXian',
    'SimSun', 'SimHei', 'KaiTi', 'Segoe UI Variable', 'Arial', 'Consolas',
  ]
  let rememberedSelection: { from: number; to: number } | null = null
  $: currentFont = version >= 0
    ? editor?.getAttributes('textStyle').fontFamily || ''
    : ''
  $: currentSize = version >= 0
    ? Number.parseInt(editor?.getAttributes('textStyle').fontSize || '16', 10)
    : 16
  $: inTable = version >= 0 && (editor?.isActive('table') ?? false)
  $: imageSelected = version >= 0 && editor?.state.selection instanceof NodeSelection
    && editor.state.selection.node.type.name === 'image'
  $: imageAttributes = version >= 0 ? (editor?.getAttributes('image') ?? {}) : {}
  $: imageLayout = normalizeImageLayout(imageAttributes.layout)
  $: imageWidth = Number(imageAttributes.width) || 0
  $: fontChoices = Array.from(new Set([
    ...(currentFont ? [currentFont] : []),
    ...preferredFonts.filter((font) => fonts.includes(font)),
  ]))

  function command(action: () => boolean | void) {
    action()
    editor?.commands.focus()
  }

  function rememberTextSelection() {
    if (!editor) return
    rememberedSelection = {
      from: editor.state.selection.from,
      to: editor.state.selection.to,
    }
  }

  function applyFont(font: string) {
    if (!editor) return
    let chain = editor.chain().focus()
    if (rememberedSelection) chain = chain.setTextSelection(rememberedSelection)
    if (font) chain.setFontFamily(font).run()
    else chain.unsetFontFamily().run()
    rememberedSelection = null
  }

  function applyFontSize(size: string) {
    if (!editor) return
    let chain = editor.chain().focus()
    if (rememberedSelection) chain = chain.setTextSelection(rememberedSelection)
    chain.setFontSize(`${size}px`).run()
    rememberedSelection = null
  }

  function editorContentWidth() {
    if (!editor) return 0
    const dom = editor.view.dom as HTMLElement
    const style = getComputedStyle(dom)
    return dom.clientWidth
      - Number.parseFloat(style.paddingLeft)
      - Number.parseFloat(style.paddingRight)
  }

  function selectedImage() {
    if (!editor) return null
    const selectedDom = editor.view.nodeDOM(editor.state.selection.from)
    return selectedDom instanceof HTMLImageElement
      ? selectedDom
      : selectedDom instanceof HTMLElement
        ? selectedDom.querySelector('img')
        : null
  }

  function imageRatio(image: HTMLImageElement | null) {
    const storedWidth = Number(imageAttributes.width)
    const storedHeight = Number(imageAttributes.height)
    if (storedWidth > 0 && storedHeight > 0) return storedWidth / storedHeight
    if (image?.naturalWidth && image?.naturalHeight) return image.naturalWidth / image.naturalHeight
    if (image?.clientWidth && image?.clientHeight) return image.clientWidth / image.clientHeight
    return 1
  }

  function updateImageSize(width: number, ratio: number) {
    if (!editor?.isActive('image')) return
    const safeWidth = Math.max(60, Math.round(width))
    const height = Math.max(40, Math.round(safeWidth / Math.max(ratio, 0.01)))
    editor.chain().focus().updateAttributes('image', { width: safeWidth, height }).run()
  }

  function setImageWidth(percent: number) {
    if (!editor?.isActive('image')) return
    updateImageSize(editorContentWidth() * percent, imageRatio(selectedImage()))
  }

  function restoreImageSize() {
    const image = selectedImage()
    if (!image?.naturalWidth || !image.naturalHeight) return
    updateImageSize(
      Math.min(image.naturalWidth, editorContentWidth()),
      image.naturalWidth / image.naturalHeight,
    )
  }

  function setImageLayout(layout: ImageLayout) {
    if (!editor?.isActive('image')) return
    editor.chain().focus().updateAttributes('image', { layout }).run()
  }
</script>

<div
  class="format-toolbar"
  class:image-mode={imageSelected}
  aria-label={imageSelected ? '图片工具栏' : '格式工具栏'}
  data-version={version}
>
  {#if imageSelected}
    <span class="image-toolbar-title"><ImageIcon />图片</span>
    <span class="image-size-label" aria-label="当前图片宽度">
      {imageWidth ? `${Math.round(imageWidth)}px` : '原始尺寸'}
    </span>
    <span class="toolbar-divider"></span>
    {#each [0.25, 0.5, 0.75, 1] as scale}
      <button
        class="text-button"
        aria-label={`图片宽度 ${scale * 100}%`}
        use:tooltip={`图片宽度 ${scale * 100}%`}
        on:click={() => setImageWidth(scale)}
      >{scale * 100}%</button>
    {/each}
    <button class="image-command" aria-label="恢复适合正文的原始尺寸" use:tooltip={'恢复适合正文的原始尺寸'} on:click={restoreImageSize}>
      <RotateCcw />原始
    </button>
    <span class="toolbar-divider"></span>
    <div class="image-layout-controls" aria-label="图片对齐与文字环绕">
      <button class="layout-button" class:active={imageLayout === 'block-left'} aria-label="独占一行并左对齐" use:tooltip={'独占一行并左对齐'} on:click={() => setImageLayout('block-left')}><AlignLeft />左</button>
      <button class="layout-button" class:active={imageLayout === 'block-center'} aria-label="独占一行并居中" use:tooltip={'独占一行并居中'} on:click={() => setImageLayout('block-center')}><AlignCenter />中</button>
      <button class="layout-button" class:active={imageLayout === 'block-right'} aria-label="独占一行并右对齐" use:tooltip={'独占一行并右对齐'} on:click={() => setImageLayout('block-right')}><AlignRight />右</button>
      <button class="layout-button wrap-button" class:active={imageLayout === 'wrap-left'} aria-label="图片靠左，文字在右侧环绕" use:tooltip={'图片靠左，文字在右侧环绕'} on:click={() => setImageLayout('wrap-left')}><WrapText />图左文右</button>
      <button class="layout-button wrap-button" class:active={imageLayout === 'wrap-right'} aria-label="图片靠右，文字在左侧环绕" use:tooltip={'图片靠右，文字在左侧环绕'} on:click={() => setImageLayout('wrap-right')}><WrapText />图右文左</button>
    </div>
  {:else}
    <select
      class="font-select"
      aria-label="字体"
      value={currentFont}
      on:mousedown={rememberTextSelection}
      on:change={(event) => applyFont(event.currentTarget.value)}
    >
      <option value="">默认字体</option>
      {#each fontChoices as font}
        <option value={font} style={`font-family:${font}`}>{font}</option>
      {/each}
    </select>

    <select
      class="size-select"
      aria-label="字号"
      value={currentSize}
      on:mousedown={rememberTextSelection}
      on:change={(event) => applyFontSize(event.currentTarget.value)}
    >
      {#each sizes as size}
        <option value={size}>{size}</option>
      {/each}
    </select>

    <span class="toolbar-divider"></span>
    <button class:active={editor?.isActive('bold')} aria-label="粗体 Ctrl+B" use:tooltip={'粗体 · Ctrl+B'} on:click={() => command(() => editor?.chain().focus().toggleBold().run())}><Bold /></button>
    <button class:active={editor?.isActive('italic')} aria-label="斜体 Ctrl+I" use:tooltip={'斜体 · Ctrl+I'} on:click={() => command(() => editor?.chain().focus().toggleItalic().run())}><Italic /></button>
    <button class:active={editor?.isActive('underline')} aria-label="下划线 Ctrl+U" use:tooltip={'下划线 · Ctrl+U'} on:click={() => command(() => editor?.chain().focus().toggleUnderline().run())}><Underline /></button>
    <button class:active={editor?.isActive('strike')} aria-label="删除线" use:tooltip={'删除线'} on:click={() => command(() => editor?.chain().focus().toggleStrike().run())}><Strikethrough /></button>

    <span class="toolbar-divider"></span>
    <button class:active={editor?.isActive('bulletList')} aria-label="项目符号" use:tooltip={'项目符号'} on:click={() => command(() => editor?.chain().focus().toggleBulletList().run())}><List /></button>
    <button class:active={editor?.isActive('orderedList')} aria-label="编号列表" use:tooltip={'编号列表'} on:click={() => command(() => editor?.chain().focus().toggleOrderedList().run())}><ListOrdered /></button>
    <button class:active={editor?.isActive('taskList')} aria-label="待办清单" use:tooltip={'待办清单'} on:click={() => command(() => editor?.chain().focus().toggleTaskList().run())}><CheckSquare /></button>

    <span class="toolbar-divider"></span>
    <button class:active={editor?.isActive({ textAlign: 'left' })} aria-label="左对齐" use:tooltip={'左对齐'} on:click={() => command(() => editor?.chain().focus().setTextAlign('left').run())}><AlignLeft /></button>
    <button class:active={editor?.isActive({ textAlign: 'center' })} aria-label="居中" use:tooltip={'居中'} on:click={() => command(() => editor?.chain().focus().setTextAlign('center').run())}><AlignCenter /></button>
    <button class:active={editor?.isActive({ textAlign: 'right' })} aria-label="右对齐" use:tooltip={'右对齐'} on:click={() => command(() => editor?.chain().focus().setTextAlign('right').run())}><AlignRight /></button>

    <span class="toolbar-divider"></span>
    <button aria-label="插入图片" use:tooltip={'插入图片'} on:click={onPickImage}><ImagePlus /></button>
    <button aria-label="插入 3×3 表格" use:tooltip={'插入 3×3 表格'} on:click={() => command(() => editor?.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run())}><Table2 /></button>

    {#if inTable}
      <button aria-label="添加一行" use:tooltip={'添加一行'} on:click={() => command(() => editor?.chain().focus().addRowAfter().run())}><Rows3 /><Plus class="corner-icon" /></button>
      <button aria-label="添加一列" use:tooltip={'添加一列'} on:click={() => command(() => editor?.chain().focus().addColumnAfter().run())}><Columns3 /><Plus class="corner-icon" /></button>
      <button aria-label="删除当前行" use:tooltip={'删除当前行'} on:click={() => command(() => editor?.chain().focus().deleteRow().run())}><Rows3 /><Minus class="corner-icon" /></button>
      <button aria-label="删除表格" use:tooltip={'删除表格'} on:click={() => command(() => editor?.chain().focus().deleteTable().run())}><Trash2 /></button>
    {/if}
  {/if}
</div>
