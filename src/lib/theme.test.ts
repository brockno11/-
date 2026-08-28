import { describe, expect, it } from 'vitest'
import { DEFAULT_LIGHT_TOKENS, normalizeThemeId, themeClass } from './theme'

describe('theme foundation', () => {
  it('migrates legacy and unknown light themes to the neutral default', () => {
    expect(normalizeThemeId('light')).toBe('default-light')
    expect(normalizeThemeId(undefined)).toBe('default-light')
    expect(themeClass('default-light')).toBe('theme-light')
  })

  it('keeps the approved semantic white palette', () => {
    expect(DEFAULT_LIGHT_TOKENS).toMatchObject({
      canvas: '#FFFFFF', surface: '#F7F8FA', border: '#E5E7EB',
      text: '#18181B', textMuted: '#6B7280', accent: '#52657D', danger: '#C2413A',
    })
  })
})
