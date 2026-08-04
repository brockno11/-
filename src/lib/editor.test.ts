import { describe, expect, it } from 'vitest'
import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { AttachmentImage, documentTitle, normalizeImageLayout } from './editor'

describe('documentTitle', () => {
  it('uses the first meaningful line', () => {
    expect(documentTitle('\n  完成首页设计  \n继续联调')).toBe('完成首页设计')
  })

  it('falls back for an empty note', () => {
    expect(documentTitle(' \n  ')).toBe('新便签')
  })

  it('limits generated titles to 80 characters', () => {
    expect(documentTitle('进'.repeat(100))).toHaveLength(80)
  })
})

describe('AttachmentImage', () => {
  it('keeps existing images in block layout by default', () => {
    const editor = new Editor({
      extensions: [StarterKit, AttachmentImage.configure({ resize: false })],
      content: {
        type: 'doc',
        content: [{ type: 'image', attrs: { src: 'asset://existing.png' } }],
      },
    })

    expect(editor.getJSON().content?.[0].attrs?.layout).toBe('block-center')
    expect(editor.getHTML()).toContain('data-layout="block-center"')
    editor.destroy()
  })

  it('persists text wrapping layout in JSON and HTML', () => {
    const editor = new Editor({
      extensions: [StarterKit, AttachmentImage.configure({ resize: false })],
      content: {
        type: 'doc',
        content: [{ type: 'image', attrs: { src: 'asset://wrapped.png' } }],
      },
    })

    editor.commands.setNodeSelection(0)
    editor.commands.updateAttributes('image', { layout: 'wrap-right', width: 240, height: 160 })

    const image = editor.getJSON().content?.[0]
    expect(image?.attrs).toMatchObject({ layout: 'wrap-right', width: 240, height: 160 })
    expect(editor.getHTML()).toContain('data-layout="wrap-right"')
    editor.destroy()
  })

  it('maps legacy layouts to the new five-layout model', () => {
    expect(normalizeImageLayout('block')).toBe('block-center')
    expect(normalizeImageLayout('float-left')).toBe('wrap-left')
    expect(normalizeImageLayout('float-right')).toBe('wrap-right')
    expect(normalizeImageLayout('block-left')).toBe('block-left')
  })
})
