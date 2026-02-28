import React from 'react';
import { generatePageMetadata } from '@/lib/seo/metadata';
import { PrivacyClient } from '@/components/marketing/PrivacyClient';

export const metadata = generatePageMetadata({
    title: 'Privacy Policy',
    description: 'How GitGov collects, processes, and protects operational metadata. No source code ever leaves your workstation.',
    path: '/privacy',
});

export default function PrivacyPage() {
    return <PrivacyClient />;
}
