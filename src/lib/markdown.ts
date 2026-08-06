export interface MarkdownNode {
  type?: string
  text?: string
  attrs?: Record<string, unknown>
  marks?: Array<{ type?: string; attrs?: Record<string, unknown> }>
  content?: MarkdownNode[]
}

export interface MarkdownOptions {
  assetUrl?: (assetPath: string, source: string) => string
}

function attribute(node: MarkdownNode, name: string): unknown {
  return node.attrs?.[name]
}

function escapeText(value: string): string {
  return value.replace(/([\\`*_[\]<>])/g, '\\$1')
}

function escapeLabel(value: string): string {
  return value.replace(/([\\\[\]])/g, '\\$1')
}

function inlineCode(value: string): string {
  const runs = value.match(/`+/g) ?? []
  const fence = '`'.repeat(Math.max(1, ...runs.map((run) => run.length + 1)))
  const padding = /^\s|\s$/.test(value) ? ' ' : ''
  return `${fence}${padding}${value}${padding}${fence}`
}

function markdownUrl(value: string): string {
  const escaped = value.replace(/>/g, '%3E')
  return /[\s()]/.test(escaped) ? `<${escaped}>` : escaped
}

function serializeText(node: MarkdownNode): string {
  const value = node.text ?? ''
  const marks = node.marks ?? []
  if (marks.some((mark) => mark.type === 'code')) return inlineCode(value)

  let rendered = escapeText(value)
  const has = (type: string) => marks.some((mark) => mark.type === type)
  if (has('bold')) rendered = `**${rendered}**`
  if (has('italic')) rendered = `*${rendered}*`
  if (has('strike')) rendered = `~~${rendered}~~`
  if (has('underline')) rendered = `<u>${rendered}</u>`

  const link = marks.find((mark) => mark.type === 'link')
  const href = typeof link?.attrs?.href === 'string' ? link.attrs.href : ''
  if (href) {
    const title = typeof link?.attrs?.title === 'string' && link.attrs.title
      ? ` "${link.attrs.title.replace(/"/g, '\\"')}"`
      : ''
    rendered = `[${rendered}](${markdownUrl(href)}${title})`
  }
  return rendered
}

function serializeInline(nodes: MarkdownNode[] = [], options: MarkdownOptions): string {
  return nodes.map((node) => {
    if (node.type === 'text') return serializeText(node)
    if (node.type === 'hardBreak') return '  \n'
    if (node.type === 'image') return serializeImage(node, options)
    return serializeInline(node.content, options)
  }).join('')
}

function escapeBlockStart(value: string): string {
  if (/^\s{0,3}\d+[.)]\s/.test(value)) {
    return value.replace(/^(\s{0,3}\d+)([.)])/, '$1\\$2')
  }
  if (/^\s{0,3}(?:#{1,6}\s|>\s|[-+*]\s|(?:-{3,}|_{3,}|\*{3,})\s*$)/.test(value)) {
    return value.replace(/^(\s{0,3})(.)/, '$1\\$2')
  }
  return value
}

function imageSource(node: MarkdownNode, options: MarkdownOptions): string {
  const assetPath = typeof attribute(node, 'assetPath') === 'string'
    ? String(attribute(node, 'assetPath'))
    : ''
  const source = typeof attribute(node, 'src') === 'string' ? String(attribute(node, 'src')) : ''
  if (options.assetUrl) return options.assetUrl(assetPath, source)
  return assetPath ? `qingjian-asset:${assetPath}` : source
}

function serializeImage(node: MarkdownNode, options: MarkdownOptions): string {
  const alt = typeof attribute(node, 'alt') === 'string' ? String(attribute(node, 'alt')) : ''
  const title = typeof attribute(node, 'title') === 'string' && attribute(node, 'title')
    ? ` "${String(attribute(node, 'title')).replace(/"/g, '\\"')}"`
    : ''
  return `![${escapeLabel(alt)}](${markdownUrl(imageSource(node, options))}${title})`
}

function indent(value: string, spaces = 2): string {
  const prefix = ' '.repeat(spaces)
  return value.split('\n').map((line) => `${prefix}${line}`).join('\n')
}

function serializeList(node: MarkdownNode, options: MarkdownOptions): string {
  const ordered = node.type === 'orderedList'
  const task = node.type === 'taskList'
  const startValue = Number(attribute(node, 'start'))
  const start = Number.isFinite(startValue) && startValue > 0 ? startValue : 1

  return (node.content ?? []).map((item, index) => {
    const checked = Boolean(attribute(item, 'checked'))
    const prefix = task ? `- [${checked ? 'x' : ' '}] ` : ordered ? `${start + index}. ` : '- '
    const children = item.content ?? []
    const first = children[0]
    const firstLine = first?.type === 'paragraph'
      ? escapeBlockStart(serializeInline(first.content, options))
      : first ? serializeBlock(first, options) : ''
    const remainder = children.slice(1)
      .map((child) => serializeBlock(child, options))
      .filter(Boolean)
      .map((block) => indent(block))
      .join('\n')
    return remainder ? `${prefix}${firstLine}\n${remainder}` : `${prefix}${firstLine}`
  }).join('\n')
}

function tableCell(node: MarkdownNode, options: MarkdownOptions): string {
  return (node.content ?? [])
    .map((child) => child.type === 'paragraph'
      ? serializeInline(child.content, options)
      : serializeBlock(child, options))
    .join('<br>')
    .replace(/\|/g, '\\|')
    .replace(/\r?\n/g, '<br>')
}

function serializeTable(node: MarkdownNode, options: MarkdownOptions): string {
  const rows = (node.content ?? []).map((row) =>
    (row.content ?? []).map((cell) => tableCell(cell, options)),
  )
  if (!rows.length) return ''
  const width = Math.max(...rows.map((row) => row.length))
  const normalized = rows.map((row) => [...row, ...Array(width - row.length).fill('')])
  const line = (row: string[]) => `| ${row.join(' | ')} |`
  return [line(normalized[0]), line(Array(width).fill('---')), ...normalized.slice(1).map(line)].join('\n')
}

function serializeBlock(node: MarkdownNode, options: MarkdownOptions): string {
  switch (node.type) {
    case 'paragraph':
      return escapeBlockStart(serializeInline(node.content, options))
    case 'heading': {
      const requested = Number(attribute(node, 'level'))
      const level = Math.min(6, Math.max(1, Number.isFinite(requested) ? requested : 1))
      return `${'#'.repeat(level)} ${serializeInline(node.content, options)}`
    }
    case 'blockquote':
      return (node.content ?? [])
        .map((child) => serializeBlock(child, options))
        .join('\n\n')
        .split('\n')
        .map((line) => `> ${line}`)
        .join('\n')
    case 'codeBlock': {
      const code = (node.content ?? []).map((child) => child.text ?? '').join('')
      const language = typeof attribute(node, 'language') === 'string' ? attribute(node, 'language') : ''
      const longest = Math.max(0, ...(code.match(/`+/g) ?? []).map((run) => run.length))
      const fence = '`'.repeat(Math.max(3, longest + 1))
      return `${fence}${language ?? ''}\n${code}\n${fence}`
    }
    case 'bulletList':
    case 'orderedList':
    case 'taskList':
      return serializeList(node, options)
    case 'horizontalRule':
      return '---'
    case 'image':
      return serializeImage(node, options)
    case 'table':
      return serializeTable(node, options)
    default:
      return (node.content ?? []).map((child) => serializeBlock(child, options)).filter(Boolean).join('\n\n')
  }
}

export function serializeMarkdown(document: MarkdownNode, options: MarkdownOptions = {}): string {
  const blocks = (document.content ?? []).map((node) => serializeBlock(node, options))
  return `${blocks.filter(Boolean).join('\n\n').trimEnd()}\n`
}
