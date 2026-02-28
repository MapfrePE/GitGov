import React from 'react';
import { generatePageMetadata } from '@/lib/seo/metadata';
import { getReleaseMetadata } from '@/lib/release';
import { DownloadClient } from '@/components/download/DownloadClient';

export const metadata = generatePageMetadata({
    title: 'Download',
    description: 'Download GitGov Desktop for Windows. Capture Git operations and connect to your Control Plane.',
    path: '/download',
});

export default async function DownloadPage() {
    const release = await getReleaseMetadata();
    return <DownloadClient release={release} />;
}
