import { describe, expect, it } from 'vitest'
import { DEFAULT_SETTINGS } from './types'

describe('DEFAULT_SETTINGS', () => {
  it('opens the core hub from the global shortcut by default', () => {
    expect(DEFAULT_SETTINGS.hotkeyAction).toBe('hub')
  })

  it('keeps local-first defaults', () => {
    expect(DEFAULT_SETTINGS.startOnBoot).toBe(false)
    expect(DEFAULT_SETTINGS.theme).toBe('system')
  })
})
