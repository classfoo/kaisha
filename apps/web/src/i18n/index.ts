import en from './en.json'
import zh from './zh.json'
import ja from './ja.json'

export type Locale = 'en' | 'zh' | 'ja'

const dict = { en, zh, ja } as const

export function resolveLocale(raw: string | null | undefined): Locale {
  if (raw === 'zh' || raw === 'ja' || raw === 'en') return raw
  if (raw?.startsWith('zh')) return 'zh'
  if (raw?.startsWith('ja')) return 'ja'
  return 'en'
}

export function t(locale: Locale, key: string): string {
  const path = key.split('.')
  let cur: unknown = dict[locale]
  for (const p of path) {
    if (typeof cur !== 'object' || cur === null || !(p in cur)) {
      cur = undefined
      break
    }
    cur = (cur as Record<string, unknown>)[p]
  }
  if (typeof cur === 'string') return cur

  // fallback to english
  cur = dict.en
  for (const p of path) {
    if (typeof cur !== 'object' || cur === null || !(p in cur)) return key
    cur = (cur as Record<string, unknown>)[p]
  }
  return typeof cur === 'string' ? cur : key
}
