import { render, waitFor } from '@testing-library/svelte'
import { describe, expect, it, vi } from 'vitest'
import type { Editor } from '@tiptap/core'
import EditorView from './Editor.svelte'

const initial = { type: 'doc', attrs: { progressEnabled: true }, content: [
  { type: 'paragraph', content: [{ type: 'text', text: '原有内容' }] },
  { type: 'taskList', content: [{ type: 'taskItem', attrs: { checked: false, progressState: 'blocked' }, content: [
    { type: 'paragraph', content: [{ type: 'text', text: '等待数据' }] },
  ] }] },
] }

describe('editor initialization and live appearance', () => {
  it('retains the initial task document and native progress clipboard after ready', async () => {
    let editor: Editor | undefined
    const changed = vi.fn()
    const result = render(EditorView, { props: { noteId: 'first', content: initial,
      onReady: instance => { editor = instance }, onChange: changed, onSelectionChange: vi.fn(), onError: vi.fn() } })
    await waitFor(() => expect(editor).toBeDefined())
    expect(editor!.getText()).toContain('原有内容')
    expect(editor!.getJSON().attrs?.progressEnabled).toBe(true)
    expect(changed).not.toHaveBeenCalled()
    editor!.commands.selectAll()
    expect(editor!.view.serializeForClipboard(editor!.state.selection.content()).dom.innerHTML).toContain('【受阻】')
    result.unmount()
  })

  it('updates appearance live and changes note content without retaining the prior document', async () => {
    let editor: Editor | undefined
    const props = { noteId: 'first', content: initial, onReady: (instance: Editor) => { editor = instance },
      onChange: vi.fn(), onSelectionChange: vi.fn(), onError: vi.fn() }
    const result = render(EditorView, { props })
    await waitFor(() => expect(editor).toBeDefined())
    await result.rerender({ ...props, defaultFontSize: 24, taskMarkerSize: 36 })
    await waitFor(() => expect(editor!.view.dom.style.fontSize).toBe('24px'))
    await waitFor(() => expect(editor!.view.dom.querySelector<HTMLElement>('li')?.style.getPropertyValue('--task-size')).toBe('36px'))
    await result.rerender({ ...props, noteId: 'second', content: { type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text: '另一张便签' }] }] } })
    await waitFor(() => expect(editor!.getText()).toBe('另一张便签'))
    expect(editor!.getText()).not.toContain('原有内容')
    result.unmount()
  })
})
