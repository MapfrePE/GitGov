import React from 'react';
import { generatePageMetadata } from '@/lib/seo/metadata';
import { siteConfig } from '@/lib/config/site';
import { HomeClient } from '@/components/marketing/HomeClient';

export const metadata = generatePageMetadata({
    title: 'Git Governance Unified',
    description: 'GitGov provides full traceability from commit to CI to compliance. One platform for engineering teams that take operational evidence seriously.',
    path: '/',
});

const jsonLd = {
    '@context': 'https://schema.org',
    '@type': 'SoftwareApplication',
    name: siteConfig.name,
    description: siteConfig.description,
    url: siteConfig.url,
    applicationCategory: 'DeveloperApplication',
    operatingSystem: 'Windows',
    offers: {
        '@type': 'Offer',
        price: '0',
        priceCurrency: 'USD',
    },
};

export default function HomePage() {
    return (
        <>
            <script
                type="application/ld+json"
                dangerouslySetInnerHTML={{ __html: JSON.stringify(jsonLd) }}
            />
            <HomeClient />
        </>
    );
}
