import { NextRequest, NextResponse } from 'next/server';

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

    // Placeholder download paths
    const downloads: Record<string, { url: string; fileName: string; version: string }> = {
        windows: {
            url: '/downloads/GitGov_0.1.0_x64-setup.exe',
            fileName: 'GitGov_0.1.0_x64-setup.exe',
            version: '0.1.0',
        },
    };

    const download = downloads[platform];

    if (!download) {
        return NextResponse.json(
            { error: `Platform '${platform}' is not available yet.` },
            { status: 404 }
        );
    }

    // Log download intent (placeholder for analytics)
    console.log('[Download Intent]', {
        platform,
        fileName: download.fileName,
        userAgent: request.headers.get('user-agent'),
        timestamp: new Date().toISOString(),
    });

    return NextResponse.json({
        success: true,
        download,
    });
}
