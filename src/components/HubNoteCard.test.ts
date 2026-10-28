import { fireEvent, render, screen } from '@testing-library/svelte'
import { describe, expect, it, vi } from 'vitest'
import HubNoteCard from './HubNoteCard.svelte'
import type { Folder, NoteSummary } from '../lib/types'

const note: NoteSummary = {
  id: 'note-1',
  title: '首页改版计划',
  preview: '整理首页层级并完成视觉验收',
  createdAt: '2026-08-05T09:00:00.000Z',
  updatedAt: '2026-08-06T10:00:00.000Z',
  pinned: false,
  archived: false,
  trashedAt: null,
  folderId: 'folder-1',
  sortOrder: 1,
  tags: [{ id: 'tag-1', name: '设计', color: '#b7863f' }],
}

const folders: Folder[] = [{
  id: 'folder-1',
  name: '产品',
  sortOrder: 0,
  createdAt: '2026-08-05T09:00:00.000Z',
  updatedAt: '2026-08-05T09:00:00.000Z',
}]

function renderCard(overrides: Record<string, unknown> = {}) {
  const callbacks = {
    onActivate: vi.fn(),
    onTogglePin: vi.fn(),
    onOpenMenu: vi.fn(),
    onContextMenu: vi.fn(),
    onDragStart: vi.fn(),
    onDragOver: vi.fn(),
    onDrop: vi.fn(),
    onDragEnd: vi.fn(),
  }

  const result = render(HubNoteCard, {
    props: {
      note,
      folders,
      formattedDate: '今天 18:00',
      ...callbacks,
      ...overrides,
    },
  })

  return { ...result, callbacks }
}

describe('HubNoteCard', () => {
  it('renders stable title, summary and metadata layers', () => {
    renderCard({ desktopVisible: true })

    expect(screen.getByText('首页改版计划')).toBeInTheDocument()
    expect(screen.getByText('整理首页层级并完成视觉验收')).toBeInTheDocument()
    expect(screen.getByText('产品')).toBeInTheDocument()
    expect(screen.getByText('设计')).toBeInTheDocument()
    expect(screen.getByLabelText('已显示在桌面')).toBeInTheDocument()
  })

  it('highlights matching search text in title and summary', () => {
    const { container } = renderCard({ query: '首页' })

    expect(Array.from(container.querySelectorAll('mark')).map((mark) => mark.textContent)).toContain('首页')
  })

  it('supports keyboard activation and discoverable card actions', async () => {
    const { callbacks } = renderCard()
    const main = screen.getByRole('button', { name: '打开 首页改版计划' })
    const pin = screen.getByRole('button', { name: '列表置顶' })

    await fireEvent.keyDown(pin, { key: 'Enter' })
    expect(callbacks.onActivate).not.toHaveBeenCalled()
    await fireEvent.keyDown(main, { key: 'Enter' })
    await fireEvent.click(pin)
    await fireEvent.click(screen.getByLabelText('更多操作'))

    expect(callbacks.onActivate).toHaveBeenCalledWith(note)
    expect(callbacks.onTogglePin).toHaveBeenCalledWith(note)
    expect(callbacks.onOpenMenu).toHaveBeenCalledWith(expect.any(MouseEvent), note)
  })

  it('exposes selection state without opening the note', async () => {
    const { callbacks } = renderCard({ selectionMode: true, selected: true })

    const selection = screen.getByRole('button', { name: '取消选择 首页改版计划' })
    expect(selection).toHaveAttribute('aria-pressed', 'true')
    await fireEvent.click(selection)

    expect(callbacks.onActivate).toHaveBeenCalledWith(note)
  })

  it('allows clicking the visible selection marker', async () => {
    const { callbacks } = renderCard({ selectionMode: true, selected: false })
    await fireEvent.click(screen.getByRole('button', { name: '勾选 首页改版计划' }))
    expect(callbacks.onActivate).toHaveBeenCalledOnce()
  })
})
