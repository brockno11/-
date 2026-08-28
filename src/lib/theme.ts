export type ThemeId = 'default-light' | 'system' | 'dark'

export interface ThemeTokens {
  canvas: string
  surface: string
  surfaceHover: string
  border: string
  text: string
  textMuted: string
  accent: string
  danger: string
}

export interface ThemeDefinition {
  id: ThemeId
  label: string
  colorScheme: 'light' | 'dark' | 'system'
  tokens: ThemeTokens
}

export const DEFAULT_LIGHT_TOKENS: ThemeTokens = {
  canvas: '#FFFFFF',
  surface: '#F7F8FA',
  surfaceHover: '#F2F4F7',
  border: '#E5E7EB',
  text: '#18181B',
  textMuted: '#6B7280',
  accent: '#52657D',
  danger: '#C2413A',
}

export const THEMES: ThemeDefinition[] = [
  { id: 'default-light', label: '默认白色', colorScheme: 'light', tokens: DEFAULT_LIGHT_TOKENS },
  { id: 'system', label: '跟随系统', colorScheme: 'system', tokens: DEFAULT_LIGHT_TOKENS },
  {
    id: 'dark', label: '深色', colorScheme: 'dark',
    tokens: {
      canvas: '#18181B', surface: '#222226', surfaceHover: '#2B2B31', border: '#35353C',
      text: '#F4F4F5', textMuted: '#A1A1AA', accent: '#93A4BC', danger: '#F08A82',
    },
  },
]

export function normalizeThemeId(value: unknown): ThemeId {
  if (value === 'dark') return 'dark'
  if (value === 'system') return 'system'
  return 'default-light'
}

export function themeClass(themeId: ThemeId): string {
  return themeId === 'system' ? '' : themeId === 'default-light' ? 'theme-light' : 'theme-dark'
}
