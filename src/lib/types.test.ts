import { describe, expect, it } from 'vitest'
import { DEFAULT_SETTINGS, normalizeHotkeyAction } from './types'

describe('DEFAULT_SETTINGS', () => {
  it('opens the last note from the global shortcut by default', () => {
    expect(DEFAULT_SETTINGS.hotkeyAction).toBe('last-note')
  })

  it('migrates the legacy hub shortcut without reopening the hub', () => {
    expect(normalizeHotkeyAction('hub')).toBe('last-note')
    expect(normalizeHotkeyAction(undefined)).toBe('last-note')
    expect(normalizeHotkeyAction('new-note')).toBe('new-note')
  })

  it('keeps local-first defaults', () => {
    expect(DEFAULT_SETTINGS.startOnBoot).toBe(false)
    expect(DEFAULT_SETTINGS.theme).toBe('system')
    expect(DEFAULT_SETTINGS.checkUpdatesOnStartup).toBe(true)
    expect(DEFAULT_SETTINGS.defaultNoteBackgroundOpacity).toBe(100)
    expect(DEFAULT_SETTINGS.defaultNoteTextOpacity).toBe(100)
  })
})
