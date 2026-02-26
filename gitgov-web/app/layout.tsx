import type { Metadata } from 'next';
import { generatePageMetadata } from '@/lib/seo/metadata';
import { ClientLayout } from './client-layout';
import './globals.css';

export const metadata: Metadata = generatePageMetadata();

export default function RootLayout({
    children,
}: {
    children: React.ReactNode;
}) {
    return (
        <html lang="en" className="dark">
            <body className="min-h-screen bg-surface-300 text-white antialiased">
                <ClientLayout>{children}</ClientLayout>
            </body>
        </html>
    );
}
