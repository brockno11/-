import { beforeEach, describe, expect, it } from 'vitest'
import { backend } from './backend'
import type { Note } from './types'

beforeEach(() => {
  const note: Note = {
    id: 'test-note', title: '测试', preview: '只有前160字的摘要',
    content: { type: 'doc', content: [{ type: 'paragraph', content: [{ type: 'text', text: '开头'.repeat(100) + '独有后段关键字' }] }] },
    createdAt: '2026-01-01', updatedAt: '2026-01-01', pinned: true, archived: false,
    trashedAt: null, folderId: null, sortOrder: 0, tags: [{ id: 'tag', name: '保留标签', color: '#888' }],
  }
  localStorage.setItem('qingjian.mock.notes', JSON.stringify([note]))
})

describe('local backend contract', () => {
  it('searches the full document beyond the preview', async () => {
    expect(await backend.listNotes('独有后段关键字')).toHaveLength(1)
    expect(await backend.listNotes('不存在的词')).toHaveLength(0)
  })
  it('content saves do not replace pinned status or tags', async () => {
    const saved = await backend.saveNote({
      id: 'test-note', title: '新正文', plainText: '新正文', content: { type: 'doc' },
    })
    expect(saved.pinned).toBe(true)
    expect(saved.tags.map(t => t.name)).toEqual(['保留标签'])
  })
})
