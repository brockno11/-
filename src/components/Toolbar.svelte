<script lang="ts">
  import type { Editor } from '@tiptap/core'
  import { NodeSelection } from '@tiptap/pm/state'
  import {
    Bold, Italic, Underline, Strikethrough, List, ListOrdered, CircleCheckBig,
    AlignLeft, AlignCenter, AlignRight, ImagePlus, Table2, Rows3, Columns3,
    Trash2, Minus, Plus, Image as ImageIcon, RotateCcw, WrapText, MoreHorizontal,
  } from 'lucide-svelte'
  import { normalizeImageLayout, type ImageLayout } from '../lib/editor'
  import { popover } from '../lib/menu'
  import { tooltip } from '../lib/tooltip'

  export let editor: Editor | undefined
  export let fonts: string[] = []
  export let version = 0
  export let defaultFontSize = 16
  export let onPickImage: () => void
  export let menuOpen = false
  export let onMenuOpenChange: (open: boolean) => void = () => undefined

  const sizes = [9, 10, 11, 12, 14, 16, 18, 20, 24, 28, 32, 36, 48, 64, 72]
  const preferredFonts = [
    'Microsoft YaHei UI', 'Microsoft YaHei', 'DengXian',
    'SimSun', 'SimHei', 'KaiTi', 'Segoe UI Variable', 'Arial', 'Consolas',
  ]
  let rememberedSelection: { from: number; to: number } | null = null
  let moreButton: HTMLButtonElement
  $: currentFont = version >= 0
    ? editor?.getAttributes('textStyle').fontFamily || ''
    : ''
  $: currentSize = version >= 0
    ? Number.parseInt(editor?.getAttributes('textStyle').fontSize || String(defaultFontSize), 10)
    : defaultFontSize
  $: blockStyle = version >= 0
    ? editor?.isActive('heading', { level: 1 }) ? 'h1'
      : editor?.isActive('heading', { level: 2 }) ? 'h2'
        : editor?.isActive('heading', { level: 3 }) ? 'h3'
          : editor?.isActive('blockquote') ? 'blockquote'
            : editor?.isActive('codeBlock') ? 'codeBlock'
              : 'paragraph'
    : 'paragraph'
  $: inTable = version >= 0 && (editor?.isActive('table') ?? false)
  $: imageSelected = version >= 0 && editor?.state.selection instanceof NodeSelection
    && editor.state.selection.node.type.name === 'image'
  $: imageAttributes = version >= 0 ? (editor?.getAttributes('image') ?? {}) : {}
  $: imageLayout = normalizeImageLayout(imageAttributes.layout)
  $: imageWidth = Number(imageAttributes.width) || 0
  $: if (imageSelected && menuOpen) onMenuOpenChange(false)
  $: fontChoices = Array.from(new Set([
    ...(currentFont ? [currentFont] : []),
    ...preferredFonts.filter((font) => fonts.includes(font)),
  ]))

  function command(action: () => boolean | void) {
    action()
    editor?.commands.focus()
  }

  function toggleMore() {
    rememberTextSelection()
    onMenuOpenChange(!menuOpen)
  }

  function moreCommand(action: () => boolean | void) {
    onMenuOpenChange(false)
    action()
    editor?.commands.focus()
    rememberedSelection = null
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

  function applyBlockStyle(style: string) {
    if (!editor) return
    let chain = editor.chain().focus()
    if (rememberedSelection) chain = chain.setTextSelection(rememberedSelection)
    const inBlockquote = editor.isActive('blockquote')
    const inCodeBlock = editor.isActive('codeBlock')
    if (inBlockquote && style !== 'blockquote') chain = chain.toggleBlockquote()
    if (inCodeBlock && style !== 'codeBlock') chain = chain.setParagraph()

    if (style === 'h1') chain.setHeading({ level: 1 })
    else if (style === 'h2') chain.setHeading({ level: 2 })
    else if (style === 'h3') chain.setHeading({ level: 3 })
    else if (style === 'blockquote' && !inBlockquote) chain.toggleBlockquote()
    else if (style === 'codeBlock' && !inCodeBlock) chain.toggleCodeBlock()
    else if (style === 'paragraph') chain.setParagraph()
    chain.run()
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
    <button class:active={editor?.isActive('bold')} aria-label="粗体 Ctrl+B" use:tooltip={'粗体 · Ctrl+B'} on:click={() => command(() => editor?.chain().focus().toggleBold().run())}><Bold /></button>
    <button class:active={editor?.isActive('italic')} aria-label="斜体 Ctrl+I" use:tooltip={'斜体 · Ctrl+I'} on:click={() => command(() => editor?.chain().focus().toggleItalic().run())}><Italic /></button>
    <button class:active={editor?.isActive('taskList')} aria-label="圆圈待办" use:tooltip={'圆圈待办 · 点击圆圈即可完成'} on:click={() => command(() => editor?.chain().focus().toggleTaskList().run())}><CircleCheckBig /></button>
    <button class:active={editor?.isActive('bulletList')} aria-label="项目符号" use:tooltip={'项目符号'} on:click={() => command(() => editor?.chain().focus().toggleBulletList().run())}><List /></button>
    <div class="format-more-anchor" data-transient-root>
      <button bind:this={moreButton} class:active={menuOpen} aria-label="更多格式" aria-haspopup="dialog" aria-expanded={menuOpen} use:tooltip={'更多格式'} on:mousedown|preventDefault={rememberTextSelection} on:click={toggleMore}><MoreHorizontal /></button>
      {#if menuOpen}
        <div class="format-more-menu" role="dialog" aria-label="更多格式" use:popover={{ onClose: () => onMenuOpenChange(false), returnFocus: moreButton }}>
          <section class="advanced-format-controls">
            <small>段落与字体</small>
            <div>
              <select class="block-select" aria-label="段落样式" value={blockStyle} on:mousedown={rememberTextSelection} on:change={(event) => applyBlockStyle(event.currentTarget.value)}><option value="paragraph">正文</option><option value="h1">一级标题</option><option value="h2">二级标题</option><option value="h3">三级标题</option><option value="blockquote">引用</option><option value="codeBlock">代码块</option></select>
              <select class="font-select" aria-label="字体" value={currentFont} on:mousedown={rememberTextSelection} on:change={(event) => applyFont(event.currentTarget.value)}><option value="">默认字体</option>{#each fontChoices as font}<option value={font} style={`font-family:${font}`}>{font}</option>{/each}</select>
              <select class="size-select" aria-label="字号" value={currentSize} on:mousedown={rememberTextSelection} on:change={(event) => applyFontSize(event.currentTarget.value)}>{#each sizes as size}<option value={size}>{size}</option>{/each}</select>
            </div>
          </section>
          <section>
            <small>文字</small>
            <div>
              <button class:active={editor?.isActive('underline')} aria-label="下划线" on:mousedown|preventDefault on:click={() => moreCommand(() => editor?.chain().focus().toggleUnderline().run())}><Underline /><span>下划线</span></button>
              <button class:active={editor?.isActive('strike')} aria-label="删除线" on:mousedown|preventDefault on:click={() => moreCommand(() => editor?.chain().focus().toggleStrike().run())}><Strikethrough /><span>删除线</span></button>
            </div>
          </section>
          <section>
            <small>列表</small>
            <div>
              <button class:active={editor?.isActive('orderedList')} on:mousedown|preventDefault on:click={() => moreCommand(() => editor?.chain().focus().toggleOrderedList().run())}><ListOrdered /><span>编号列表</span></button>
            </div>
          </section>
          <section>
            <small>对齐</small>
            <div>
              <button class:active={editor?.isActive({ textAlign: 'left' })} on:mousedown|preventDefault on:click={() => moreCommand(() => editor?.chain().focus().setTextAlign('left').run())}><AlignLeft /><span>左对齐</span></button>
              <button class:active={editor?.isActive({ textAlign: 'center' })} on:mousedown|preventDefault on:click={() => moreCommand(() => editor?.chain().focus().setTextAlign('center').run())}><AlignCenter /><span>居中</span></button>
              <button class:active={editor?.isActive({ textAlign: 'right' })} on:mousedown|preventDefault on:click={() => moreCommand(() => editor?.chain().focus().setTextAlign('right').run())}><AlignRight /><span>右对齐</span></button>
            </div>
          </section>
          <section>
            <small>{inTable ? '表格操作' : '插入'}</small>
            <div class="table-menu-grid">
              {#if !inTable}
                <button on:mousedown|preventDefault on:click={() => { onPickImage(); onMenuOpenChange(false) }}><ImagePlus /><span>图片</span></button>
                <button on:mousedown|preventDefault on:click={() => moreCommand(() => editor?.chain().focus().insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run())}><Table2 /><span>3×3 表格</span></button>
              {:else}
                <button on:mousedown|preventDefault on:click={() => moreCommand(() => editor?.chain().focus().addRowAfter().run())}><Rows3 /><span>增行</span><Plus class="corner-icon" /></button>
                <button on:mousedown|preventDefault on:click={() => moreCommand(() => editor?.chain().focus().addColumnAfter().run())}><Columns3 /><span>增列</span><Plus class="corner-icon" /></button>
                <button on:mousedown|preventDefault on:click={() => moreCommand(() => editor?.chain().focus().deleteRow().run())}><Rows3 /><span>删行</span><Minus class="corner-icon" /></button>
                <button class="danger" on:mousedown|preventDefault on:click={() => moreCommand(() => editor?.chain().focus().deleteTable().run())}><Trash2 /><span>删表格</span></button>
              {/if}
            </div>
          </section>
        </div>
      {/if}
    </div>
  {/if}
</div>
