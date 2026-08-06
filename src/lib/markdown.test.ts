import { describe, expect, it } from 'vitest'
import { serializeMarkdown } from './markdown'

describe('serializeMarkdown', () => {
  it('serializes semantic blocks and inline marks', () => {
    expect(serializeMarkdown({
      type: 'doc',
      content: [
        { type: 'heading', attrs: { level: 2 }, content: [{ type: 'text', text: '今日工作' }] },
        {
          type: 'paragraph',
          content: [
            { type: 'text', text: '已经', marks: [{ type: 'bold' }] },
            { type: 'text', text: '完成' },
          ],
        },
        {
          type: 'blockquote',
          content: [{ type: 'paragraph', content: [{ type: 'text', text: '明天发布' }] }],
        },
      ],
    })).toBe('## 今日工作\n\n**已经**完成\n\n> 明天发布\n')
  })

  it('serializes task lists, nested lists and code blocks', () => {
    expect(serializeMarkdown({
      type: 'doc',
      content: [
        {
          type: 'taskList',
          content: [
            {
              type: 'taskItem', attrs: { checked: true },
              content: [
                { type: 'paragraph', content: [{ type: 'text', text: '完成编辑器' }] },
                {
                  type: 'bulletList',
                  content: [{
                    type: 'listItem',
                    content: [{ type: 'paragraph', content: [{ type: 'text', text: '补测试' }] }],
                  }],
                },
              ],
            },
          ],
        },
        { type: 'codeBlock', attrs: { language: 'ts' }, content: [{ type: 'text', text: 'const ready = true' }] },
      ],
    })).toBe('- [x] 完成编辑器\n  - 补测试\n\n```ts\nconst ready = true\n```\n')
  })

  it('uses portable asset markers and GFM tables', () => {
    const markdown = serializeMarkdown({
      type: 'doc',
      content: [
        { type: 'image', attrs: { alt: '示意图', src: 'asset://preview', assetPath: 'abc.png' } },
        {
          type: 'table', content: [
            { type: 'tableRow', content: [
              { type: 'tableHeader', content: [{ type: 'paragraph', content: [{ type: 'text', text: '名称' }] }] },
              { type: 'tableHeader', content: [{ type: 'paragraph', content: [{ type: 'text', text: '状态' }] }] },
            ] },
            { type: 'tableRow', content: [
              { type: 'tableCell', content: [{ type: 'paragraph', content: [{ type: 'text', text: '轻笺' }] }] },
              { type: 'tableCell', content: [{ type: 'paragraph', content: [{ type: 'text', text: '完成' }] }] },
            ] },
          ],
        },
      ],
    }, { title: '版本记录' })

    expect(markdown).toContain('# 版本记录')
    expect(markdown).toContain('![示意图](qingjian-asset:abc.png)')
    expect(markdown).toContain('| 名称 | 状态 |\n| --- | --- |\n| 轻笺 | 完成 |')
  })

  it('keeps Markdown-looking plain paragraphs as plain text', () => {
    expect(serializeMarkdown({
      type: 'doc',
      content: [
        { type: 'paragraph', content: [{ type: 'text', text: '# 这不是标题' }] },
        { type: 'paragraph', content: [{ type: 'text', text: '1. 这不是列表' }] },
        { type: 'paragraph', content: [{ type: 'text', text: '---' }] },
      ],
    })).toBe('\\# 这不是标题\n\n1\\. 这不是列表\n\n\\---\n')
  })
})
