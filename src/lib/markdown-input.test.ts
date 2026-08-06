import { describe, expect, it } from 'vitest'
import { Editor } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import TaskList from '@tiptap/extension-task-list'
import TaskItem from '@tiptap/extension-task-item'
import { MarkdownTaskShortcut } from './editor'

function type(editor: Editor, value: string) {
  for (const character of value) {
    const { from, to } = editor.view.state.selection
    const handled = editor.view.someProp('handleTextInput', (handler) =>
      handler(editor.view, from, to, character, () => editor.view.state.tr),
    )
    if (!handled) editor.view.dispatch(editor.view.state.tr.insertText(character, from, to))
  }
}

function createEditor() {
  return new Editor({
    extensions: [
      StarterKit,
      TaskList,
      TaskItem.configure({ nested: true }),
      MarkdownTaskShortcut,
    ],
    content: '<p></p>',
  })
}

describe('Markdown editing shortcuts', () => {
  it('turns a heading prefix into a semantic heading', () => {
    const editor = createEditor()
    type(editor, '## ')
    expect(editor.getJSON().content?.[0]).toMatchObject({ type: 'heading', attrs: { level: 2 } })
    editor.destroy()
  })

  it('turns a familiar Markdown checkbox into a task item', () => {
    const editor = createEditor()
    type(editor, '- [ ] ')
    expect(editor.getJSON().content?.[0]).toMatchObject({
      type: 'taskList',
      content: [{ type: 'taskItem', attrs: { checked: false } }],
    })
    editor.destroy()
  })

  it('turns fenced input into a code block', () => {
    const editor = createEditor()
    type(editor, '```ts ')
    expect(editor.getJSON().content?.[0]).toMatchObject({
      type: 'codeBlock', attrs: { language: 'ts' },
    })
    editor.destroy()
  })
})
