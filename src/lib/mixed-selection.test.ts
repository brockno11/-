import { describe, expect, it } from 'vitest'
import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { TableKit } from '@tiptap/extension-table'
import { TextSelection } from '@tiptap/pm/state'
import { CellSelection } from '@tiptap/pm/tables'
import { AttachmentImage } from './editor'
import {
  expandMixedSelection, selectedObjectRanges, selectionObjectRanges,
} from './mixed-selection'

function mixedEditor() {
  return new Editor({
    extensions: [
      StarterKit,
      TableKit,
      AttachmentImage.configure({ allowBase64: false }),
    ],
    content: {
      type: 'doc',
      content: [
        { type: 'paragraph', content: [{ type: 'text', text: '开头' }] },
        { type: 'image', attrs: { src: 'asset://image.png' } },
        {
          type: 'table',
          content: [{
            type: 'tableRow',
            content: [
              {
                type: 'tableHeader',
                content: [{ type: 'paragraph', content: [{ type: 'text', text: '表头' }] }],
              },
              {
                type: 'tableHeader',
                content: [{ type: 'paragraph', content: [{ type: 'text', text: '进度' }] }],
              },
            ],
          }],
        },
        { type: 'paragraph', content: [{ type: 'text', text: '结尾' }] },
      ],
    },
  })
}

function nodePosition(editor: Editor, type: string) {
  let found = -1
  editor.state.doc.descendants((node, position) => {
    if (found < 0 && node.type.name === type) found = position
    return found < 0
  })
  return found
}

function nodePositions(editor: Editor, type: string) {
  const found: number[] = []
  editor.state.doc.descendants((node, position) => {
    if (node.type.name === type) found.push(position)
  })
  return found
}

describe('mixed content selection', () => {
  it('finds images and tables crossed by a text range', () => {
    const editor = mixedEditor()
    const image = nodePosition(editor, 'image')
    const table = nodePosition(editor, 'table')
    const tableNode = editor.state.doc.nodeAt(table)!

    expect(selectedObjectRanges(editor.state.doc, 1, table + tableNode.nodeSize + 1))
      .toEqual([
        { from: image, to: image + 1, type: 'image' },
        { from: table, to: table + tableNode.nodeSize, type: 'table' },
      ])
    editor.destroy()
  })

  it('expands a drag entering a table from outside to the whole table', () => {
    const editor = mixedEditor()
    const table = nodePosition(editor, 'table')
    const tableNode = editor.state.doc.nodeAt(table)!
    editor.view.dispatch(editor.state.tr.setSelection(
      TextSelection.create(editor.state.doc, 1, table + 4),
    ))

    expect(expandMixedSelection(editor.state, 1, table + 4)).toEqual({
      anchor: 1,
      head: table + tableNode.nodeSize,
    })
    editor.destroy()
  })

  it('keeps a selection wholly inside one table as a cell text selection', () => {
    const editor = mixedEditor()
    const table = nodePosition(editor, 'table')
    editor.view.dispatch(editor.state.tr.setSelection(
      TextSelection.create(editor.state.doc, table + 4, table + 6),
    ))

    expect(expandMixedSelection(editor.state, table + 4, table + 6)).toBeUndefined()
    editor.destroy()
  })

  it('does not decorate the whole table for a cell selection', () => {
    const editor = mixedEditor()
    const cells = nodePositions(editor, 'tableHeader')
    editor.view.dispatch(editor.state.tr.setSelection(
      CellSelection.create(editor.state.doc, cells[0], cells[0]),
    ))

    expect(selectionObjectRanges(editor.state)).toEqual([])
    editor.destroy()
  })
})
