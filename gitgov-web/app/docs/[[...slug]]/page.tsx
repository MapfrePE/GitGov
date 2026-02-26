import React from 'react';
import { notFound } from 'next/navigation';
import { generatePageMetadata } from '@/lib/seo/metadata';
import { getDocBySlug, getDocsMeta } from '@/lib/content/docs';
import { DocsClient } from '@/components/docs/DocsClient';

interface DocsPageProps {
    params: { slug?: string[] };
}

export async function generateMetadata({ params }: DocsPageProps) {
    const slug = params.slug?.[0] || 'introduction';
    const doc = await getDocBySlug(slug, 'en');

    if (!doc) {
        return generatePageMetadata({
            title: 'Documentation',
            description: 'GitGov documentation and guides.',
            path: '/docs',
        });
    }

    return generatePageMetadata({
        title: doc.title,
        description: doc.description,
        path: `/docs/${slug}`,
    });
}

export default async function DocsPage({ params }: DocsPageProps) {
    const slug = params.slug?.[0] || 'introduction';

    // Fetch both versions for dynamic client-side switching
    const docEn = await getDocBySlug(slug, 'en');
    const docEs = await getDocBySlug(slug, 'es');
    const allDocsEn = getDocsMeta('en');
    const allDocsEs = getDocsMeta('es');

    if (!docEn) {
        notFound();
    }

    return (
        <DocsClient
            slug={slug}
            docs={{ en: docEn, es: docEs || docEn }}
            allDocs={{ en: allDocsEn, es: allDocsEs }}
        />
    );
}
