'use client';

import React from 'react';
import { motion } from 'framer-motion';
import { Container } from '@/components/layout';
import { Button } from '@/components/ui';
import { HiOutlineHome, HiOutlineDocumentSearch } from 'react-icons/hi';
import { useTranslation } from '@/lib/i18n';

export function NotFoundClient() {
    const { t } = useTranslation();

    return (
        <section className="min-h-screen flex items-center relative overflow-hidden">
            {/* Background */}
            <div className="absolute inset-0">
                <div
                    className="absolute inset-0 opacity-[0.03]"
                    style={{
                        backgroundImage: `linear-gradient(rgba(0,229,218,0.2) 1px, transparent 1px), linear-gradient(90deg, rgba(0,229,218,0.2) 1px, transparent 1px)`,
                        backgroundSize: '40px 40px',
                    }}
                />
                <div
                    className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full blur-[150px]"
                    style={{
                        background: 'radial-gradient(circle, rgba(0, 229, 218, 0.06) 0%, transparent 70%)',
                    }}
                />
            </div>

            <Container>
                <div className="text-center max-w-xl mx-auto">
                    {/* 404 number */}
                    <motion.div
                        initial={{ opacity: 0, scale: 0.8 }}
                        animate={{ opacity: 1, scale: 1 }}
                        transition={{ duration: 0.5 }}
                    >
                        <span className="text-[8rem] md:text-[12rem] font-bold leading-none gradient-text opacity-20">
                            404
                        </span>
                    </motion.div>

                    {/* Message */}
                    <motion.div
                        initial={{ opacity: 0, y: 20 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ delay: 0.2, duration: 0.5 }}
                        className="-mt-8 md:-mt-12"
                    >
                        <h1 className="text-2xl md:text-3xl font-bold text-white mb-4">
                            {t('404.title')}
                        </h1>
                        <p className="text-gray-400 mb-8">
                            {t('404.description')}
                        </p>
                    </motion.div>

                    {/* Actions */}
                    <motion.div
                        className="flex flex-col sm:flex-row items-center justify-center gap-4"
                        initial={{ opacity: 0, y: 20 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ delay: 0.4, duration: 0.5 }}
                    >
                        <Button
                            variant="primary"
                            href="/"
                            icon={<HiOutlineHome size={18} />}
                        >
                            {t('404.home')}
                        </Button>
                        <Button
                            variant="outline"
                            href="/docs"
                            icon={<HiOutlineDocumentSearch size={18} />}
                        >
                            {t('404.docs')}
                        </Button>
                    </motion.div>
                </div>
            </Container>
        </section>
    );
}
