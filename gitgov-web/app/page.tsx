import React from 'react';
import { generatePageMetadata } from '@/lib/seo/metadata';
import { HomeClient } from '@/components/marketing/HomeClient';

export const metadata = generatePageMetadata({
    title: 'Git Governance Unified',
    description: 'GitGov provides full traceability from commit to CI to compliance. One platform for engineering teams that take operational evidence seriously.',
    path: '/',
});

export default function HomePage() {
    return <HomeClient />;
}
