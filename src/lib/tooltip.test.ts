import { expect, it, vi } from 'vitest'
import { tooltip } from './tooltip'

it('delays pointer help, shows keyboard help and consumes Escape before window actions', () => {
  vi.useFakeTimers()
  const button = document.createElement('button'); document.body.append(button)
  const action = tooltip(button, '展开完整编辑')
  button.dispatchEvent(new MouseEvent('mouseenter'))
  vi.advanceTimersByTime(399)
  expect(button.hasAttribute('aria-describedby')).toBe(false)
  vi.advanceTimersByTime(1)
  expect(button.getAttribute('aria-describedby')).toBe('qingjian-tooltip')
  const bubbled = vi.fn(); document.addEventListener('keydown', bubbled)
  const escape = new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
  button.dispatchEvent(escape)
  expect(escape.defaultPrevented).toBe(true)
  expect(bubbled).not.toHaveBeenCalled()
  expect(button.hasAttribute('aria-describedby')).toBe(false)
  button.dispatchEvent(new FocusEvent('focus'))
  expect(button.hasAttribute('aria-describedby')).toBe(true)
  action.destroy(); button.remove(); document.removeEventListener('keydown', bubbled)
  expect(document.querySelector('.app-tooltip.visible')).toBeNull()
  vi.useRealTimers()
})
