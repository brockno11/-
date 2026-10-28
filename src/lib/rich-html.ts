import { taskMarkerPixels } from './appearance'
import { PROGRESS_LABELS, unfinishedState } from './progress'

/** Carry inherited note typography into clipboard HTML and standalone exports. */
export function richHtml(html: string, appearance: { fontFamily: string; fontSize: number; lineHeight: number; taskMarkerSize: number }, progressEnabled = false) {
  const wrapper = document.createElement('div')
  wrapper.style.fontFamily = appearance.fontFamily
  wrapper.style.fontSize = `${appearance.fontSize}px`
  wrapper.style.lineHeight = String(appearance.lineHeight)
  wrapper.innerHTML = html
  wrapper.querySelectorAll<HTMLElement>('li[data-type="taskItem"]').forEach(item => {
    const text = document.createTreeWalker(item.querySelector('p') ?? item, NodeFilter.SHOW_TEXT).nextNode()
    const styled = text?.parentElement?.closest<HTMLElement>('[style*="font-size"]')
    const font = Number.parseFloat(styled?.style.fontSize ?? '') || appearance.fontSize
    const size = taskMarkerPixels(font, appearance.taskMarkerSize)
    item.style.setProperty('--task-size', `${size}px`)
    const checkbox = item.querySelector<HTMLInputElement>('input[type="checkbox"]')
    if (checkbox) { checkbox.style.width = `${size}px`; checkbox.style.height = `${size}px` }
    if (progressEnabled) {
      const state = item.dataset.checked === 'true' ? 'done' : unfinishedState(item.dataset.progressState)
      const label = document.createElement('span')
      label.dataset.qingjianProgressLabel = 'true'
      label.textContent = `【${PROGRESS_LABELS[state]}】`
      item.querySelector('p')?.prepend(label)
    }
  })
  return wrapper.outerHTML
}
