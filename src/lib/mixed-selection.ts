import { Extension } from '@tiptap/core'
import type { Node as ProseMirrorNode } from '@tiptap/pm/model'
import { NodeSelection, Plugin, PluginKey, Selection } from '@tiptap/pm/state'
import type { EditorState } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'
import type { EditorView } from '@tiptap/pm/view'
import type { Mappable } from '@tiptap/pm/transform'

export type ObjectRange = {
  from: number
  to: number
  type: 'image' | 'table'
}

const mixedSelectionKey = new PluginKey<DecorationSet>('qingjianMixedSelection')

export class MixedRangeSelection extends Selection {
  static create(doc: ProseMirrorNode, anchor: number, head: number) {
    return new MixedRangeSelection(doc.resolve(anchor), doc.resolve(head))
  }

  static fromJSON(doc: ProseMirrorNode, json: { anchor: number; head: number }) {
    return MixedRangeSelection.create(doc, json.anchor, json.head)
  }

  eq(selection: Selection) {
    return selection instanceof MixedRangeSelection
      && selection.anchor === this.anchor
      && selection.head === this.head
  }

  map(doc: ProseMirrorNode, mapping: Mappable) {
    return MixedRangeSelection.create(
      doc,
      mapping.map(this.anchor),
      mapping.map(this.head),
    )
  }

  toJSON() {
    return { type: 'qingjian-mixed-range', anchor: this.anchor, head: this.head }
  }
}

try {
  Selection.jsonID('qingjian-mixed-range', MixedRangeSelection)
} catch {
  // The module can be reloaded by Vite during development.
}

function tableAncestor(state: EditorState, position: number): number | undefined {
  const resolved = state.doc.resolve(Math.max(0, Math.min(position, state.doc.content.size)))
  for (let depth = resolved.depth; depth > 0; depth -= 1) {
    if (resolved.node(depth).type.name === 'table') return resolved.before(depth)
  }
  return undefined
}

export function selectedObjectRanges(
  doc: ProseMirrorNode,
  from: number,
  to: number,
): ObjectRange[] {
  if (from === to) return []
  const rangeFrom = Math.min(from, to)
  const rangeTo = Math.max(from, to)
  const ranges: ObjectRange[] = []
  doc.descendants((node, position) => {
    if (node.type.name !== 'image' && node.type.name !== 'table') return true
    const nodeTo = position + node.nodeSize
    if (position < rangeTo && nodeTo > rangeFrom) {
      ranges.push({
        from: position,
        to: nodeTo,
        type: node.type.name as ObjectRange['type'],
      })
    }
    return node.type.name !== 'table'
  })
  return ranges
}

export function expandMixedSelection(
  state: EditorState,
  dragStart: number,
  dragEnd = state.selection.head,
): { anchor: number; head: number } | undefined {
  const selection = state.selection
  if (dragStart === dragEnd || selection instanceof NodeSelection) return

  const startTable = tableAncestor(state, dragStart)
  const endTable = tableAncestor(state, dragEnd)
  const sameTable = startTable !== undefined && startTable === endTable
  const rawFrom = Math.min(dragStart, dragEnd)
  const rawTo = Math.max(dragStart, dragEnd)
  const objects = selectedObjectRanges(state.doc, rawFrom, rawTo)
    .filter((range) => range.type !== 'table' || !sameTable)
  if (!objects.length) return

  const from = Math.min(rawFrom, ...objects.map((range) => range.from))
  const to = Math.max(rawTo, ...objects.map((range) => range.to))

  return dragStart <= dragEnd
    ? { anchor: from, head: to }
    : { anchor: to, head: from }
}

export function selectionObjectRanges(state: EditorState): ObjectRange[] {
  const selection = state.selection
  if (selection.empty || selection instanceof NodeSelection) return []
  const sameTable = tableAncestor(state, selection.anchor) !== undefined
    && tableAncestor(state, selection.anchor) === tableAncestor(state, selection.head)
  return selectedObjectRanges(
    state.doc,
    selection.from,
    selection.to,
  )
    .filter((range) => range.type !== 'table' || !sameTable)
}

function selectionDecorations(state: EditorState) {
  const selection = state.selection
  if (selection.empty || selection instanceof NodeSelection) return DecorationSet.empty
  const decorations: Decoration[] = selectionObjectRanges(state)
    .map((range) => Decoration.node(range.from, range.to, {
      class: 'mixed-selected-object',
      'aria-selected': 'true',
    }))
  if (selection instanceof MixedRangeSelection) {
    state.doc.nodesBetween(selection.from, selection.to, (node, position) => {
      if (!node.isText) return true
      const from = Math.max(selection.from, position)
      const to = Math.min(selection.to, position + node.nodeSize)
      if (from < to) {
        decorations.push(Decoration.inline(from, to, {
          class: 'mixed-selected-text',
        }))
      }
      return false
    })
  }
  return DecorationSet.create(state.doc, decorations)
}

function nodePositionFromResolved(state: EditorState, position: number, type: string) {
  const resolved = state.doc.resolve(Math.max(0, Math.min(position, state.doc.content.size)))
  for (let depth = resolved.depth; depth > 0; depth -= 1) {
    if (resolved.node(depth).type.name === type) return resolved.before(depth)
  }
  const direct = state.doc.nodeAt(position)
  return direct?.type.name === type ? position : undefined
}

function selectObject(view: EditorView, position: number, shiftKey: boolean) {
  const node = view.state.doc.nodeAt(position)
  if (!node) return false
  let selection
  if (shiftKey) {
    const anchor = view.state.selection.anchor
    const farEdge = anchor <= position ? position + node.nodeSize : position
    selection = MixedRangeSelection.create(view.state.doc, anchor, farEdge)
  } else {
    selection = NodeSelection.create(view.state.doc, position)
  }
  view.dispatch(view.state.tr.setSelection(selection).scrollIntoView())
  view.focus()
  return true
}

function isTableGrip(event: MouseEvent) {
  const target = event.target
  if (!(target instanceof Element)) return
  const wrapper = target.closest('.tableWrapper')
  if (!(wrapper instanceof HTMLElement)) return
  const rect = wrapper.getBoundingClientRect()
  const insideGrip = event.clientX >= rect.left
    && event.clientX <= rect.left + 25
    && event.clientY >= rect.top
    && event.clientY <= rect.top + 24
  return insideGrip ? wrapper : undefined
}

export const MixedContentSelection = Extension.create({
  name: 'qingjianMixedContentSelection',

  addProseMirrorPlugins() {
    let dragStart: number | undefined

    return [
      new Plugin<DecorationSet>({
        key: mixedSelectionKey,
        state: {
          init: (_, state) => selectionDecorations(state),
          apply: (transaction, previous, _oldState, newState) =>
            transaction.docChanged || transaction.selectionSet
              ? selectionDecorations(newState)
              : previous,
        },
        props: {
          decorations: (state) => mixedSelectionKey.getState(state),
          handleDOMEvents: {
            mousedown: (view, rawEvent) => {
              const event = rawEvent as MouseEvent
              if (event.button !== 0) return false
              const target = event.target
              if (
                target instanceof Element
                && target.closest('[data-image-node-view], [data-resize-handle]')
              ) {
                dragStart = undefined
                return false
              }

              const grip = isTableGrip(event)
              if (grip) {
                event.preventDefault()
                const table = grip.querySelector('table')
                if (!table) return false
                const domPosition = view.posAtDOM(table, 0)
                const position = nodePositionFromResolved(view.state, domPosition, 'table')
                return position === undefined
                  ? false
                  : selectObject(view, position, event.shiftKey)
              }

              const coordinates = view.posAtCoords({ left: event.clientX, top: event.clientY })
              dragStart = coordinates?.pos
              return false
            },
            mouseup: (view, rawEvent) => {
              const start = dragStart
              dragStart = undefined
              if (start === undefined) return false
              const event = rawEvent as MouseEvent
              const end = view.posAtCoords({ left: event.clientX, top: event.clientY })?.pos
              if (end === undefined) return false
              queueMicrotask(() => {
                const expanded = expandMixedSelection(view.state, start, end)
                if (!expanded) return
                view.dispatch(view.state.tr.setSelection(
                  MixedRangeSelection.create(view.state.doc, expanded.anchor, expanded.head),
                ))
              })
              return false
            },
          },
        },
      }),
    ]
  },
})
