import React from 'react';
import { generatePageMetadata } from '@/lib/seo/metadata';
import { ContactClient } from '@/components/marketing/ContactClient';

export const metadata = generatePageMetadata({
    title: 'Contact',
    description: 'Get in touch with the GitGov team for enterprise deployment, support, or questions.',
    path: '/contact',
});

export default function ContactPage() {
    return <ContactClient />;
}
