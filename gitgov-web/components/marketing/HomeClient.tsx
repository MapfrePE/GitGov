'use client';

import React from 'react';
import { Hero, SectionHeader, FeatureCard, CTASection, RoleCards, FlowDiagram } from '@/components/marketing';
import { Container } from '@/components/layout';
import { SectionReveal } from '@/components/ui';
import { useTranslation } from '@/lib/i18n';
import {
    HiOutlineShieldCheck,
    HiOutlineEye,
    HiOutlineDocumentSearch,
    HiOutlineLightningBolt,
} from 'react-icons/hi';
import {
    FaUserTie,
    FaUserShield,
    FaUserCog,
} from 'react-icons/fa';

export function HomeClient() {
    const { t } = useTranslation();

    return (
        <>
            {/* ═══ Hero ═══ */}
            <Hero />

            {/* ═══ What is GitGov ═══ */}
            <section className="section-gap" id="what-is-gitgov">
                <Container>
                    <SectionHeader
                        badge={t('whatIs.badge') as string}
                        title={t('whatIs.title') as string}
                        titleAccent={t('whatIs.titleAccent') as string}
                        description={t('whatIs.description') as string}
                    />

                    <div className="grid md:grid-cols-2 gap-8 mt-12 items-stretch">
                        <SectionReveal className="h-full">
                            <div className="glass-card rounded-2xl p-8 h-full">
                                <div className="flex items-center gap-3 mb-4">
                                    <div className="w-10 h-10 rounded-xl bg-red-500/10 flex items-center justify-center">
                                        <HiOutlineEye className="text-red-400" size={22} />
                                    </div>
                                    <h3 className="text-lg font-semibold text-white">{t('whatIs.problemTitle')}</h3>
                                </div>
                                <p className="text-gray-400 leading-relaxed">{t('whatIs.problemDescription')}</p>
                            </div>
                        </SectionReveal>

                        <SectionReveal delay={0.15} className="h-full">
                            <div className="glass-card rounded-2xl p-8 glow-border h-full">
                                <div className="flex items-center gap-3 mb-4">
                                    <div className="w-10 h-10 rounded-xl bg-brand-500/10 flex items-center justify-center">
                                        <HiOutlineShieldCheck className="text-brand-400" size={22} />
                                    </div>
                                    <h3 className="text-lg font-semibold text-white">{t('whatIs.solutionTitle')}</h3>
                                </div>
                                <p className="text-gray-400 leading-relaxed">{t('whatIs.solutionDescription')}</p>
                            </div>
                        </SectionReveal>
                    </div>
                </Container>
            </section>

            {/* ═══ How It Works (Flow) ═══ */}
            <section className="section-gap bg-surface-100/30" id="how-it-works">
                <Container>
                    <SectionHeader
                        badge={t('howItWorks.badge') as string}
                        title={t('howItWorks.title') as string}
                        titleAccent={t('howItWorks.titleAccent') as string}
                        description={t('howItWorks.description') as string}
                    />
                    <FlowDiagram />
                </Container>
            </section>

            {/* ═══ Key Capabilities ═══ */}
            <section className="section-gap" id="capabilities">
                <Container>
                    <SectionHeader
                        badge={t('capabilities.badge') as string}
                        title={t('capabilities.title') as string}
                        titleAccent={t('capabilities.titleAccent') as string}
                        description={t('capabilities.description') as string}
                    />

                    <div className="grid sm:grid-cols-2 lg:grid-cols-4 gap-6">
                        <FeatureCard
                            icon={<HiOutlineShieldCheck size={24} />}
                            title={t('capabilities.governance.title') as string}
                            description={t('capabilities.governance.desc') as string}
                            index={0}
                        />
                        <FeatureCard
                            icon={<HiOutlineDocumentSearch size={24} />}
                            title={t('capabilities.audit.title') as string}
                            description={t('capabilities.audit.desc') as string}
                            index={1}
                        />
                        <FeatureCard
                            icon={<HiOutlineLightningBolt size={24} />}
                            title={t('capabilities.ci.title') as string}
                            description={t('capabilities.ci.desc') as string}
                            badge="Jenkins"
                            index={2}
                        />
                        <FeatureCard
                            icon={<HiOutlineEye size={24} />}
                            title={t('capabilities.ticket.title') as string}
                            description={t('capabilities.ticket.desc') as string}
                            badge={t('preview') as string}
                            index={3}
                        />
                    </div>
                </Container>
            </section>

            {/* ═══ Who It's For ═══ */}
            <section className="section-gap bg-surface-100/30" id="who-its-for">
                <Container>
                    <SectionHeader
                        badge={t('roles.badge') as string}
                        title={t('roles.title') as string}
                        titleAccent={t('roles.titleAccent') as string}
                        description={t('roles.description') as string}
                    />

                    <RoleCards
                        roles={[
                            {
                                icon: <FaUserShield className="text-brand-400" size={24} />,
                                role: t('roles.cto.role') as string,
                                painPoint: t('roles.cto.pain') as string,
                                solution: t('roles.cto.solution') as string,
                            },
                            {
                                icon: <FaUserTie className="text-accent-400" size={24} />,
                                role: t('roles.em.role') as string,
                                painPoint: t('roles.em.pain') as string,
                                solution: t('roles.em.solution') as string,
                            },
                            {
                                icon: <FaUserCog className="text-brand-300" size={24} />,
                                role: t('roles.devops.role') as string,
                                painPoint: t('roles.devops.pain') as string,
                                solution: t('roles.devops.solution') as string,
                            },
                        ]}
                    />
                </Container>
            </section>

            {/* ═══ CTA ═══ */}
            <CTASection
                title={t('cta.title') as string}
                titleAccent={t('cta.titleAccent') as string}
                description={t('cta.description') as string}
                primaryCta={{ label: t('cta.primary') as string, href: '/download' }}
                secondaryCta={{ label: t('cta.secondary') as string, href: '/docs' }}
            />
        </>
    );
}
