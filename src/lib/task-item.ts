import TaskItem from '@tiptap/extension-task-item'
import { taskMarkerPixels } from './appearance'
import { PROGRESS_LABELS, taskState, unfinishedState } from './progress'

/** Keep Tiptap's editing/checkbox semantics, measuring only changed task nodes. */
export const SizedTaskItem = TaskItem.extend({
  addAttributes() {
    return { ...this.parent?.(), progressState: {
      default: 'todo',
      keepOnSplit: false,
      parseHTML: element => unfinishedState(element.getAttribute('data-progress-state')),
      renderHTML: attributes => ({ 'data-progress-state': unfinishedState(attributes.progressState) }),
    } }
  },
  addNodeView() {
    const create = this.parent?.()
    if (!create) return null
    return props => {
      const view = create(props)
      const item = view.dom as HTMLElement
      const root = props.view.dom
      let scheduled = false
      let disposed = false
      let currentNode = props.node
      let status: HTMLSelectElement | undefined
      function syncProgress() {
        const enabled = props.view.state.doc.attrs.progressEnabled === true
        if (!enabled) { status?.remove(); status = undefined; return }
        if (!status) {
          status = document.createElement('select')
          status.className = 'task-progress-state'
          status.contentEditable = 'false'
          for (const [value, label] of Object.entries(PROGRESS_LABELS)) {
            const option = document.createElement('option'); option.value = value; option.textContent = label
            status.append(option)
          }
          status.addEventListener('keydown', event => {
            if (event.key === 'Escape') event.stopPropagation()
            if ((event.ctrlKey || event.metaKey) && !event.altKey && ['z', 'y'].includes(event.key.toLowerCase())) {
              event.preventDefault(); event.stopPropagation()
              if (!props.editor.isEditable) return
              if (event.key.toLowerCase() === 'y' || event.shiftKey) props.editor.commands.redo()
              else props.editor.commands.undo()
            }
          })
          status.addEventListener('change', () => {
            if (!props.editor.isEditable) { syncProgress(); return }
            const position = props.getPos()
            if (typeof position !== 'number' || !status) return
            const value = status.value
            props.view.dispatch(props.view.state.tr.setNodeMarkup(position, undefined, {
              ...currentNode.attrs, checked: value === 'done',
              progressState: value === 'done' ? currentNode.attrs.progressState : unfinishedState(value),
            }))
          })
          item.insertBefore(status, view.contentDOM ?? null)
        }
        status.value = taskState({ attrs: currentNode.attrs })
        status.disabled = !props.editor.isEditable
        status.setAttribute('aria-label', `步骤状态：${currentNode.firstChild?.textContent || '空步骤'}`)
      }
      root.addEventListener('qingjian:progress', syncProgress)
      syncProgress()
      function measure() {
        const block = (view.contentDOM as HTMLElement | undefined)?.firstElementChild
        if (!(block instanceof HTMLElement)) return
        const firstText = document.createTreeWalker(block, NodeFilter.SHOW_TEXT).nextNode()
        const ink = firstText?.parentElement ?? block
        const style = getComputedStyle(ink)
        const fontSize = Number.parseFloat(style.fontSize) || 16
        const lineHeight = Number.parseFloat(getComputedStyle(block).lineHeight) || fontSize * 1.55
        const setting = Number.parseInt(getComputedStyle(root).getPropertyValue('--task-marker-setting'), 10) || 0
        const values = {
          '--task-size': `${taskMarkerPixels(fontSize, setting)}px`,
          '--task-line-height': `${Math.max(lineHeight, taskMarkerPixels(fontSize, setting))}px`,
        }
        for (const [key, value] of Object.entries(values)) {
          if (item.style.getPropertyValue(key) !== value) item.style.setProperty(key, value)
        }
      }
      function schedule() {
        if (scheduled) return
        scheduled = true
        queueMicrotask(() => { scheduled = false; if (!disposed) measure() })
      }
      const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(schedule)
      if (view.contentDOM) observer?.observe(view.contentDOM as Element)
      root.addEventListener('qingjian:task-style', schedule)
      schedule()
      return {
        ...view,
        update(node, decorations, innerDecorations) {
          const updated = view.update?.(node, decorations, innerDecorations) ?? false
          if (updated) { currentNode = node; syncProgress(); schedule() }
          return updated
        },
        stopEvent(event) {
          if (status && event.target instanceof Node && status.contains(event.target)) return true
          return view.stopEvent?.(event) ?? false
        },
        ignoreMutation(record) {
          if (record.type === 'childList' && record.target === item) return true
          if (status && (record.target === status || status.contains(record.target))) return true
          if (record.type === 'attributes' && record.target === item && record.attributeName === 'style') return true
          return view.ignoreMutation?.(record) ?? false
        },
        destroy() {
          disposed = true; observer?.disconnect()
          root.removeEventListener('qingjian:task-style', schedule)
          root.removeEventListener('qingjian:progress', syncProgress)
          view.destroy?.()
        },
      }
    }
  },
})
