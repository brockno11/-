import { describe, expect, it } from 'vitest'
import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import { TableKit } from '@tiptap/extension-table'
import { TextSelection } from '@tiptap/pm/state'
import {
  parsePlainTextGrid, pastePlainTextGrid, shouldUsePlainTextGrid,
} from './table-paste'

function tableEditor(rows = 2, columns = 1) {
  const editor = new Editor({
    extensions: [StarterKit, TableKit],
    content: {
      type: 'doc',
      content: [{
        type: 'table',
        content: Array.from({ length: rows }, (_, row) => ({
          type: 'tableRow',
          content: Array.from({ length: columns }, () => ({
            type: row === 0 ? 'tableHeader' : 'tableCell',
            content: [{ type: 'paragraph' }],
          })),
        })),
      }],
    },
  })
  let firstCell = -1
  editor.state.doc.descendants((node, position) => {
    if (firstCell < 0 && ['tableHeader', 'tableCell'].includes(node.type.name)) {
      firstCell = position
    }
  })
  editor.view.dispatch(editor.state.tr.setSelection(
    TextSelection.create(editor.state.doc, firstCell + 2),
  ))
  return editor
}

function tableValues(editor: Editor) {
  const values: string[][] = []
  editor.state.doc.descendants((node) => {
    if (node.type.name === 'tableRow') {
      values.push(Array.from({ length: node.childCount }, (_, index) =>
        node.child(index).textContent,
      ))
      return false
    }
    return true
  })
  return values
}

describe('plain text table paste', () => {
  it('parses Notepad lines as one row per line without a trailing blank row', () => {
    expect(parsePlainTextGrid('完成设计\r\n联调接口\r\n发布版本\r\n')).toEqual([
      ['完成设计'],
      ['联调接口'],
      ['发布版本'],
    ])
  })

  it('parses tabs as Excel-style columns and preserves empty cells', () => {
    expect(parsePlainTextGrid('任务\t负责人\n测试\t')).toEqual([
      ['任务', '负责人'],
      ['测试', ''],
    ])
  })

  it('keeps ordinary single-line paste on the normal editor path', () => {
    expect(parsePlainTextGrid('只有一行')).toBeUndefined()
  })

  it('uses line distribution for copied note paragraphs but preserves real HTML tables', () => {
    expect(shouldUsePlainTextGrid('第一行\n第二行', '<p>第一行</p><p>第二行</p>')).toBe(true)
    expect(shouldUsePlainTextGrid('第一行\n第二行', '<table><tr><td>第一行</td></tr></table>')).toBe(false)
  })

  it('fills downward and grows the table for five pasted lines', () => {
    const editor = tableEditor(2, 1)
    expect(pastePlainTextGrid(editor.view, '一\n二\n三\n四\n五')).toBe(true)
    expect(tableValues(editor)).toEqual([
      ['一'], ['二'], ['三'], ['四'], ['五'],
    ])
    editor.destroy()
  })

  it('fills a rectangular tab-separated grid and grows columns', () => {
    const editor = tableEditor(2, 1)
    expect(pastePlainTextGrid(editor.view, '任务\t状态\n设计\t完成')).toBe(true)
    expect(tableValues(editor)).toEqual([
      ['任务', '状态'],
      ['设计', '完成'],
    ])
    editor.destroy()
  })
})
