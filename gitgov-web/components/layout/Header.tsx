'use client';

import React, { useState, useEffect } from 'react';
import Link from 'next/link';
import { motion, AnimatePresence } from 'framer-motion';
import { HiOutlineMenuAlt3, HiOutlineX } from 'react-icons/hi';
import { siteConfig } from '@/lib/config/site';
import { useTranslation } from '@/lib/i18n';
import { Container } from './Container';
import { Button } from '@/components/ui/Button';

const navKeys: Record<string, string> = {
    '/features': 'nav.features',
    '/download': 'nav.download',
    '/docs': 'nav.docs',
    '/pricing': 'nav.pricing',
    '/contact': 'nav.contact',
};

export function Header() {
    const [scrolled, setScrolled] = useState(false);
    const [mobileMenuOpen, setMobileMenuOpen] = useState(false);
    const { locale, setLocale, t } = useTranslation();

    useEffect(() => {
        const handleScroll = () => setScrolled(window.scrollY > 20);
        window.addEventListener('scroll', handleScroll, { passive: true });
        return () => window.removeEventListener('scroll', handleScroll);
    }, []);

    useEffect(() => {
        if (mobileMenuOpen) {
            document.body.style.overflow = 'hidden';
        } else {
            document.body.style.overflow = '';
        }
        return () => { document.body.style.overflow = ''; };
    }, [mobileMenuOpen]);

    return (
        <>
            <motion.header
                className={`fixed top-0 left-0 right-0 z-50 transition-all duration-500 ${scrolled
                    ? 'bg-surface-300/80 backdrop-blur-xl border-b border-white/[0.06] shadow-lg shadow-black/20'
                    : 'bg-transparent'
                    }`}
                initial={{ y: -100 }}
                animate={{ y: 0 }}
                transition={{ duration: 0.6, ease: [0.25, 0.4, 0.25, 1] }}
            >
                <Container>
                    <nav className="flex items-center justify-between h-16 md:h-20" aria-label="Main navigation">
                        {/* Logo */}
                        <Link href="/" className="flex items-center gap-2.5 group" aria-label="GitGov Home">
                            <div className="relative w-8 h-8 md:w-9 md:h-9">
                                <div className="absolute inset-0 bg-brand-500 rounded-lg rotate-45 group-hover:rotate-[135deg] transition-transform duration-500" />
                                <div className="absolute inset-[3px] bg-surface-300 rounded-[5px] rotate-45 group-hover:rotate-[135deg] transition-transform duration-500" />
                                <div className="absolute inset-0 flex items-center justify-center">
                                    <svg width="16" height="16" viewBox="0 0 16 16" fill="none" className="relative z-10">
                                        <circle cx="8" cy="3" r="2" fill="#00e5da" />
                                        <circle cx="8" cy="13" r="2" fill="#00e5da" />
                                        <circle cx="13" cy="8" r="2" fill="#ffbb1a" />
                                        <line x1="8" y1="5" x2="8" y2="11" stroke="#00e5da" strokeWidth="1.5" />
                                        <line x1="9.5" y1="6" x2="11.5" y2="7.5" stroke="#ffbb1a" strokeWidth="1.5" />
                                    </svg>
                                </div>
                            </div>
                            <span className="text-lg md:text-xl font-bold tracking-tight">
                                <span className="text-white">Git</span>
                                <span className="text-brand-400">Gov</span>
                            </span>
                        </Link>

                        {/* Desktop Navigation */}
                        <div className="hidden md:flex items-center gap-1">
                            {siteConfig.nav.map((item) => (
                                <Link
                                    key={item.href}
                                    href={item.href}
                                    className="px-4 py-2 text-sm font-medium text-gray-400 hover:text-white transition-colors duration-300 rounded-lg hover:bg-white/5"
                                >
                                    {t(navKeys[item.href] as any) as string}
                                </Link>
                            ))}
                        </div>

                        {/* Desktop CTA + Language Flags */}
                        <div className="hidden md:flex items-center gap-3">
                            {/* Language Switcher */}
                            <div className="flex items-center gap-1 mr-2">
                                <button
                                    onClick={() => setLocale('en')}
                                    className={`w-7 h-5 rounded overflow-hidden border transition-all duration-200 ${locale === 'en' ? 'border-brand-500 shadow-glow scale-110' : 'border-white/10 opacity-50 hover:opacity-80'
                                        }`}
                                    aria-label="Switch to English"
                                    title="English"
                                >
                                    <svg viewBox="0 0 60 30" className="w-full h-full">
                                        <clipPath id="s"><path d="M0,0 v30 h60 v-30 z"/></clipPath>
                                        <clipPath id="t"><path d="M30,15 h30 v15 z v15 h-30 z h-30 v-15 z v-15 h30 z"/></clipPath>
                                        <g clipPath="url(#s)">
                                            <path d="M0,0 v30 h60 v-30 z" fill="#012169"/>
                                            <path d="M0,0 L60,30 M60,0 L0,30" stroke="#fff" strokeWidth="6"/>
                                            <path d="M0,0 L60,30 M60,0 L0,30" clipPath="url(#t)" stroke="#C8102E" strokeWidth="4"/>
                                            <path d="M30,0 v30 M0,15 h60" stroke="#fff" strokeWidth="10"/>
                                            <path d="M30,0 v30 M0,15 h60" stroke="#C8102E" strokeWidth="6"/>
                                        </g>
                                    </svg>
                                </button>
                                <button
                                    onClick={() => setLocale('es')}
                                    className={`w-7 h-5 rounded overflow-hidden border transition-all duration-200 ${locale === 'es' ? 'border-brand-500 shadow-glow scale-110' : 'border-white/10 opacity-50 hover:opacity-80'
                                        }`}
                                    aria-label="Cambiar a Español"
                                    title="Español"
                                >
                                    <svg viewBox="0 0 28 20" className="w-full h-full">
                                        <rect width="28" height="5" fill="#AA151B" />
                                        <rect y="5" width="28" height="10" fill="#F1BF00" />
                                        <rect y="15" width="28" height="5" fill="#AA151B" />
                                    </svg>
                                </button>
                            </div>

                            <Button variant="primary" size="sm" href="/download">
                                {t('nav.download') as string}
                            </Button>
                        </div>

                        {/* Mobile: Flags + Menu Button */}
                        <div className="md:hidden flex items-center gap-2">
                            <div className="flex items-center gap-1">
                                <button
                                    onClick={() => setLocale('en')}
                                    className={`w-6 h-4 rounded-sm overflow-hidden border transition-all ${locale === 'en' ? 'border-brand-500 scale-110' : 'border-white/10 opacity-50'
                                        }`}
                                    aria-label="English"
                                >
                                    <svg viewBox="0 0 60 30" className="w-full h-full">
                                        <clipPath id="s"><path d="M0,0 v30 h60 v-30 z"/></clipPath>
                                        <clipPath id="t"><path d="M30,15 h30 v15 z v15 h-30 z h-30 v-15 z v-15 h30 z"/></clipPath>
                                        <g clipPath="url(#s)">
                                            <path d="M0,0 v30 h60 v-30 z" fill="#012169"/>
                                            <path d="M0,0 L60,30 M60,0 L0,30" stroke="#fff" strokeWidth="6"/>
                                            <path d="M0,0 L60,30 M60,0 L0,30" clipPath="url(#t)" stroke="#C8102E" strokeWidth="4"/>
                                            <path d="M30,0 v30 M0,15 h60" stroke="#fff" strokeWidth="10"/>
                                            <path d="M30,0 v30 M0,15 h60" stroke="#C8102E" strokeWidth="6"/>
                                        </g>
                                    </svg>
                                </button>
                                <button
                                    onClick={() => setLocale('es')}
                                    className={`w-6 h-4 rounded-sm overflow-hidden border transition-all ${locale === 'es' ? 'border-brand-500 scale-110' : 'border-white/10 opacity-50'
                                        }`}
                                    aria-label="Español"
                                >
                                    <svg viewBox="0 0 28 20" className="w-full h-full">
                                        <rect width="28" height="5" fill="#AA151B" />
                                        <rect y="5" width="28" height="10" fill="#F1BF00" />
                                        <rect y="15" width="28" height="5" fill="#AA151B" />
                                    </svg>
                                </button>
                            </div>
                            <button
                                className="p-2 text-gray-400 hover:text-white transition-colors"
                                onClick={() => setMobileMenuOpen(!mobileMenuOpen)}
                                aria-label={mobileMenuOpen ? 'Close menu' : 'Open menu'}
                                aria-expanded={mobileMenuOpen}
                            >
                                {mobileMenuOpen ? <HiOutlineX size={24} /> : <HiOutlineMenuAlt3 size={24} />}
                            </button>
                        </div>
                    </nav>
                </Container>
            </motion.header>

            {/* Mobile Menu Overlay */}
            <AnimatePresence>
                {mobileMenuOpen && (
                    <motion.div
                        className="fixed inset-0 z-40 md:hidden"
                        initial={{ opacity: 0 }}
                        animate={{ opacity: 1 }}
                        exit={{ opacity: 0 }}
                        transition={{ duration: 0.3 }}
                    >
                        <div
                            className="absolute inset-0 bg-black/60 backdrop-blur-sm"
                            onClick={() => setMobileMenuOpen(false)}
                        />
                        <motion.div
                            className="absolute top-16 left-0 right-0 bg-surface-300/95 backdrop-blur-xl border-b border-white/[0.06]"
                            initial={{ opacity: 0, y: -20 }}
                            animate={{ opacity: 1, y: 0 }}
                            exit={{ opacity: 0, y: -20 }}
                            transition={{ duration: 0.3, ease: [0.25, 0.4, 0.25, 1] }}
                        >
                            <Container>
                                <div className="py-6 space-y-1">
                                    {siteConfig.nav.map((item, i) => (
                                        <motion.div
                                            key={item.href}
                                            initial={{ opacity: 0, x: -20 }}
                                            animate={{ opacity: 1, x: 0 }}
                                            transition={{ delay: i * 0.05 }}
                                        >
                                            <Link
                                                href={item.href}
                                                className="block px-4 py-3 text-base font-medium text-gray-300 hover:text-white hover:bg-white/5 rounded-xl transition-colors"
                                                onClick={() => setMobileMenuOpen(false)}
                                            >
                                                {t(navKeys[item.href] as any) as string}
                                            </Link>
                                        </motion.div>
                                    ))}
                                    <div className="pt-4 px-4">
                                        <Button variant="primary" size="md" href="/download" className="w-full">
                                            {t('hero.cta') as string}
                                        </Button>
                                    </div>
                                </div>
                            </Container>
                        </motion.div>
                    </motion.div>
                )}
            </AnimatePresence>
        </>
    );
}
