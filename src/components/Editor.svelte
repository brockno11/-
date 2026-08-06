<script lang="ts">
  import { onDestroy, onMount } from 'svelte'
  import { Editor } from '@tiptap/core'
  import { NodeSelection } from '@tiptap/pm/state'
  import {
    AlignCenter, AlignLeft, AlignRight, RotateCcw, WrapText,
  } from 'lucide-svelte'
  import StarterKit from '@tiptap/starter-kit'
  import { FontSize, TextStyle } from '@tiptap/extension-text-style'
  import FontFamily from '@tiptap/extension-font-family'
  import TextAlign from '@tiptap/extension-text-align'
  import TaskList from '@tiptap/extension-task-list'
  import TaskItem from '@tiptap/extension-task-item'
  import Placeholder from '@tiptap/extension-placeholder'
  import { TableKit } from '@tiptap/extension-table'
  import DOMPurify from 'dompurify'
  import {
    AttachmentImage, fileToDataUrl, MarkdownTaskShortcut, normalizeImageLayout, type ImageLayout,
  } from '../lib/editor'
  import { MixedContentSelection } from '../lib/mixed-selection'
  import { pastePlainTextGrid, shouldUsePlainTextGrid } from '../lib/table-paste'
  import { tooltip } from '../lib/tooltip'
  import { backend } from '../lib/backend'

  export let noteId: string
  export let content: Record<string, unknown>
  export let defaultFontFamily = 'Microsoft YaHei UI'
  export let defaultFontSize = 16
  export let onChange: (content: Record<string, unknown>, plainText: string) => void
  export let onReady: (editor: Editor) => void
  export let onSelectionChange: () => void
  export let onError: (message: string) => void

  let host: HTMLDivElement
  let editor: Editor | undefined
  let loadedNoteId = ''
  let editorScroll: HTMLElement | null = null
  let positionFrame = 0
  let imageSelected = false
  let imageToolbarLeft = 0
  let imageToolbarTop = 0
  let imageToolbarBelow = false
  let imageWidth = 0
  let imageLayout: ImageLayout = 'block-center'

  const sanitizeOptions = {
    ALLOWED_TAGS: [
      'p', 'br', 'strong', 'b', 'em', 'i', 'u', 's', 'strike', 'span',
      'h1', 'h2', 'h3', 'h4', 'ul', 'ol', 'li', 'blockquote', 'pre', 'code',
      'table', 'thead', 'tbody', 'tfoot', 'tr', 'th', 'td', 'img', 'a',
    ],
    ALLOWED_ATTR: [
      'href', 'title', 'target', 'colspan', 'rowspan', 'style', 'src', 'alt',
      'width', 'height', 'data-asset-path', 'data-attachment-id', 'data-layout',
    ],
    ALLOW_DATA_ATTR: false,
  }

  function sanitizeClipboardHtml(html: string): string {
    const clean = DOMPurify.sanitize(html, sanitizeOptions)
    const document = new DOMParser().parseFromString(clean, 'text/html')
    const allowedStyles = new Set([
      'font-family', 'font-size', 'font-style', 'font-weight',
      'text-align', 'text-decoration', 'color', 'background-color',
    ])
    document.querySelectorAll<HTMLElement>('[style]').forEach((element) => {
      const safe: string[] = []
      for (const property of Array.from(element.style)) {
        if (!allowedStyles.has(property)) continue
        const value = element.style.getPropertyValue(property)
        if (/url\s*\(|expression\s*\(|javascript:/i.test(value)) continue
        safe.push(`${property}:${value}`)
      }
      if (safe.length) element.setAttribute('style', safe.join(';'))
      else element.removeAttribute('style')
    })
    document.querySelectorAll<HTMLImageElement>('img').forEach((image) => {
      if (!/^(asset:|http:\/\/asset\.localhost)/i.test(image.src)) image.remove()
    })
    return document.body.innerHTML
  }

  function selectImageBeforeCursor() {
    if (!editor) return
    const cursor = editor.state.selection.to
    let candidate = -1
    editor.state.doc.descendants((node, position) => {
      if (position >= cursor) return false
      if (node.type.name === 'image') candidate = position
      return true
    })
    if (candidate >= 0) editor.commands.setNodeSelection(candidate)
  }

  function selectedImageElement() {
    if (!editor || !(editor.state.selection instanceof NodeSelection)) return null
    const dom = editor.view.nodeDOM(editor.state.selection.from)
    return dom instanceof HTMLImageElement
      ? dom
      : dom instanceof HTMLElement
        ? dom.querySelector('img')
        : null
  }

  function editorContentWidth() {
    if (!editor) return 0
    const dom = editor.view.dom as HTMLElement
    const style = getComputedStyle(dom)
    return dom.clientWidth
      - Number.parseFloat(style.paddingLeft)
      - Number.parseFloat(style.paddingRight)
  }

  function updateImageSize(width: number, ratio: number) {
    if (!editor || !(editor.state.selection instanceof NodeSelection)) return
    const safeWidth = Math.max(60, Math.min(Math.round(width), editorContentWidth()))
    const height = Math.max(40, Math.round(safeWidth / Math.max(ratio, 0.01)))
    editor.chain().focus().updateAttributes('image', { width: safeWidth, height }).run()
  }

  function setImageWidth(percent: number) {
    const image = selectedImageElement()
    const attributes = editor?.getAttributes('image') ?? {}
    const storedWidth = Number(attributes.width)
    const storedHeight = Number(attributes.height)
    const ratio = storedWidth > 0 && storedHeight > 0
      ? storedWidth / storedHeight
      : image?.naturalWidth && image.naturalHeight
        ? image.naturalWidth / image.naturalHeight
        : 1
    updateImageSize(editorContentWidth() * percent, ratio)
  }

  function restoreImageSize() {
    const image = selectedImageElement()
    if (!image?.naturalWidth || !image.naturalHeight) return
    updateImageSize(
      Math.min(image.naturalWidth, editorContentWidth()),
      image.naturalWidth / image.naturalHeight,
    )
  }

  function setImageLayout(layout: ImageLayout) {
    if (!editor || !(editor.state.selection instanceof NodeSelection)) return
    editor.chain().focus().updateAttributes('image', { layout }).run()
  }

  function positionImageToolbar() {
    cancelAnimationFrame(positionFrame)
    positionFrame = requestAnimationFrame(() => {
      const image = selectedImageElement()
      const container = image?.closest('[data-image-node-view]')
      if (!image || !(container instanceof HTMLElement)) {
        imageSelected = false
        return
      }
      const rect = container.getBoundingClientRect()
      imageToolbarBelow = rect.top < 146
      imageToolbarLeft = Math.max(138, Math.min(
        window.innerWidth - 138,
        rect.left + rect.width / 2,
      ))
      imageToolbarTop = imageToolbarBelow ? rect.bottom + 9 : rect.top - 9
    })
  }

  function updateImageTools(instance: Editor) {
    const selection = instance.state.selection
    imageSelected = selection instanceof NodeSelection && selection.node.type.name === 'image'
    if (selection instanceof NodeSelection && selection.node.type.name === 'image') {
      const attributes = selection.node.attrs
      imageWidth = Number(attributes.width) || 0
      imageLayout = normalizeImageLayout(attributes.layout)
      positionImageToolbar()
    }
  }

  async function insertImageBlob(blob: Blob, selectAfterInsert = true) {
    if (!editor) return
    try {
      const dataUrl = await fileToDataUrl(blob)
      const attachment = await backend.saveAttachment(dataUrl, blob.type || 'image/png')
      editor
        .chain()
        .focus()
        .setImage({
          src: backend.assetUrl(attachment.filePath),
          alt: '便签图片',
          assetPath: attachment.assetPath,
          attachmentId: attachment.id,
          width: attachment.width || undefined,
          layout: 'block-center',
        } as never)
        .run()
      if (selectAfterInsert) selectImageBeforeCursor()
    } catch (error) {
      onError(error instanceof Error ? error.message : '图片插入失败')
    }
  }

  export async function insertFiles(files: FileList | File[]) {
    const images = Array.from(files).filter((file) => file.type.startsWith('image/'))
    for (const [index, file] of images.entries()) {
      await insertImageBlob(file, index === images.length - 1)
    }
  }

  export function focus() {
    editor?.commands.focus()
  }

  onMount(() => {
    editor = new Editor({
      element: host,
      content,
      extensions: [
        StarterKit.configure({
          link: {
            openOnClick: false,
            autolink: true,
            HTMLAttributes: { rel: 'noopener noreferrer' },
          },
        }),
        TextStyle,
        FontFamily,
        FontSize,
        TextAlign.configure({ types: ['heading', 'paragraph'] }),
        TaskList,
        TaskItem.configure({ nested: true }),
        MarkdownTaskShortcut,
        TableKit.configure({
          table: {
            resizable: true,
            lastColumnResizable: true,
            allowTableNodeSelection: true,
          },
        }),
        AttachmentImage.configure({
          inline: false,
          allowBase64: false,
        }),
        MixedContentSelection,
        Placeholder.configure({
          placeholder: '开始记录… 输入 #、-、[ ] 或 ``` 后按空格快速排版',
        }),
      ],
      editorProps: {
        attributes: {
          class: 'note-prosemirror',
          spellcheck: 'true',
          style: `font-family:${defaultFontFamily};font-size:${defaultFontSize}px`,
        },
        handleClickOn: (view, _position, node, nodePosition, _event) => {
          if (node.type.name !== 'image') return false
          view.dispatch(
            view.state.tr.setSelection(NodeSelection.create(view.state.doc, nodePosition)),
          )
          view.focus()
          return true
        },
        handlePaste: (view, event) => {
          const clipboard = event.clipboardData
          if (!clipboard) return false
          const image = Array.from(clipboard.items).find(
            (item) => item.kind === 'file' && item.type.startsWith('image/'),
          )
          if (image) {
            const blob = image.getAsFile()
            if (blob) {
              event.preventDefault()
              void insertImageBlob(blob)
              return true
            }
          }
          const html = clipboard.getData('text/html')
          const plainText = clipboard.getData('text/plain')
          if (plainText && shouldUsePlainTextGrid(plainText, html)) {
            try {
              if (pastePlainTextGrid(view, plainText)) {
                event.preventDefault()
                return true
              }
            } catch (error) {
              event.preventDefault()
              onError(error instanceof Error ? error.message : '表格粘贴失败')
              return true
            }
          }
          if (html) {
            event.preventDefault()
            const clean = sanitizeClipboardHtml(html)
            editor?.chain().focus().insertContent(clean).run()
            return true
          }
          return false
        },
        handleDrop: (_view, event) => {
          const files = Array.from(event.dataTransfer?.files ?? []).filter((file) =>
            file.type.startsWith('image/'),
          )
          if (!files.length) return false
          event.preventDefault()
          void insertFiles(files)
          return true
        },
      },
      onCreate: ({ editor: instance }) => {
        loadedNoteId = noteId
        onReady(instance)
        updateImageTools(instance)
      },
      onUpdate: ({ editor: instance }) => {
        onChange(instance.getJSON() as Record<string, unknown>, instance.getText())
      },
      onSelectionUpdate: ({ editor: instance }) => {
        updateImageTools(instance)
        onSelectionChange()
      },
      onTransaction: ({ editor: instance }) => {
        updateImageTools(instance)
        onSelectionChange()
      },
    })
    editorScroll = host.closest('.editor-scroll')
    editorScroll?.addEventListener('scroll', positionImageToolbar, { passive: true })
    window.addEventListener('resize', positionImageToolbar)
  })

  $: if (editor && noteId && noteId !== loadedNoteId) {
    editor.commands.setContent(content, { emitUpdate: false })
    loadedNoteId = noteId
    queueMicrotask(() => editor?.commands.focus('end'))
  }

  $: if (editor) {
    editor.setOptions({
      editorProps: {
        ...editor.options.editorProps,
        attributes: {
          ...editor.options.editorProps.attributes,
          class: 'note-prosemirror',
          spellcheck: 'true',
          style: `font-family:${defaultFontFamily};font-size:${defaultFontSize}px`,
        },
      },
    })
  }

  onDestroy(() => {
    cancelAnimationFrame(positionFrame)
    editorScroll?.removeEventListener('scroll', positionImageToolbar)
    window.removeEventListener('resize', positionImageToolbar)
    editor?.destroy()
  })
</script>

<div class="editor-stage">
  <div class="editor-host" bind:this={host} aria-label="便签正文"></div>
  {#if imageSelected}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      class="image-context-toolbar"
      class:below={imageToolbarBelow}
      style={`left:${imageToolbarLeft}px;top:${imageToolbarTop}px`}
      aria-label="图片快捷工具"
      on:mousedown|stopPropagation
    >
      <span class="context-size">{imageWidth ? `${Math.round(imageWidth)}px` : '原始'}</span>
      <select
        aria-label="图片宽度"
        value=""
        on:change={(event) => {
          const value = Number(event.currentTarget.value)
          if (value > 0) setImageWidth(value)
          event.currentTarget.value = ''
        }}
      >
        <option value="" disabled>尺寸</option>
        <option value="0.25">25%</option>
        <option value="0.5">50%</option>
        <option value="0.75">75%</option>
        <option value="1">100%</option>
      </select>
      <button
        class:active={imageLayout === 'block-left'}
        aria-label="图片左对齐"
        use:tooltip={'图片左对齐'}
        on:click={() => setImageLayout('block-left')}
      ><AlignLeft /></button>
      <button
        class:active={imageLayout === 'block-center'}
        aria-label="图片居中"
        use:tooltip={'图片居中'}
        on:click={() => setImageLayout('block-center')}
      ><AlignCenter /></button>
      <button
        class:active={imageLayout === 'block-right'}
        aria-label="图片右对齐"
        use:tooltip={'图片右对齐'}
        on:click={() => setImageLayout('block-right')}
      ><AlignRight /></button>
      <button
        class:active={imageLayout === 'wrap-left'}
        aria-label="左图右文"
        use:tooltip={'左图右文'}
        on:click={() => setImageLayout('wrap-left')}
      ><WrapText /></button>
      <button
        class="flip-icon"
        class:active={imageLayout === 'wrap-right'}
        aria-label="右图左文"
        use:tooltip={'右图左文'}
        on:click={() => setImageLayout('wrap-right')}
      ><WrapText /></button>
      <button
        aria-label="恢复图片原始尺寸"
        use:tooltip={'恢复适合正文的原始尺寸'}
        on:click={restoreImageSize}
      ><RotateCcw /></button>
    </div>
  {/if}
</div>
