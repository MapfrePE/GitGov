import type { Metadata, Viewport } from 'next';
import { generatePageMetadata } from '@/lib/seo/metadata';
import { ClientLayout } from './client-layout';
import './globals.css';

export const metadata: Metadata = {
    ...generatePageMetadata(),
    manifest: '/manifest.json',
    icons: {
        icon: '/logo.png',
        apple: '/logo.png',
    },
};

export const viewport: Viewport = {
    themeColor: '#090909',
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
            <head>
                <link rel="preload" as="image" href="/fox.png" fetchPriority="high" />
            </head>
            <body className="min-h-[100dvh] bg-surface-300 text-white antialiased">
                <ClientLayout>{children}</ClientLayout>
            </body>
        </html>
    );
}
