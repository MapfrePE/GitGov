'use client';

import React, { createContext, useContext, useState, useCallback, useEffect } from 'react';
import { type Locale, type TranslationKey, translations } from './translations';

interface I18nContextValue {
    locale: Locale;
    setLocale: (locale: Locale) => void;
    t: (key: TranslationKey) => string | string[];
}

const I18nContext = createContext<I18nContextValue>({
    locale: 'en',
    setLocale: () => { },
    t: (key) => {
        const entry = translations[key];
        if (!entry) return key;
        return (entry as Record<string, string | string[]>)['en'] ?? key;
    },
});

export function I18nProvider({ children }: { children: React.ReactNode }) {
    const [locale, setLocaleState] = useState<Locale>('en');

    // Persist locale preference
    useEffect(() => {
        const saved = localStorage.getItem('gitgov-locale') as Locale | null;
        if (saved && (saved === 'en' || saved === 'es')) {
            setLocaleState(saved);
        }
    }, []);

    const setLocale = useCallback((newLocale: Locale) => {
        setLocaleState(newLocale);
        localStorage.setItem('gitgov-locale', newLocale);
        document.documentElement.lang = newLocale;
    }, []);

    const t = useCallback(
        (key: TranslationKey): string | string[] => {
            const entry = translations[key];
            if (!entry) return key;
            return (entry as Record<string, string | string[]>)[locale] ?? (entry as Record<string, string | string[]>)['en'] ?? key;
        },
        [locale]
    );

    return (
        <I18nContext.Provider value={{ locale, setLocale, t }}>
            {children}
        </I18nContext.Provider>
    );
}

export function useTranslation() {
    return useContext(I18nContext);
}

export { type Locale };
