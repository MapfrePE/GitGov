import type { Metadata, Viewport } from 'next';
import { generatePageMetadata } from '@/lib/seo/metadata';
import { ClientLayout } from './client-layout';
import './globals.css';

export const metadata: Metadata = {
    ...generatePageMetadata(),
    manifest: '/manifest.json',
    icons: {
        icon: '/favicon.ico',
    },
};

export const viewport: Viewport = {
    themeColor: '#0a0e1a',
    width: 'device-width',
    initialScale: 1,
};

export default function RootLayout({
    children,
}: {
    children: React.ReactNode;
}) {
    return (
        <html lang="en" className="dark" suppressHydrationWarning>
            <body className="min-h-[100dvh] bg-surface-300 text-white antialiased">
                <ClientLayout>{children}</ClientLayout>
            </body>
        </html>
    );
}
