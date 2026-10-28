import { afterEach, describe, expect, it } from 'vitest'
import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import TaskList from '@tiptap/extension-task-list'
import { FontSize, TextStyle } from '@tiptap/extension-text-style'
import { SizedTaskItem } from './task-item'

let editor: Editor | undefined
afterEach(() => { editor?.destroy(); editor = undefined; document.body.replaceChildren() })

describe('sized task node views', () => {
  it('uses the first task line size and updates nested and empty tasks together', async () => {
    const host = document.createElement('div'); document.body.append(host)
    editor = new Editor({ element: host, extensions: [StarterKit, TextStyle, FontSize, TaskList, SizedTaskItem.configure({ nested: true })],
      editorProps: { attributes: { style: 'font-size:16px;--task-marker-setting:0' } },
      content: '<ul data-type="taskList"><li data-type="taskItem"><p><span style="font-size:24px">大字号</span></p><ul data-type="taskList"><li data-type="taskItem"><p>子任务</p></li></ul></li><li data-type="taskItem"><p></p></li></ul>' })
    await Promise.resolve()
    const items = [...host.querySelectorAll<HTMLElement>('li[data-checked]')]
    expect(items).toHaveLength(3)
    expect(items[0].style.getPropertyValue('--task-size')).toBe('27px')
    expect(items[1].style.getPropertyValue('--task-size')).toBe('18px')
    expect(items[2].style.getPropertyValue('--task-size')).toBe('18px')
    editor.view.dom.style.setProperty('--task-marker-setting', '36')
    editor.view.dom.dispatchEvent(new Event('qingjian:task-style'))
    await Promise.resolve()
    expect(items.map(item => item.style.getPropertyValue('--task-size'))).toEqual(['36px', '36px', '36px'])
  })

  it('opens existing task documents and retains order and checked state', () => {
    const host = document.createElement('div'); document.body.append(host)
    editor = new Editor({ element: host, extensions: [StarterKit, TaskList, SizedTaskItem],
      content: '<ul data-type="taskList"><li data-type="taskItem" data-checked="false"><p>第一项</p></li><li data-type="taskItem" data-checked="true"><p>第二项</p></li></ul>' })
    const inputs = host.querySelectorAll<HTMLInputElement>('input[type=checkbox]')
    expect(inputs).toHaveLength(2)
    inputs[0].click()
    expect(editor.state.doc.firstChild?.firstChild?.attrs.checked).toBe(true)
    expect(editor.state.doc.firstChild?.child(1).textContent).toBe('第二项')
  })
})
