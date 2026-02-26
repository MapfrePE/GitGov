import React from 'react';
import { generatePageMetadata } from '@/lib/seo/metadata';
import { FeaturesClient } from '@/components/marketing/FeaturesClient';

export const metadata = generatePageMetadata({
    title: 'Features',
    description: 'Explore GitGov features: Git governance, audit trails, CI traceability, ticket coverage, and policy enforcement.',
    path: '/features',
});

export default function FeaturesPage() {
    return <FeaturesClient />;
}
