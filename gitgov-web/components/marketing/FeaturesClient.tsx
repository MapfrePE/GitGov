'use client';

import React from 'react';
import { Container } from '@/components/layout';
import { SectionHeader, FeatureCard, CTASection } from '@/components/marketing';
import { useTranslation } from '@/lib/i18n';
import {
    HiOutlineShieldCheck,
    HiOutlineDocumentSearch,
    HiOutlineLightningBolt,
    HiOutlineEye,
    HiOutlineLockClosed,
    HiOutlineClipboardCheck,
    HiOutlinePuzzle,
    HiOutlineTrendingUp,
} from 'react-icons/hi';

export function FeaturesClient() {
    const { t } = useTranslation();

    const featureSections = [
        {
            badge: t('features.core.badge') as string,
            title: t('features.core.title') as string,
            titleAccent: t('features.core.titleAccent') as string,
            description: t('features.core.description') as string,
            features: [
                {
                    icon: <HiOutlineShieldCheck size={24} />,
                    title: t('features.commit.title') as string,
                    description: t('features.commit.desc') as string,
                },
                {
                    icon: <HiOutlineLockClosed size={24} />,
                    title: t('features.appendOnly.title') as string,
                    description: t('features.appendOnly.desc') as string,
                },
                {
                    icon: <HiOutlineClipboardCheck size={24} />,
                    title: t('features.policy.title') as string,
                    description: t('features.policy.desc') as string,
                    badge: t('advisory') as string,
                },
            ],
        },
        {
            badge: t('features.infra.badge') as string,
            title: t('features.infra.title') as string,
            titleAccent: t('features.infra.titleAccent') as string,
            description: t('features.infra.description') as string,
            features: [
                {
                    icon: <HiOutlineDocumentSearch size={24} />,
                    title: t('features.centralized.title') as string,
                    description: t('features.centralized.desc') as string,
                },
                {
                    icon: <HiOutlineEye size={24} />,
                    title: t('features.realtime.title') as string,
                    description: t('features.realtime.desc') as string,
                },
            ],
        },
        {
            badge: t('features.integrations.badge') as string,
            title: t('features.integrations.title') as string,
            titleAccent: t('features.integrations.titleAccent') as string,
            description: t('features.integrations.description') as string,
            features: [
                {
                    icon: <HiOutlineLightningBolt size={24} />,
                    title: t('features.jenkins.title') as string,
                    description: t('features.jenkins.desc') as string,
                    badge: 'Jenkins',
                },
                {
                    icon: <HiOutlinePuzzle size={24} />,
                    title: t('features.jira.title') as string,
                    description: t('features.jira.desc') as string,
                    badge: t('preview') as string,
                },
                {
                    icon: <HiOutlineTrendingUp size={24} />,
                    title: t('features.github.title') as string,
                    description: t('features.github.desc') as string,
                    badge: t('inProgress') as string,
                },
            ],
        },
    ];

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
                        badge={t('features.badge') as string}
                        title={t('features.title') as string}
                        titleAccent={t('features.titleAccent') as string}
                        description={t('features.description') as string}
                    />
                </Container>
            </section>

            {/* Feature Sections */}
            {featureSections.map((section, sectionIndex) => (
                <section
                    key={section.title}
                    className={`py-20 ${sectionIndex % 2 === 1 ? 'bg-surface-100/30' : ''}`}
                >
                    <Container>
                        <SectionHeader
                            badge={section.badge}
                            title={section.title}
                            titleAccent={section.titleAccent}
                            description={section.description}
                        />
                        <div className={`grid sm:grid-cols-2 ${section.features.length === 3 ? 'lg:grid-cols-3' : 'lg:grid-cols-2 max-w-4xl mx-auto'} gap-6`}>
                            {section.features.map((feature, i) => (
                                <FeatureCard
                                    key={feature.title}
                                    icon={feature.icon}
                                    title={feature.title}
                                    description={feature.description}
                                    badge={feature.badge}
                                    index={i}
                                />
                            ))}
                        </div>
                    </Container>
                </section>
            ))}

            {/* CTA */}
            <CTASection
                title={t('features.cta.title') as string}
                titleAccent={t('features.cta.titleAccent') as string}
                description={t('features.cta.desc') as string}
                primaryCta={{ label: t('features.cta.primary') as string, href: '/download' }}
                secondaryCta={{ label: t('features.cta.secondary') as string, href: '/docs' }}
            />
        </>
    );
}
