import { Metadata } from 'next';
import { siteConfig } from '@/lib/config/site';

interface PageSEOProps {
    title?: string;
    description?: string;
    path?: string;
    ogImage?: string;
}

export function generatePageMetadata({
    title,
    description,
    path = '',
    ogImage,
}: PageSEOProps = {}): Metadata {
    const pageTitle = title
        ? `${title} | ${siteConfig.name}`
        : `${siteConfig.name} — ${siteConfig.tagline}`;

    const pageDescription = description || siteConfig.description;
    const pageUrl = `${siteConfig.url}${path}`;
    const pageOgImage = ogImage || siteConfig.ogImage;

    return {
        title: pageTitle,
        description: pageDescription,
        metadataBase: new URL(siteConfig.url),
        alternates: {
            canonical: pageUrl,
        },
        openGraph: {
            title: pageTitle,
            description: pageDescription,
            url: pageUrl,
            siteName: siteConfig.name,
            images: [
                {
                    url: pageOgImage,
                    width: 1200,
                    height: 630,
                    alt: pageTitle,
                },
            ],
            locale: 'en_US',
            type: 'website',
        },
        twitter: {
            card: 'summary_large_image',
            title: pageTitle,
            description: pageDescription,
            images: [pageOgImage],
        },
        robots: {
            index: true,
            follow: true,
            googleBot: {
                index: true,
                follow: true,
                'max-video-preview': -1,
                'max-image-preview': 'large',
                'max-snippet': -1,
            },
        },
    };
}
