import { ResizableNodeView } from '@tiptap/core'
import Image from '@tiptap/extension-image'
import { NodeSelection, TextSelection } from '@tiptap/pm/state'
import type { Node as ProseMirrorNode } from '@tiptap/pm/model'

export type ImageLayout =
  | 'block-left'
  | 'block-center'
  | 'block-right'
  | 'wrap-left'
  | 'wrap-right'

export function normalizeImageLayout(value: unknown): ImageLayout {
  if (value === 'block-left' || value === 'block-right') return value
  if (value === 'wrap-left' || value === 'float-left') return 'wrap-left'
  if (value === 'wrap-right' || value === 'float-right') return 'wrap-right'
  return 'block-center'
}

function syncImageElement(image: HTMLImageElement, node: ProseMirrorNode) {
  const attributes = node.attrs
  const stringAttributes = {
    src: attributes.src,
    alt: attributes.alt,
    title: attributes.title,
    'data-asset-path': attributes.assetPath,
    'data-attachment-id': attributes.attachmentId,
    'data-layout': normalizeImageLayout(attributes.layout),
  }
  Object.entries(stringAttributes).forEach(([name, value]) => {
    if (typeof value === 'string' && value) image.setAttribute(name, value)
    else image.removeAttribute(name)
  })

  const width = Number(attributes.width)
  const height = Number(attributes.height)
  image.style.width = width > 0 ? `${width}px` : ''
  image.style.height = height > 0 ? `${height}px` : ''
}

export const AttachmentImage = Image.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      assetPath: {
        default: null,
        parseHTML: (element) => element.getAttribute('data-asset-path'),
        renderHTML: (attributes) =>
          attributes.assetPath ? { 'data-asset-path': attributes.assetPath } : {},
      },
      attachmentId: {
        default: null,
        parseHTML: (element) => element.getAttribute('data-attachment-id'),
        renderHTML: (attributes) =>
          attributes.attachmentId
            ? { 'data-attachment-id': attributes.attachmentId }
            : {},
      },
      width: {
        default: null,
        parseHTML: (element) => element.getAttribute('width') || element.style.width,
        renderHTML: (attributes) =>
          attributes.width ? { width: attributes.width, style: `width:${attributes.width}px` } : {},
      },
      height: {
        default: null,
        parseHTML: (element) => element.getAttribute('height') || element.style.height,
        renderHTML: (attributes) =>
          attributes.height ? { height: attributes.height } : {},
      },
      layout: {
        default: 'block-center',
        parseHTML: (element) => normalizeImageLayout(element.getAttribute('data-layout')),
        renderHTML: (attributes) => ({
          'data-layout': normalizeImageLayout(attributes.layout),
        }),
      },
    }
  },

  addNodeView() {
    return ({ node, getPos, editor }) => {
      let currentNode = node
      const image = document.createElement('img')
      image.draggable = false
      syncImageElement(image, currentNode)

      const contentWidth = () => {
        const editorElement = editor.view.dom as HTMLElement
        const style = getComputedStyle(editorElement)
        return Math.max(
          60,
          editorElement.clientWidth
            - Number.parseFloat(style.paddingLeft)
            - Number.parseFloat(style.paddingRight),
        )
      }
      const clampSize = (width: number, height: number) => {
        const safeWidth = Math.max(60, Math.min(width, contentWidth()))
        const ratio = width > 0 && height > 0 ? width / height : 1
        return {
          width: Math.round(safeWidth),
          height: Math.max(40, Math.round(safeWidth / Math.max(ratio, 0.01))),
        }
      }

      const nodeView = new ResizableNodeView({
        element: image,
        editor,
        node,
        getPos,
        onResize: (width, height) => {
          const size = clampSize(width, height)
          image.style.width = `${size.width}px`
          image.style.height = `${size.height}px`
        },
        onCommit: (width, height) => {
          const position = getPos()
          if (position === undefined) return
          const size = clampSize(width, height)
          const attributes = { ...currentNode.attrs, ...size }
          const transaction = editor.state.tr
            .setNodeMarkup(position, undefined, attributes)
          transaction.setSelection(NodeSelection.create(transaction.doc, position))
          editor.view.dispatch(transaction)
        },
        onUpdate: (updatedNode) => {
          if (updatedNode.type !== currentNode.type) return false
          currentNode = updatedNode
          syncImageElement(image, updatedNode)
          return true
        },
        options: {
          directions: ['top-left', 'top-right', 'bottom-left', 'bottom-right'],
          min: { width: 60, height: 40 },
          preserveAspectRatio: true,
        },
      })

      nodeView.dom.dataset.imageNodeView = ''
      nodeView.dom.draggable = true
      nodeView.dom.setAttribute('aria-label', '便签图片，可拖动换位')

      const selectImage = (event: MouseEvent) => {
        if (event.button !== 0 || (event.target as Element)?.closest('[data-resize-handle]')) return
        const position = getPos()
        if (position === undefined) return
        const selection = event.shiftKey
          ? TextSelection.create(
            editor.state.doc,
            editor.state.selection.anchor,
            editor.state.selection.anchor <= position
              ? position + currentNode.nodeSize
              : position,
          )
          : NodeSelection.create(editor.state.doc, position)
        editor.view.dispatch(editor.state.tr.setSelection(selection))
      }
      nodeView.dom.addEventListener('mousedown', selectImage)

      const destroy = nodeView.destroy.bind(nodeView)
      nodeView.destroy = () => {
        nodeView.dom.removeEventListener('mousedown', selectImage)
        destroy()
      }
      return nodeView
    }
  },
})

export async function fileToDataUrl(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onload = () => resolve(String(reader.result))
    reader.onerror = () => reject(reader.error)
    reader.readAsDataURL(file)
  })
}

export function documentTitle(plainText: string): string {
  const firstLine = plainText
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean)
  return (firstLine || '新便签').slice(0, 80)
}
