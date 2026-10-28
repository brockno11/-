import { describe, expect, it } from 'vitest'
import { INHERITED_APPEARANCE, resolveAppearance, taskMarkerPixels } from './appearance'
import { DEFAULT_SETTINGS } from './types'

describe('note appearance inheritance', () => {
  it('follows global changes without overriding explicit note values', () => {
    const global = { ...DEFAULT_SETTINGS, defaultFontSize: 24, defaultTaskMarkerSize: 30 }
    expect(resolveAppearance(global, INHERITED_APPEARANCE)).toMatchObject({ fontSize: 24, taskMarkerSize: 30 })
    expect(resolveAppearance(global, { ...INHERITED_APPEARANCE, fontSize: 18, taskMarkerSize: 0 }))
      .toMatchObject({ fontSize: 18, taskMarkerSize: 0 })
  })
  it('scales automatic markers and keeps independent sizes in the supported range', () => {
    expect(taskMarkerPixels(16, 0)).toBe(18)
    expect(taskMarkerPixels(24, 0)).toBe(27)
    expect(taskMarkerPixels(72, 0)).toBe(36)
    expect(taskMarkerPixels(9, 0)).toBe(12)
    expect(taskMarkerPixels(16, 30)).toBe(30)
  })
})
