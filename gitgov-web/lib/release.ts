import { promises as fs } from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { siteConfig } from '@/lib/config/site';

export interface ReleaseMetadata {
    version: string;
    downloadUrl: string;
    checksum: string;
    msiUrl: string | null;
    available: boolean;
}

export async function getReleaseMetadata(): Promise<ReleaseMetadata> {
    const msiUrl = siteConfig.downloadMsiUrl;

    // External URL mode: do not block the CTA by checking local filesystem.
    // Useful for Vercel/Next deployments where installers are hosted on CDN/S3.
    if (/^https?:\/\//i.test(siteConfig.downloadPath)) {
        return {
            version: siteConfig.version,
            downloadUrl: siteConfig.downloadPath,
            checksum: siteConfig.downloadChecksum,
            msiUrl,
            available: true,
        };
    }

    const relativePath = siteConfig.downloadPath.replace(/^\//, '');
    const absolutePath = path.join(process.cwd(), 'public', relativePath);

    try {
        const stat = await fs.stat(absolutePath);
        if (!stat.isFile()) {
            return {
                version: siteConfig.version,
                downloadUrl: siteConfig.downloadPath,
                checksum: siteConfig.downloadChecksum,
                msiUrl,
                available: false,
            };
        }

        const buffer = await fs.readFile(absolutePath);
        const checksum = `sha256:${createHash('sha256').update(buffer).digest('hex')}`;

        return {
            version: siteConfig.version,
            downloadUrl: siteConfig.downloadPath,
            checksum,
            msiUrl,
            available: true,
        };
    } catch {
        return {
            version: siteConfig.version,
            downloadUrl: siteConfig.downloadPath,
            checksum: siteConfig.downloadChecksum,
            msiUrl,
            available: false,
        };
    }
}
