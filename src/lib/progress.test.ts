import { afterEach, describe, expect, it } from 'vitest'
import { Editor, type JSONContent } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import TaskList from '@tiptap/extension-task-list'
import { closeHistory } from '@tiptap/pm/history'
import { SizedTaskItem } from './task-item'
import { ProgressDocument } from './progress-extension'
import { progressPlainText, progressSummary } from './progress'
import { serializeMarkdown, type MarkdownNode } from './markdown'
import { richHtml } from './rich-html'

const paragraph = (text: string) => ({ type: 'paragraph', content: text ? [{ type: 'text', text }] : [] })
const step = (text: string, state = 'todo', checked = false, children: JSONContent[] = []) => ({
  type: 'taskItem', attrs: { checked, progressState: state }, content: [paragraph(text), ...children],
})
const list = (...content: JSONContent[]) => ({ type: 'taskList', content })
const doc = (...content: JSONContent[]) => ({ type: 'doc', attrs: { progressEnabled: true }, content })
let editor: Editor | undefined
afterEach(() => { editor?.destroy(); editor = undefined; document.body.replaceChildren() })

describe('optional progress', () => {
  it('counts only nonempty top-level steps and allows parallel work', () => {
    const summary = progressSummary(doc(list(step('分析', 'doing'), step('数据', 'doing'), step('交付', 'blocked'),
      step('完成', 'todo', true, [list(step('不重复计数'))]), step('  '), step('', 'todo', false, [list(step('仅子任务'))]))))
    expect(summary).toEqual({ enabled: true, total: 4, done: 1, doing: 2, current: '分析', blocked: 1 })
    expect(progressSummary(doc(list(step('下一步')))).current).toBe('下一步')
    expect(progressSummary(doc(list(step('完成', 'todo', true)))).done).toBe(1)
    expect(progressSummary(doc()).total).toBe(0)
  })

  it('roundtrips old documents, four states, undo, checkbox restore and disabled progress', () => {
    const host = document.createElement('div'); document.body.append(host)
    editor = new Editor({ element: host, extensions: [StarterKit, TaskList, SizedTaskItem, ProgressDocument],
      content: { type: 'doc', content: [list(step('需求'))] } })
    expect(editor.getJSON().attrs?.progressEnabled).toBe(false)
    expect(host.querySelector('select')).toBeNull()
    editor.view.dispatch(editor.state.tr.setDocAttribute('progressEnabled', true))
    let select = host.querySelector('select')!
    expect(select.value).toBe('todo')
    select.value = 'doing'; select.dispatchEvent(new Event('change'))
    host.querySelector<HTMLInputElement>('input')!.click()
    expect(select.value).toBe('done')
    host.querySelector<HTMLInputElement>('input')!.click()
    expect(select.value).toBe('doing')
    editor.view.dispatch(closeHistory(editor.state.tr))
    select.value = 'blocked'; select.dispatchEvent(new Event('change'))
    select.dispatchEvent(new KeyboardEvent('keydown', { key: 'z', ctrlKey: true, bubbles: true, cancelable: true }))
    expect(select.value).toBe('doing')
    select.dispatchEvent(new KeyboardEvent('keydown', { key: 'z', ctrlKey: true, shiftKey: true, bubbles: true, cancelable: true }))
    expect(select.value).toBe('blocked')
    editor.view.dispatch(closeHistory(editor.state.tr))
    editor.view.dispatch(editor.state.tr.setDocAttribute('progressEnabled', false))
    expect(host.querySelector('select')).toBeNull()
    editor.view.dispatch(editor.state.tr.setDocAttribute('progressEnabled', true))
    select = host.querySelector('select')!
    expect(select.value).toBe('blocked')
    const saved = editor.getJSON()
    editor.commands.setContent(JSON.parse(JSON.stringify(saved)))
    expect(host.querySelector<HTMLSelectElement>('select')?.value).toBe('blocked')
    expect((saved as MarkdownNode).content?.[0].content?.[0].attrs?.progressState).toBe('blocked')
  })

  it('exports readable states without putting them in body text or ordinary exports', () => {
    const content = doc(list(step('分析', 'doing'), step('加数据', 'blocked'), step('交付', 'doing', true)))
    expect(serializeMarkdown(content)).toContain('- [ ] 【进行中】分析')
    expect(serializeMarkdown(content)).toContain('- [x] 【已完成】交付')
    expect(progressPlainText(content)).toContain('【受阻】加数据')
    const html = richHtml('<ul data-type="taskList"><li data-type="taskItem" data-checked="false" data-progress-state="blocked"><p>加数据</p></li></ul>',
      { fontFamily: 'sans-serif', fontSize: 16, lineHeight: 1.55, taskMarkerSize: 24 }, true)
    expect(html).toContain('【受阻】')
    expect(serializeMarkdown({ ...content, attrs: { progressEnabled: false } })).not.toContain('【')
    const host = document.createElement('div'); document.body.append(host)
    editor = new Editor({ element: host, extensions: [StarterKit, TaskList, SizedTaskItem, ProgressDocument], content })
    expect(editor.getText()).not.toContain('进行中')
    expect(editor.getHTML()).toContain('data-progress-state="doing"')
    expect(editor.getText()).toContain('分析')
  })

  it('starts a new step as pending when splitting a blocked step', () => {
    const host = document.createElement('div'); document.body.append(host)
    editor = new Editor({ element: host, extensions: [StarterKit, TaskList, SizedTaskItem, ProgressDocument], content: doc(list(step('受阻事项', 'blocked'))) })
    editor.commands.setTextSelection(3 + '受阻事项'.length)
    expect(editor.commands.splitListItem('taskItem')).toBe(true)
    const items = (editor.getJSON() as MarkdownNode).content?.[0].content
    expect(items).toHaveLength(2)
    expect(items?.[0].attrs?.progressState).toBe('blocked')
    expect(items?.[1].attrs?.progressState).toBe('todo')
    expect(items?.[1].attrs?.checked).toBe(false)
  })

  it('includes readable state in native rich clipboard and leaves ordinary HTML unchanged', () => {
    const host = document.createElement('div'); document.body.append(host)
    editor = new Editor({ element: host, extensions: [StarterKit, TaskList, SizedTaskItem, ProgressDocument], content: doc(list(step('步骤A', 'blocked'))) })
    const instance = editor
    instance.commands.selectAll()
    const copied = instance.view.serializeForClipboard(instance.state.selection.content()).dom.innerHTML
    expect(copied).toContain('【受阻】')
    expect(copied).toContain('data-progress-state="blocked"')
    instance.view.dispatch(instance.state.tr.setDocAttribute('progressEnabled', false))
    instance.commands.selectAll()
    expect(instance.view.serializeForClipboard(instance.state.selection.content()).dom.innerHTML).not.toContain('【受阻】')
  })
})
