import { Fragment } from '@tiptap/pm/model'
import type { Node as ProseMirrorNode, NodeType } from '@tiptap/pm/model'
import type { EditorView } from '@tiptap/pm/view'
import {
  __insertCells as insertCells,
  isInTable,
  selectedRect,
  type __Area as CellArea,
} from '@tiptap/pm/tables'

const MAX_PASTE_CELLS = 20_000

export function shouldUsePlainTextGrid(text: string, html: string) {
  return (text.includes('\n') || text.includes('\t'))
    && !/<table(?:\s|>)/i.test(html)
}

export function parsePlainTextGrid(text: string): string[][] | undefined {
  const normalized = text.replace(/\r\n?/g, '\n')
  if (!normalized.includes('\n') && !normalized.includes('\t')) return

  const rows = normalized.split('\n')
  // Clipboard formats commonly terminate records with one newline. It is a
  // separator, not an extra empty Excel row.
  if (normalized.endsWith('\n')) rows.pop()
  if (!rows.length) return

  const grid = rows.map((row) => row.split('\t'))
  const width = Math.max(1, ...grid.map((row) => row.length))
  if (grid.length * width > MAX_PASTE_CELLS) {
    throw new RangeError(`一次最多向表格粘贴 ${MAX_PASTE_CELLS} 个单元格`)
  }
  return grid.map((row) => [
    ...row,
    ...Array.from({ length: width - row.length }, () => ''),
  ])
}

function cellTypeAt(
  table: ProseMirrorNode,
  map: ReturnType<typeof selectedRect>['map'],
  row: number,
  column: number,
  fallback: NodeType,
) {
  if (row < map.height && column < map.width) {
    const cell = table.nodeAt(map.map[row * map.width + column])
    if (cell?.type.spec.tableRole === 'cell' || cell?.type.spec.tableRole === 'header_cell') {
      return cell.type
    }
  }
  if (row < map.height && map.width > 0) {
    const cell = table.nodeAt(map.map[row * map.width + Math.min(column, map.width - 1)])
    if (cell?.type.spec.tableRole === 'cell' || cell?.type.spec.tableRole === 'header_cell') {
      return cell.type
    }
  }
  return fallback
}

export function pastePlainTextGrid(view: EditorView, text: string): boolean {
  const grid = parsePlainTextGrid(text)
  if (!grid || !isInTable(view.state)) return false

  const rect = selectedRect(view.state)
  const schema = view.state.schema
  const paragraph = schema.nodes.paragraph
  const fallbackCell = schema.nodes.tableCell ?? schema.nodes.tableHeader
  if (!paragraph || !fallbackCell) return false

  const rows = grid.map((values, rowOffset) => {
    const cells = values.map((value, columnOffset) => {
      const cellType = cellTypeAt(
        rect.table,
        rect.map,
        rect.top + rowOffset,
        rect.left + columnOffset,
        fallbackCell,
      )
      const content = paragraph.create(
        null,
        value ? schema.text(value) : undefined,
      )
      return cellType.createAndFill(null, content) ?? cellType.create(null, content)
    })
    return Fragment.from(cells)
  })

  const area: CellArea = {
    width: grid[0].length,
    height: grid.length,
    rows,
  }
  insertCells(view.state, view.dispatch, rect.tableStart, rect, area)
  view.focus()
  return true
}
