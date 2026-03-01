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
            gradientFrom: 'rgba(255,255,255,0.03)',
            gradientTo: 'rgba(255,255,255,0.01)',
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
            gradientFrom: 'rgba(249,115,22,0.1)',
            gradientTo: 'rgba(249,115,22,0.02)',
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
            gradientFrom: 'rgba(255,255,255,0.03)',
            gradientTo: 'rgba(255,255,255,0.01)',
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
                            backgroundImage: `linear-gradient(rgba(249,115,22,0.2) 1px, transparent 1px), linear-gradient(90deg, rgba(249,115,22,0.2) 1px, transparent 1px)`,
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
                        <div className="grid md:grid-cols-3 gap-8 max-w-6xl mx-auto">
                            {plans.map((plan, i) => (
                                <div
                                    key={plan.name}
                                    onMouseEnter={() => setHoveredPlan(i)}
                                    onMouseLeave={() => setHoveredPlan(null)}
                                    className={`relative rounded-[2rem] p-[1px] transition-all duration-500 group overflow-hidden ${plan.highlighted
                                            ? 'shadow-[0_0_50px_rgba(249,115,22,0.1)] hover:shadow-[0_0_70px_rgba(249,115,22,0.15)] -mt-4 mb-4'
                                            : 'hover:shadow-2xl'
                                        }`}
                                >
                                    {/* Animated Border Gradient Layer */}
                                    <div
                                        className="absolute inset-0 opacity-100 z-0 transition-opacity duration-500"
                                        style={{
                                            background: plan.highlighted
                                                ? 'linear-gradient(135deg, rgba(249,115,22,0.5) 0%, rgba(249,115,22,0.1) 50%, rgba(249,115,22,0.05) 100%)'
                                                : hoveredPlan === i
                                                    ? 'linear-gradient(135deg, rgba(255,255,255,0.2) 0%, rgba(255,255,255,0.05) 100%)'
                                                    : 'linear-gradient(135deg, rgba(255,255,255,0.05) 0%, rgba(255,255,255,0.01) 100%)',
                                        }}
                                    />

                                    {/* Inner Card content */}
                                    <div
                                        className="relative h-full rounded-[31px] flex flex-col p-8 md:p-10 z-10"
                                        style={{
                                            background: `linear-gradient(145deg, ${plan.gradientFrom}, ${plan.gradientTo}), #090909`,
                                        }}
                                    >
                                        {/* Radial Hover Glow (Subtle) */}
                                        <div className={`absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-700 pointer-events-none rounded-[31px] z-0 ${plan.highlighted ? 'bg-[radial-gradient(circle_at_50%_0%,rgba(249,115,22,0.05),transparent_70%)]' : 'bg-[radial-gradient(circle_at_50%_0%,rgba(255,255,255,0.02),transparent_70%)]'}`} />

                                        <div className="relative z-10 flex-col flex h-full">
                                            {/* Card Header */}
                                            <div className="mb-8">
                                                <div className="flex items-start justify-between mb-6">
                                                    <div
                                                        className={`w-12 h-12 rounded-2xl flex items-center justify-center shadow-inner ${plan.highlighted
                                                                ? 'bg-gradient-to-br from-brand-500/20 to-brand-500/5 border border-brand-500/20 text-brand-400'
                                                                : 'bg-surface-300 border border-white/5 text-gray-300'
                                                            }`}
                                                    >
                                                        {plan.icon}
                                                    </div>
                                                    {plan.badge && (
                                                        <span className="text-[10px] font-bold uppercase tracking-widest px-3 py-1.5 rounded-full bg-brand-500/10 text-brand-400 border border-brand-500/20 shadow-inner">
                                                            {plan.badge}
                                                        </span>
                                                    )}
                                                </div>

                                                <h3 className="text-2xl font-bold font-sans text-white mb-3 tracking-tight">{plan.name}</h3>
                                                <p className="text-sm text-gray-400 leading-relaxed font-medium">{plan.description}</p>
                                            </div>

                                            {/* Price */}
                                            <div className="mb-8 pb-8 border-b border-white/5 relative">
                                                <div className="absolute bottom-0 left-0 w-1/3 h-[1px] bg-gradient-to-r from-transparent via-white/10 to-transparent" />
                                                <div className="flex items-baseline gap-1">
                                                    <span
                                                        className={`text-5xl font-black tracking-tighter ${plan.highlighted ? 'text-white' : 'text-white'
                                                            }`}
                                                    >
                                                        {plan.price}
                                                    </span>
                                                </div>
                                                <p className="text-xs text-brand-400 font-semibold tracking-wide mt-2">{plan.priceNote}</p>
                                            </div>

                                            {/* Features */}
                                            <ul className="space-y-4 flex-1 mb-10">
                                                {plan.features.map((f, fi) => (
                                                    <li key={fi} className="flex items-start gap-3 group/feature">
                                                        <div
                                                            className={`mt-1 w-5 h-5 rounded-full flex items-center justify-center flex-shrink-0 transition-colors shadow-inner ${f.included
                                                                    ? plan.highlighted
                                                                        ? 'bg-brand-500/20 text-brand-400 border border-brand-500/20'
                                                                        : 'bg-surface-300 text-gray-300 border border-white/10 group-hover/feature:border-white/20'
                                                                    : 'bg-transparent text-gray-700 border border-transparent'
                                                                }`}
                                                        >
                                                            {f.included ? (
                                                                <HiOutlineCheck size={12} strokeWidth={2.5} />
                                                            ) : (
                                                                <span className="w-1.5 h-0.5 bg-current rounded" />
                                                            )}
                                                        </div>
                                                        <span
                                                            className={`text-sm pt-0.5 ${f.included ? 'text-gray-300 font-medium' : 'text-gray-600 line-through decoration-gray-700/50'
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
                                                className={`group/btn relative flex items-center justify-center gap-2 w-full py-4 px-6 rounded-xl font-bold transition-all duration-300 overflow-hidden ${plan.highlighted
                                                        ? 'bg-brand-500 text-white shadow-[0_0_20px_rgba(249,115,22,0.2)] hover:shadow-[0_0_30px_rgba(249,115,22,0.4)]'
                                                        : 'bg-surface-300 text-white border border-white/5 hover:bg-surface-200 hover:border-white/10'
                                                    }`}
                                            >
                                                {plan.highlighted && (
                                                    <>
                                                        <div className="absolute inset-0 bg-white/20 opacity-0 group-hover/btn:opacity-100 transition-opacity duration-300" />
                                                        <div className="absolute inset-0 bg-gradient-to-b from-white/20 to-transparent opacity-30" />
                                                    </>
                                                )}
                                                <HiOutlineMail size={18} className="relative z-10" />
                                                <span className="relative z-10">{plan.cta}</span>
                                            </a>
                                        </div>
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
