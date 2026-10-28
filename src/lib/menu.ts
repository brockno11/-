import { quietScroll } from './quiet-scroll'

/** Shared keyboard behavior for the small, local command menus. */
export function menu(node: HTMLElement, options: { onClose: () => void; returnFocus?: HTMLElement }) {
  const scrolling = quietScroll(node)
  const source = options.returnFocus ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null)
  const items = () => [...node.querySelectorAll<HTMLElement>('button:not(:disabled), [role="menuitem"]:not(:disabled)')]
  items().forEach(item => item.setAttribute('role', 'menuitem'))
  const frame = requestAnimationFrame(() => items()[0]?.focus())
  function keydown(event: KeyboardEvent) {
    const buttons = items()
    const current = buttons.indexOf(document.activeElement as HTMLElement)
    let index: number | undefined
    if (event.key === 'ArrowDown') index = (current + 1) % buttons.length
    if (event.key === 'ArrowUp') index = (current - 1 + buttons.length) % buttons.length
    if (event.key === 'Home') index = 0
    if (event.key === 'End') index = buttons.length - 1
    if (index !== undefined) { event.preventDefault(); event.stopPropagation(); buttons[index]?.focus() }
    if (event.key === 'Escape' || event.key === 'Tab') {
      if (event.key === 'Escape') event.preventDefault()
      event.stopPropagation(); options.onClose(); source?.focus()
    }
  }
  node.addEventListener('keydown', keydown)
  return {
    update(next: typeof options) { options = next },
    destroy() { scrolling.destroy(); cancelAnimationFrame(frame); node.removeEventListener('keydown', keydown) },
  }
}

/** Nonmodal form popovers preserve native select keys and normal Tab order. */
export function popover(node: HTMLElement, options: { onClose: () => void; returnFocus?: HTMLElement }) {
  const scrolling = quietScroll(node)
  const source = options.returnFocus ?? (document.activeElement instanceof HTMLElement ? document.activeElement : null)
  const frame = requestAnimationFrame(() => node.querySelector<HTMLElement>('input:not(:disabled), select:not(:disabled), button:not(:disabled)')?.focus())
  const keydown = (event: KeyboardEvent) => {
    if (event.key !== 'Escape') return
    event.preventDefault(); event.stopPropagation(); options.onClose(); source?.focus()
  }
  node.addEventListener('keydown', keydown)
  return { destroy() { scrolling.destroy(); cancelAnimationFrame(frame); node.removeEventListener('keydown', keydown) } }
}
