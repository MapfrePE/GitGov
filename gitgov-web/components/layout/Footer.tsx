'use client';

import React from 'react';
import Link from 'next/link';
import Image from 'next/image';
import { siteConfig } from '@/lib/config/site';
import { useTranslation } from '@/lib/i18n';
import { Container } from './Container';

export function Footer() {
    const currentYear = new Date().getFullYear();
    const { t } = useTranslation();

    return (
        <footer className="relative border-t border-white/[0.06] bg-[#04060a]" role="contentinfo">
            <div className="absolute top-0 left-0 right-0 h-px bg-gradient-to-r from-transparent via-brand-500/30 to-transparent" />

            <Container>
                <div className="py-12 md:py-16">
                    <div className="grid grid-cols-2 md:grid-cols-4 gap-8 md:gap-12">
                        {/* Brand */}
                        <div className="col-span-2 md:col-span-1">
                            <Link href="/" className="inline-flex items-center gap-2 mb-4">
                                <Image
                                    src="/logo.png"
                                    alt="GitGov"
                                    width={80}
                                    height={80}
                                    className="w-20 h-20 object-contain"
                                />
                                <span className="text-lg font-bold">
                                    <span className="text-white">Git</span>
                                    <span className="text-brand-400">Gov</span>
                                </span>
                            </Link>
                            <p className="text-sm text-gray-500 leading-relaxed max-w-xs">
                                {siteConfig.description}
                            </p>
                        </div>

                        {/* Product */}
                        <div>
                            <h3 className="text-sm font-semibold text-white mb-4 tracking-wide uppercase">{t('footer.product') as string}</h3>
                            <ul className="space-y-2.5" role="list">
                                {siteConfig.footer.product.map((link) => (
                                    <li key={link.href}>
                                        <Link href={link.href} className="text-sm text-gray-500 hover:text-white transition-colors duration-300">
                                            {t(`nav.${link.label.toLowerCase()}` as any)}
                                        </Link>
                                    </li>
                                ))}
                            </ul>
                        </div>

                        {/* Resources */}
                        <div>
                            <h3 className="text-sm font-semibold text-white mb-4 tracking-wide uppercase">{t('footer.resources') as string}</h3>
                            <ul className="space-y-2.5" role="list">
                                {siteConfig.footer.resources.map((link) => (
                                    <li key={link.href}>
                                        <Link href={link.href} className="text-sm text-gray-500 hover:text-white transition-colors duration-300">
                                            {t(`footer.resources.${link.label.replace(/\s+/g, '').toLowerCase()}` as any)}
                                        </Link>
                                    </li>
                                ))}
                            </ul>
                        </div>

                        {/* Company */}
                        <div>
                            <h3 className="text-sm font-semibold text-white mb-4 tracking-wide uppercase">{t('footer.company') as string}</h3>
                            <ul className="space-y-2.5" role="list">
                                {siteConfig.footer.company.map((link) => (
                                    <li key={link.href}>
                                        {link.href.startsWith('http') ? (
                                            <a href={link.href} target="_blank" rel="noopener noreferrer" className="text-sm text-gray-500 hover:text-white transition-colors duration-300">
                                                {t(`nav.${link.label.toLowerCase()}` as any)}
                                            </a>
                                        ) : (
                                            <Link href={link.href} className="text-sm text-gray-500 hover:text-white transition-colors duration-300">
                                                {t(`nav.${link.label.toLowerCase()}` as any)}
                                            </Link>
                                        )}
                                    </li>
                                ))}
                            </ul>
                        </div>
                    </div>

                    <div className="mt-12 pt-8 border-t border-white/[0.04] flex flex-col sm:flex-row items-center justify-between gap-4">
                        <p className="text-xs text-gray-600">
                            © {currentYear} {siteConfig.name}. {t('footer.rights') as string}
                        </p>
                        <p className="text-xs text-gray-600">
                            {t('footer.tagline') as string}
                        </p>
                    </div>
                </div>
            </Container>
        </footer>
    );
}
