'use client';

import React from 'react';
import { Container } from '@/components/layout';
import { SectionHeader } from '@/components/marketing';
import { Badge, SectionReveal } from '@/components/ui';
import { HiOutlineMail } from 'react-icons/hi';
import { useTranslation } from '@/lib/i18n';

export function PricingClient() {
    const { t } = useTranslation();

    const features = t('pricing.features' as any) as string[];

    return (
        <>
            {/* Hero */}
            <section className="pt-32 md:pt-40 pb-16 relative overflow-hidden">
                <div className="absolute inset-0">
                    <div
                        className="absolute inset-0 opacity-[0.03]"
                        style={{
                            backgroundImage: `linear-gradient(rgba(0,229,218,0.2) 1px, transparent 1px), linear-gradient(90deg, rgba(0,229,218,0.2) 1px, transparent 1px)`,
                            backgroundSize: '40px 40px',
                        }}
                    />
                </div>
                <Container>
                    <SectionHeader
                        badge={t('pricing.badge') as string}
                        title={t('pricing.title') as string}
                        titleAccent={t('pricing.titleAccent') as string}
                        description={t('pricing.description') as string}
                    />
                </Container>
            </section>

            {/* Coming Soon */}
            <section className="pb-32">
                <Container size="narrow">
                    <SectionReveal>
                        <div className="glass-card rounded-2xl p-12 md:p-16 text-center max-w-2xl mx-auto glow-border">
                            <Badge variant="accent" size="md" className="mb-6">{t('pricing.comingSoon')}</Badge>

                            <h3 className="text-2xl md:text-3xl font-bold text-white mb-4">
                                {t('pricing.underDev')}
                            </h3>

                            <p className="text-gray-400 leading-relaxed mb-8 max-w-lg mx-auto">
                                {t('pricing.underDevDesc')}
                            </p>

                            {/* What's included preview */}
                            <div className="grid sm:grid-cols-2 gap-4 text-left mb-10">
                                {Array.isArray(features) && features.map((feature) => (
                                    <div key={feature} className="flex items-center gap-2 text-sm text-gray-300">
                                        <span className="w-1.5 h-1.5 rounded-full bg-brand-500 flex-shrink-0" />
                                        {feature}
                                    </div>
                                ))}
                            </div>

                            <a
                                href="/contact"
                                className="inline-flex items-center gap-2 px-8 py-3 rounded-xl bg-brand-500 text-surface-300 font-semibold hover:bg-brand-400 transition-colors duration-300 shadow-glow hover:shadow-glow-lg"
                            >
                                <HiOutlineMail size={18} />
                                {t('pricing.contactBtn')}
                            </a>
                        </div>
                    </SectionReveal>
                </Container>
            </section>
        </>
    );
}
