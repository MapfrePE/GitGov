import fs from 'fs';
import path from 'path';
import matter from 'gray-matter';
import { remark } from 'remark';
import html from 'remark-html';

import remarkGfm from 'remark-gfm';

const docsDirectory = path.join(process.cwd(), 'content', 'docs');

export interface DocPage {
    slug: string;
    title: string;
    description: string;
    order: number;
    contentHtml: string;
}

export interface DocMeta {
    slug: string;
    title: string;
    description: string;
    order: number;
}

export function getDocsSlugs(): string[] {
    try {
        const files = fs.readdirSync(docsDirectory);
        return files
            .filter((f) => f.endsWith('.md'))
            .map((f) => f.replace(/\.md$/, ''));
    } catch {
        return [];
    }
}

export function getDocsMeta(locale: string = 'en'): DocMeta[] {
    const slugs = getDocsSlugs();
    const docs = slugs.map((slug) => {
        let fullPath = path.join(docsDirectory, `${slug}.md`);

        // Try localized version first
        const localizedPath = path.join(docsDirectory, locale, `${slug}.md`);
        if (locale !== 'en' && fs.existsSync(localizedPath)) {
            fullPath = localizedPath;
        }

        const fileContents = fs.readFileSync(fullPath, 'utf-8');
        const { data } = matter(fileContents);
        return {
            slug,
            title: (data.title as string) || slug,
            description: (data.description as string) || '',
            order: (data.order as number) || 99,
        };
    });
    return docs.sort((a, b) => a.order - b.order);
}

export async function getDocBySlug(slug: string, locale: string = 'en'): Promise<DocPage | null> {
    try {
        let fullPath = path.join(docsDirectory, `${slug}.md`);

        // Try localized version first
        const localizedPath = path.join(docsDirectory, locale, `${slug}.md`);
        if (locale !== 'en' && fs.existsSync(localizedPath)) {
            fullPath = localizedPath;
        }

        const fileContents = fs.readFileSync(fullPath, 'utf-8');
        const { data, content } = matter(fileContents);

        const processedContent = await remark()
            .use(remarkGfm)
            .use(html)
            .process(content);
        const contentHtml = processedContent.toString();

        return {
            slug,
            title: (data.title as string) || slug,
            description: (data.description as string) || '',
            order: (data.order as number) || 99,
            contentHtml,
        };
    } catch {
        return null;
    }
}
