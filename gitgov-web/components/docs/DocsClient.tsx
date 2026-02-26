'use client';

import React from 'react';
import Link from 'next/link';
import { Container } from '@/components/layout';
import { motion, AnimatePresence } from 'framer-motion';
import { HiOutlineChevronRight, HiOutlineDocumentText, HiOutlineTranslate } from 'react-icons/hi';
import { useTranslation } from '@/lib/i18n';
import { Button } from '@/components/ui';

interface DocData {
    title: string;
    contentHtml: string;
}

interface DocMeta {
    slug: string;
    title: string;
}

interface DocsClientProps {
    slug: string;
    docs: {
        en: DocData;
        es: DocData;
    };
    allDocs: {
        en: DocMeta[];
        es: DocMeta[];
    };
}

export function DocsClient({ slug, docs, allDocs }: DocsClientProps) {
    const { locale, t } = useTranslation();

    const currentDoc = locale === 'es' ? docs.es : docs.en;
    const currentMenu = locale === 'es' ? allDocs.es : allDocs.en;

    return (
        <section className="pt-28 md:pt-36 pb-20 relative min-h-screen">
            {/* Background elements for depth */}
            <div className="absolute top-0 right-0 w-[400px] h-[400px] bg-brand-500/5 blur-[120px] rounded-full -z-10 pointer-events-none" />
            <div className="absolute bottom-0 left-0 w-[300px] h-[300px] bg-accent-500/5 blur-[100px] rounded-full -z-10 pointer-events-none" />

            <Container size="wide">
                <div className="flex flex-col lg:flex-row gap-12 lg:gap-16">
                    {/* Sidebar Navigation */}
                    <aside className="lg:w-72 flex-shrink-0">
                        <div className="lg:sticky lg:top-32">
                            <div className="flex items-center gap-2 mb-6 pl-4">
                                <div className="w-1.5 h-6 bg-brand-500 rounded-full" />
                                <h3 className="text-sm font-bold tracking-wider uppercase text-white/90">
                                    {t('docs.title')}
                                </h3>
                            </div>

                            <nav aria-label="Documentation navigation">
                                <ul className="space-y-1" role="list">
                                    {currentMenu.map((d) => (
                                        <li key={d.slug}>
                                            <Link
                                                href={`/docs/${d.slug}`}
                                                className={`
                                                    group flex items-center justify-between px-4 py-2.5 text-sm rounded-xl transition-all duration-300
                                                    ${d.slug === slug
                                                        ? 'bg-gradient-to-r from-brand-500/15 to-transparent text-brand-400 font-semibold border-l-2 border-brand-500 shadow-sm'
                                                        : 'text-gray-400 hover:text-white hover:bg-white/5'
                                                    }
                                                `}
                                            >
                                                <div className="flex items-center gap-2.5">
                                                    <HiOutlineDocumentText size={18} className={`flex-shrink-0 transition-colors ${d.slug === slug ? 'text-brand-400' : 'text-gray-500 group-hover:text-gray-300'}`} />
                                                    <span>{d.title}</span>
                                                </div>
                                                <AnimatePresence>
                                                    {d.slug === slug && (
                                                        <motion.div
                                                            layoutId="active-indicator"
                                                            initial={{ opacity: 0, scale: 0 }}
                                                            animate={{ opacity: 1, scale: 1 }}
                                                            exit={{ opacity: 0, scale: 0 }}
                                                            className="w-1.5 h-1.5 rounded-full bg-brand-400 shadow-[0_0_8px_rgba(0,229,218,0.4)]"
                                                        />
                                                    )}
                                                </AnimatePresence>
                                            </Link>
                                        </li>
                                    ))}
                                </ul>
                            </nav>

                            {/* Help Box */}
                            <div className="mt-12 p-5 rounded-2xl bg-surface-100/50 border border-white/5 glow-border transition-all">
                                <h4 className="text-white text-sm font-semibold mb-2">{t('contact.title')}</h4>
                                <p className="text-gray-500 text-xs leading-relaxed mb-4">
                                    {t('contact.description')}
                                </p>
                                <Link
                                    href="/contact"
                                    className="text-xs font-bold text-brand-400 hover:text-brand-300 transition-colors flex items-center gap-1 group"
                                >
                                    {t('contact.form.send')}
                                    <HiOutlineChevronRight size={14} className="group-hover:translate-x-0.5 transition-transform" />
                                </Link>
                            </div>
                        </div>
                    </aside>

                    {/* Content */}
                    <article className="flex-1 min-w-0">
                        {/* Breadcrumb - Clean & Subtle */}
                        <div className="flex items-center gap-2 text-xs font-medium uppercase tracking-widest text-gray-500 mb-10">
                            <Link href="/docs" className="hover:text-white transition-colors">{t('docs.title')}</Link>
                            <span className="opacity-30">/</span>
                            <span className="text-brand-400/80">{currentDoc.title}</span>
                        </div>

                        {/* Title Section */}
                        <header className="mb-12">
                            <h1 className="text-4xl md:text-5xl font-extrabold text-white tracking-tight mb-4 leading-tight">
                                {currentDoc.title}
                            </h1>
                            <div className="h-1.5 w-20 bg-brand-500 rounded-full" />
                        </header>

                        {/* Rendered markdown with Enhanced Typography */}
                        <div
                            className="prose prose-invert prose-lg max-w-none doc-content
                                prose-headings:text-white prose-headings:font-bold prose-headings:tracking-tight
                                prose-h2:text-2xl prose-h2:mt-16 prose-h2:mb-8 prose-h2:pb-4 prose-h2:border-b prose-h2:border-white/[0.08]
                                prose-h3:text-xl prose-h3:mt-12 prose-h3:mb-6
                                prose-p:text-gray-400 prose-p:leading-[1.9] prose-p:mb-8
                                prose-li:text-gray-400 prose-li:leading-relaxed prose-li:mb-4
                                prose-strong:text-white prose-strong:font-bold
                                prose-blockquote:border-brand-500 prose-blockquote:bg-brand-500/5 prose-blockquote:py-4 prose-blockquote:px-6 prose-blockquote:rounded-r-2xl prose-blockquote:text-gray-300 prose-blockquote:italic prose-blockquote:my-10
                                prose-code:text-brand-300 prose-code:bg-brand-500/10 prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded-lg prose-code:font-mono prose-code:text-sm prose-code:before:content-none prose-code:after:content-none
                                prose-pre:bg-surface-100 prose-pre:border prose-pre:border-white/[0.08] prose-pre:rounded-2xl prose-pre:p-8 prose-pre:shadow-2xl prose-pre:shadow-black/40 prose-pre:my-10
                                prose-a:text-brand-400 prose-a:font-semibold prose-a:no-underline hover:prose-a:text-brand-300 prose-a:transition-all prose-a:decoration-brand-500/30 prose-a:underline-offset-4 hover:prose-a:underline
                                prose-img:rounded-3xl prose-img:shadow-2xl prose-img:my-12 prose-img:border prose-img:border-white/5
                                
                                prose-table:my-12 prose-table:overflow-hidden prose-table:rounded-2xl prose-table:border prose-table:border-white/[0.08] prose-table:bg-white/[0.02]
                                prose-thead:bg-white/[0.05] prose-thead:border-b prose-thead:border-white/[0.08]
                                prose-th:text-brand-400 prose-th:font-bold prose-th:p-5 prose-th:text-left prose-th:uppercase prose-th:tracking-wider prose-th:text-xs
                                prose-td:p-5 prose-td:text-gray-400 prose-td:border-b prose-td:border-white/[0.04] prose-td:text-sm
                                prose-tr:last:border-none hover:prose-tr:bg-white/[0.02] prose-tr:transition-colors
                            "
                            dangerouslySetInnerHTML={{ __html: currentDoc.contentHtml }}
                        />

                        {/* Navigation Footer */}
                        <div className="mt-20 pt-10 border-t border-white/5 flex items-center justify-between">
                            <div className="flex flex-col gap-1">
                                <span className="text-[10px] uppercase tracking-tighter text-gray-500 font-bold">{t('footer.rights') as string}</span>
                                <span className="text-xs text-gray-400">© {new Date().getFullYear()} GitGov</span>
                            </div>
                            <Button variant="ghost" size="sm" onClick={() => window.scrollTo({ top: 0, behavior: 'smooth' })} aria-label="Back to top">
                                Back to top
                            </Button>
                        </div>
                    </article>
                </div>
            </Container>
        </section>
    );
}
