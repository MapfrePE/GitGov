'use client';

import React, { useState } from 'react';
import { Container } from '@/components/layout';
import { SectionHeader } from '@/components/marketing';
import { SectionReveal } from '@/components/ui';
import { HiOutlineMail, HiOutlineCheck, HiOutlineLightningBolt, HiOutlineOfficeBuilding, HiOutlineUser } from 'react-icons/hi';
import { useTranslation } from '@/lib/i18n';

interface PlanFeature {
    text: string;
    included: boolean;
}

interface Plan {
    name: string;
    badge?: string;
    price: string;
    priceNote: string;
    description: string;
    icon: React.ReactNode;
    features: PlanFeature[];
    cta: string;
    ctaHref: string;
    highlighted: boolean;
    gradientFrom: string;
    gradientTo: string;
}

export function PricingClient() {
    const { t } = useTranslation();
    const [hoveredPlan, setHoveredPlan] = useState<number | null>(null);

    const plans: Plan[] = [
        {
            name: t('pricing.plans.free.name') as string,
            price: t('pricing.plans.free.price') as string,
            priceNote: t('pricing.plans.free.priceNote') as string,
            description: t('pricing.plans.free.description') as string,
            icon: <HiOutlineUser size={22} />,
            highlighted: false,
            gradientFrom: 'rgba(0,229,218,0.08)',
            gradientTo: 'rgba(0,229,218,0.02)',
            cta: t('pricing.plans.free.cta') as string,
            ctaHref: '/contact',
            features: [
                { text: t('pricing.plans.free.f1') as string, included: true },
                { text: t('pricing.plans.free.f2') as string, included: true },
                { text: t('pricing.plans.free.f3') as string, included: true },
                { text: t('pricing.plans.free.f4') as string, included: false },
                { text: t('pricing.plans.free.f5') as string, included: false },
                { text: t('pricing.plans.free.f6') as string, included: false },
            ],
        },
        {
            name: t('pricing.plans.team.name') as string,
            badge: t('pricing.plans.team.badge') as string,
            price: t('pricing.plans.team.price') as string,
            priceNote: t('pricing.plans.team.priceNote') as string,
            description: t('pricing.plans.team.description') as string,
            icon: <HiOutlineLightningBolt size={22} />,
            highlighted: true,
            gradientFrom: 'rgba(0,229,218,0.18)',
            gradientTo: 'rgba(0,229,218,0.06)',
            cta: t('pricing.plans.team.cta') as string,
            ctaHref: '/contact',
            features: [
                { text: t('pricing.plans.team.f1') as string, included: true },
                { text: t('pricing.plans.team.f2') as string, included: true },
                { text: t('pricing.plans.team.f3') as string, included: true },
                { text: t('pricing.plans.team.f4') as string, included: true },
                { text: t('pricing.plans.team.f5') as string, included: true },
                { text: t('pricing.plans.team.f6') as string, included: false },
            ],
        },
        {
            name: t('pricing.plans.enterprise.name') as string,
            price: t('pricing.plans.enterprise.price') as string,
            priceNote: t('pricing.plans.enterprise.priceNote') as string,
            description: t('pricing.plans.enterprise.description') as string,
            icon: <HiOutlineOfficeBuilding size={22} />,
            highlighted: false,
            gradientFrom: 'rgba(0,229,218,0.06)',
            gradientTo: 'rgba(0,229,218,0.01)',
            cta: t('pricing.plans.enterprise.cta') as string,
            ctaHref: '/contact',
            features: [
                { text: t('pricing.plans.enterprise.f1') as string, included: true },
                { text: t('pricing.plans.enterprise.f2') as string, included: true },
                { text: t('pricing.plans.enterprise.f3') as string, included: true },
                { text: t('pricing.plans.enterprise.f4') as string, included: true },
                { text: t('pricing.plans.enterprise.f5') as string, included: true },
                { text: t('pricing.plans.enterprise.f6') as string, included: true },
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
                    {/* Glow orbs */}
                    <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[300px] bg-brand-500/5 rounded-full blur-3xl pointer-events-none" />
                </div>
                <Container>
                    <SectionHeader
                        badge={t('pricing.badge') as string}
                        title={t('pricing.title') as string}
                        titleAccent={t('pricing.titleAccent') as string}
                        description={t('pricing.descriptionNew') as string}
                    />
                </Container>
            </section>

            {/* Pricing Cards */}
            <section className="pb-32">
                <Container>
                    <SectionReveal>
                        <div className="grid md:grid-cols-3 gap-6 max-w-5xl mx-auto">
                            {plans.map((plan, i) => (
                                <div
                                    key={plan.name}
                                    onMouseEnter={() => setHoveredPlan(i)}
                                    onMouseLeave={() => setHoveredPlan(null)}
                                    className={`relative rounded-2xl p-px transition-all duration-300 ${
                                        plan.highlighted
                                            ? 'shadow-[0_0_40px_rgba(0,229,218,0.15)]'
                                            : hoveredPlan === i
                                            ? 'shadow-[0_0_20px_rgba(0,229,218,0.08)]'
                                            : ''
                                    }`}
                                    style={{
                                        background: plan.highlighted
                                            ? 'linear-gradient(135deg, rgba(0,229,218,0.5) 0%, rgba(0,229,218,0.1) 50%, rgba(0,229,218,0.05) 100%)'
                                            : hoveredPlan === i
                                            ? 'linear-gradient(135deg, rgba(0,229,218,0.2) 0%, rgba(0,229,218,0.05) 100%)'
                                            : 'linear-gradient(135deg, rgba(255,255,255,0.06) 0%, rgba(255,255,255,0.02) 100%)',
                                    }}
                                >
                                    <div
                                        className="rounded-2xl h-full flex flex-col p-7"
                                        style={{
                                            background: `linear-gradient(145deg, ${plan.gradientFrom}, ${plan.gradientTo}), #0d1117`,
                                        }}
                                    >
                                        {/* Card Header */}
                                        <div className="mb-6">
                                            <div className="flex items-start justify-between mb-4">
                                                <div
                                                    className={`w-10 h-10 rounded-xl flex items-center justify-center ${
                                                        plan.highlighted
                                                            ? 'bg-brand-500/20 text-brand-400'
                                                            : 'bg-white/5 text-gray-400'
                                                    }`}
                                                >
                                                    {plan.icon}
                                                </div>
                                                {plan.badge && (
                                                    <span className="text-xs font-semibold px-2.5 py-1 rounded-full bg-brand-500/20 text-brand-400 border border-brand-500/30">
                                                        {plan.badge}
                                                    </span>
                                                )}
                                            </div>

                                            <h3 className="text-lg font-bold text-white mb-1">{plan.name}</h3>
                                            <p className="text-sm text-gray-500 leading-relaxed">{plan.description}</p>
                                        </div>

                                        {/* Price */}
                                        <div className="mb-6 pb-6 border-b border-white/5">
                                            <div className="flex items-baseline gap-1">
                                                <span
                                                    className={`text-3xl font-bold ${
                                                        plan.highlighted ? 'text-brand-400' : 'text-white'
                                                    }`}
                                                >
                                                    {plan.price}
                                                </span>
                                            </div>
                                            <p className="text-xs text-gray-600 mt-1">{plan.priceNote}</p>
                                        </div>

                                        {/* Features */}
                                        <ul className="space-y-3 flex-1 mb-8">
                                            {plan.features.map((f, fi) => (
                                                <li key={fi} className="flex items-start gap-2.5">
                                                    <div
                                                        className={`mt-0.5 w-4 h-4 rounded-full flex items-center justify-center flex-shrink-0 ${
                                                            f.included
                                                                ? plan.highlighted
                                                                    ? 'bg-brand-500/20 text-brand-400'
                                                                    : 'bg-white/10 text-gray-400'
                                                                : 'bg-white/5 text-gray-700'
                                                        }`}
                                                    >
                                                        {f.included ? (
                                                            <HiOutlineCheck size={10} />
                                                        ) : (
                                                            <span className="w-1 h-0.5 bg-current rounded" />
                                                        )}
                                                    </div>
                                                    <span
                                                        className={`text-sm ${
                                                            f.included ? 'text-gray-300' : 'text-gray-600 line-through decoration-gray-700'
                                                        }`}
                                                    >
                                                        {f.text}
                                                    </span>
                                                </li>
                                            ))}
                                        </ul>

                                        {/* CTA */}
                                        <a
                                            href={plan.ctaHref}
                                            className={`flex items-center justify-center gap-2 w-full py-3 px-5 rounded-xl text-sm font-semibold transition-all duration-300 ${
                                                plan.highlighted
                                                    ? 'bg-brand-500 text-surface-300 hover:bg-brand-400 shadow-glow hover:shadow-glow-lg'
                                                    : 'bg-white/5 text-gray-300 border border-white/10 hover:bg-white/10'
                                            }`}
                                        >
                                            <HiOutlineMail size={15} />
                                            {plan.cta}
                                        </a>
                                    </div>
                                </div>
                            ))}
                        </div>

                        {/* Bottom note */}
                        <p className="text-center text-sm text-gray-600 mt-10">
                            {t('pricing.bottomNote') as string}
                        </p>
                    </SectionReveal>
                </Container>
            </section>
        </>
    );
}
