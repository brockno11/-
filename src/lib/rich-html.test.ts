import { expect, it } from 'vitest'
import { richHtml } from './rich-html'

it('exports inherited typography, checked state and task marker size together', () => {
  const html = richHtml('<ul data-type="taskList"><li data-type="taskItem" data-checked="true"><label><input type="checkbox" checked></label><div><p><span style="font-size:24px">完成事项</span></p></div></li></ul>', {
    fontFamily: 'Microsoft YaHei UI', fontSize: 16, lineHeight: 1.55, taskMarkerSize: 0,
  })
  const doc = new DOMParser().parseFromString(html, 'text/html')
  expect(doc.querySelector<HTMLInputElement>('input')?.checked).toBe(true)
  expect(doc.querySelector<HTMLInputElement>('input')?.style.width).toBe('27px')
  expect(doc.body.firstElementChild?.getAttribute('style')).toContain('font-size: 16px')
})
