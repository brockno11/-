import type { AppSettings, NoteAppearance } from './types'

export const INHERITED_APPEARANCE: NoteAppearance = {
  fontFamily: null, fontSize: null, lineHeight: null, taskMarkerSize: null,
}

export function resolveAppearance(settings: AppSettings, appearance: NoteAppearance) {
  return {
    fontFamily: appearance.fontFamily ?? settings.defaultFontFamily,
    fontSize: appearance.fontSize ?? settings.defaultFontSize,
    lineHeight: appearance.lineHeight ?? settings.defaultLineHeight,
    taskMarkerSize: appearance.taskMarkerSize ?? settings.defaultTaskMarkerSize,
  }
}

export function taskMarkerPixels(fontSize: number, size: number) {
  return size === 0 ? Math.max(12, Math.min(36, fontSize * 1.125)) : Math.max(12, Math.min(36, size))
}
