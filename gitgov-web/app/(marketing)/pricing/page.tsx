import React from 'react';
import { generatePageMetadata } from '@/lib/seo/metadata';
import { PricingClient } from '@/components/marketing/PricingClient';

export const metadata = generatePageMetadata({
    title: 'Pricing',
    description: 'GitGov pricing and plans. Transparent governance for teams of all sizes.',
    path: '/pricing',
});

export default function PricingPage() {
    return <PricingClient />;
}
