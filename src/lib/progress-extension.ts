import { Extension } from '@tiptap/core'
import { Plugin } from '@tiptap/pm/state'
import { progressClipboardSerializer } from './progress-clipboard'

/** Document attributes survive JSON saves without changing the existing node schema. */
export const ProgressDocument = Extension.create({
  name: 'progressDocument',
  addGlobalAttributes() {
    return [{ types: ['doc'], attributes: { progressEnabled: { default: false, rendered: false } } }]
  },
  addProseMirrorPlugins() {
    const editor = this.editor
    return [new Plugin({
      props: { clipboardSerializer: progressClipboardSerializer(editor.schema, () => editor.state.doc.attrs.progressEnabled === true) },
      view(view) {
      let previous: boolean | undefined
      const sync = () => {
        const enabled = view.state.doc.attrs.progressEnabled === true
        if (enabled === previous) return
        previous = enabled
        view.dom.dataset.progressEnabled = String(enabled)
        view.dom.dispatchEvent(new Event('qingjian:progress'))
      }
      sync()
      return { update: sync }
    } })]
  },
})
