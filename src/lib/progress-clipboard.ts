import { DOMSerializer, type DOMOutputSpec, type Schema } from '@tiptap/pm/model'
import { PROGRESS_LABELS, taskState } from './progress'

/** Include status in native rich clipboard targets; imports strip this annotation. */
export function progressClipboardSerializer(schema: Schema, enabled: () => boolean) {
  const original = DOMSerializer.fromSchema(schema)
  const taskItem = original.nodes.taskItem
  return new DOMSerializer({ ...original.nodes, taskItem: node => {
    const spec = taskItem(node)
    if (!enabled() || !Array.isArray(spec)) return spec
    return [spec[0] as string, ...spec.slice(1), ['span', { 'data-qingjian-progress-label': 'true' }, `【${PROGRESS_LABELS[taskState({ attrs: node.attrs })]}】`]] as DOMOutputSpec
  } }, original.marks)
}
