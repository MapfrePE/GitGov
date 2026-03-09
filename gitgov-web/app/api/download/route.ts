import { NextRequest, NextResponse } from 'next/server';
import { getReleaseMetadata } from '@/lib/release';

/**
 * Download tracking API endpoint — Placeholder.
 *
 * In production, this should:
 * - Track download analytics
 * - Redirect to actual file / CDN
 * - Log user agent and platform
 *
 * Currently returns the download URL for the client to handle.
 */
export async function GET(request: NextRequest) {
    const platform = request.nextUrl.searchParams.get('platform') || 'windows';

    if (platform !== 'windows') {
        return NextResponse.json(
            { error: `Platform '${platform}' is not available yet.` },
            { status: 404 }
        );
    }

    const metadata = await getReleaseMetadata();
    const fileName = metadata.downloadUrl.split('/').pop() || 'desktop-installer.exe';

    if (!metadata.available) {
        return NextResponse.json(
            {
                error: 'Desktop installer is not available at the configured URL.',
                platform,
                download: {
                    url: metadata.downloadUrl,
                    fileName,
                    version: metadata.version,
                    checksum: metadata.checksum,
                    msiUrl: metadata.msiUrl,
                },
            },
            { status: 503 }
        );
    }

    // Log download intent (placeholder for analytics)
    console.log('[Download Intent]', {
        platform,
        fileName,
        userAgent: request.headers.get('user-agent'),
        timestamp: new Date().toISOString(),
    });

    return NextResponse.json({
        success: true,
        download: {
            url: metadata.downloadUrl,
            fileName,
            version: metadata.version,
            checksum: metadata.checksum,
            msiUrl: metadata.msiUrl,
        },
    });
}
