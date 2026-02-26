'use client';

import React from 'react';
import { Header, Footer, Preloader } from '@/components/layout';
import { I18nProvider } from '@/lib/i18n';

export function ClientLayout({ children }: { children: React.ReactNode }) {
    return (
        <I18nProvider>
            <Preloader />
            <Header />
            <main className="min-h-screen">{children}</main>
            <Footer />
        </I18nProvider>
    );
}
