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

const REMOTE_CHECK_TIMEOUT_MS = 5000;

function isHttpUrl(value: string): boolean {
    return /^https?:\/\//i.test(value);
}

async function checkRemoteAsset(url: string): Promise<boolean> {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), REMOTE_CHECK_TIMEOUT_MS);

    try {
        const headResponse = await fetch(url, {
            method: 'HEAD',
            redirect: 'follow',
            cache: 'no-store',
            signal: controller.signal,
        });

        if (headResponse.ok) {
            return true;
        }

        if (headResponse.status !== 405) {
            return false;
        }

        const getResponse = await fetch(url, {
            method: 'GET',
            headers: { Range: 'bytes=0-0' },
            redirect: 'follow',
            cache: 'no-store',
            signal: controller.signal,
        });

        return getResponse.ok || getResponse.status === 206;
    } catch {
        return false;
    } finally {
        clearTimeout(timer);
    }
}

export async function getReleaseMetadata(): Promise<ReleaseMetadata> {
    const msiUrl = siteConfig.downloadMsiUrl;

    // External URL mode: validate the remote asset exists before enabling CTA.
    if (isHttpUrl(siteConfig.downloadPath)) {
        const available = await checkRemoteAsset(siteConfig.downloadPath);

        return {
            version: siteConfig.version,
            downloadUrl: siteConfig.downloadPath,
            checksum: siteConfig.downloadChecksum,
            msiUrl,
            available,
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
