import { fireEvent, render, screen, waitFor } from '@testing-library/svelte'
import { describe, expect, it, vi } from 'vitest'
import SettingsModal from './SettingsModal.svelte'
import { DEFAULT_SETTINGS } from '../lib/types'

function renderSettings() {
  const launcher = document.createElement('button')
  launcher.textContent = '设置'
  const background = document.createElement('main')
  background.className = 'qj-workbench'
  background.append(launcher)
  document.body.append(background)
  launcher.focus()

  const result = render(SettingsModal, {
    props: {
      settings: structuredClone(DEFAULT_SETTINGS),
      onSave: vi.fn(async () => undefined),
      onClose: vi.fn(),
      onBackup: vi.fn(),
      onRestore: vi.fn(),
      onCheckUpdate: vi.fn(),
    },
  })
  return { ...result, launcher, background }
}

describe('SettingsModal focus management', () => {
  it('retains keyboard position through narrow category navigation', async () => {
    const originalWidth = window.innerWidth
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 404 })
    const { unmount, background } = renderSettings()
    try {
      const category = screen.getByRole('button', { name: /便签外观/ })
      await fireEvent.click(category)
      await waitFor(() => expect(screen.getByRole('button', { name: '所有设置' })).toHaveFocus())
      await fireEvent.click(screen.getByRole('button', { name: '所有设置' }))
      await waitFor(() => expect(screen.getByRole('button', { name: /便签外观/ })).toHaveFocus())
    } finally {
      unmount(); background.remove()
      Object.defineProperty(window, 'innerWidth', { configurable: true, value: originalWidth })
    }
  })

  it('moves focus inside, traps Tab, makes the background inert and restores focus', async () => {
    const { unmount, launcher, background } = renderSettings()
    const dialog = screen.getByRole('dialog', { name: '轻笺设置' })
    const close = screen.getByRole('button', { name: '关闭设置' })

    await waitFor(() => expect(close).toHaveFocus())
    expect(background).toHaveAttribute('aria-hidden', 'true')
    expect(background.inert).toBe(true)

    const focusable = Array.from(dialog.querySelectorAll<HTMLElement>(
      'button:not(:disabled), input:not(:disabled), select:not(:disabled), textarea:not(:disabled), a[href], [tabindex]:not([tabindex="-1"])',
    ))
    const last = focusable.at(-1)!

    await fireEvent.keyDown(dialog, { key: 'Tab', shiftKey: true })
    expect(last).toHaveFocus()
    await fireEvent.keyDown(dialog, { key: 'Tab' })
    expect(close).toHaveFocus()

    unmount()
    await waitFor(() => expect(launcher).toHaveFocus())
    expect(background).not.toHaveAttribute('aria-hidden')
    expect(background.inert).toBe(false)
    background.remove()
  })
})
