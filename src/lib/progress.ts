import type { MarkdownNode } from './markdown'

export const PROGRESS_LABELS = { todo: '待开始', doing: '进行中', done: '已完成', blocked: '受阻' } as const
export type ProgressState = keyof typeof PROGRESS_LABELS
export function unfinishedState(value: unknown): Exclude<ProgressState, 'done'> {
  return value === 'doing' || value === 'blocked' ? value : 'todo'
}
export function taskState(node: MarkdownNode): ProgressState {
  return node.attrs?.checked ? 'done' : unfinishedState(node.attrs?.progressState)
}
function ownText(node: MarkdownNode): string {
  if (node.type === 'taskList') return ''
  if (node.type === 'image') return String(node.attrs?.alt || '图片')
  return node.text ?? (node.content ?? []).map(ownText).join('')
}
/** Descendants of a task are supporting detail, never extra top-level steps. */
export function progressSummary(doc: MarkdownNode) {
  if (doc.attrs?.progressEnabled !== true) return { enabled: false, total: 0, done: 0, blocked: 0, current: '', doing: 0 }
  const steps: Array<{ text: string; state: ProgressState }> = []
  function visit(node: MarkdownNode) {
    if (node.type === 'taskItem') {
      const text = ownText(node).trim()
      if (text) steps.push({ text, state: taskState(node) })
      return
    }
    node.content?.forEach(visit)
  }
  visit(doc)
  const doing = steps.filter(step => step.state === 'doing')
  const done = steps.filter(step => step.state === 'done').length
  return {
    enabled: doc.attrs?.progressEnabled === true, total: steps.length, done,
    blocked: steps.filter(step => step.state === 'blocked').length,
    current: doing[0]?.text ?? steps.find(step => step.state === 'todo')?.text ?? '',
    doing: doing.length,
  }
}

/** Export-only text. Never use this for note titles or full-text indexes. */
export function progressPlainText(doc: MarkdownNode): string {
  const enabled = doc.attrs?.progressEnabled === true
  function render(node: MarkdownNode): string {
    if (node.type === 'text') return node.text ?? ''
    if (node.type === 'hardBreak') return '\n'
    if (node.type === 'image') return String(node.attrs?.alt || '图片')
    const inline = node.type === 'paragraph' || node.type === 'heading'
    const text = (node.content ?? []).map(render).join(inline ? '' : '\n')
    return node.type === 'taskItem' && enabled ? `【${PROGRESS_LABELS[taskState(node)]}】${text}` : text
  }
  return render(doc)
}
