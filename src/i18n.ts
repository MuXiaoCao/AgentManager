import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import LanguageDetector from 'i18next-browser-languagedetector'
import en from './locales/en.json'
import zh from './locales/zh.json'

export const SUPPORTED_LANGUAGES = ['en', 'zh'] as const
export type Language = (typeof SUPPORTED_LANGUAGES)[number]

const LS_KEY = 'agent-manager:lang'

i18n
  .use(LanguageDetector)
  .use(initReactI18next)
  .init({
    resources: {
      en: { translation: en },
      zh: { translation: zh },
    },
    fallbackLng: 'en',
    supportedLngs: SUPPORTED_LANGUAGES,
    interpolation: { escapeValue: false },
    detection: {
      order: ['localStorage', 'navigator'],
      lookupLocalStorage: LS_KEY,
      caches: ['localStorage'],
    },
  })

export function toggleLanguage(): Language {
  const next: Language = i18n.language?.startsWith('zh') ? 'en' : 'zh'
  i18n.changeLanguage(next)
  try {
    localStorage.setItem(LS_KEY, next)
  } catch {
    /* ignore */
  }
  return next
}

export function currentLanguage(): Language {
  return i18n.language?.startsWith('zh') ? 'zh' : 'en'
}

export default i18n
