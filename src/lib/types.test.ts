import { describe, expect, it } from 'vitest'
import { DEFAULT_SETTINGS, isReservedGlobalHotkey, normalizeHotkeyAction } from './types'

describe('DEFAULT_SETTINGS', () => {
  it('opens the last note from the global shortcut by default', () => {
    expect(DEFAULT_SETTINGS.hotkeyAction).toBe('last-note')
  })

  it('migrates the legacy hub shortcut without reopening the hub', () => {
    expect(normalizeHotkeyAction('hub')).toBe('last-note')
    expect(normalizeHotkeyAction(undefined)).toBe('last-note')
    expect(normalizeHotkeyAction('new-note')).toBe('new-note')
  })

  it('protects fixed rescue shortcuts from user reassignment', () => {
    expect(isReservedGlobalHotkey('Ctrl+Alt+H')).toBe(true)
    expect(isReservedGlobalHotkey('ctrl+alt+l')).toBe(true)
    expect(isReservedGlobalHotkey('Ctrl+Alt+N')).toBe(false)
  })

  it('keeps local-first defaults', () => {
    expect(DEFAULT_SETTINGS.startOnBoot).toBe(false)
    expect(DEFAULT_SETTINGS.themeId).toBe('default-light')
    expect(DEFAULT_SETTINGS.checkUpdatesOnStartup).toBe(true)
    expect(DEFAULT_SETTINGS.defaultNoteBackgroundOpacity).toBe(100)
    expect(DEFAULT_SETTINGS.defaultNoteTextOpacity).toBe(100)
    expect(DEFAULT_SETTINGS.defaultLineHeight).toBe(1.55)
    expect(DEFAULT_SETTINGS.capsuleDisplay).toBe('comfortable')
    expect(DEFAULT_SETTINGS.autoHideAtScreenEdge).toBe(true)
  })
})
