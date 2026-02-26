import React from 'react';
import { promises as fs } from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { generatePageMetadata } from '@/lib/seo/metadata';
import { siteConfig } from '@/lib/config/site';
import { DownloadClient } from '@/components/download/DownloadClient';

export const metadata = generatePageMetadata({
    title: 'Download',
    description: 'Download GitGov Desktop for Windows. Capture Git operations and connect to your Control Plane.',
    path: '/download',
});

async function getWindowsReleaseInfo() {
    const relativePath = siteConfig.downloadPath.replace(/^\//, '');
    const absolutePath = path.join(process.cwd(), 'public', relativePath);

    try {
        const stat = await fs.stat(absolutePath);
        if (!stat.isFile()) {
            return {
                available: false,
                checksum: siteConfig.downloadChecksum,
            };
        }

        const buffer = await fs.readFile(absolutePath);
        const checksum = createHash('sha256').update(buffer).digest('hex');

        return {
            available: true,
            checksum: `sha256:${checksum}`,
        };
    } catch {
        return {
            available: false,
            checksum: siteConfig.downloadChecksum,
        };
    }
}

export default async function DownloadPage() {
    const windowsRelease = await getWindowsReleaseInfo();

    return <DownloadClient windowsRelease={windowsRelease} />;
}
