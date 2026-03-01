'use client';

import React from 'react';
import { Button } from '@/components/ui/Button';
import { SectionReveal } from '@/components/ui/SectionReveal';
import { Container } from '@/components/layout/Container';

interface CTASectionProps {
    title: string;
    titleAccent?: string;
    description: string;
    primaryCta: { label: string; href: string };
    secondaryCta?: { label: string; href: string };
}

export function CTASection({
    title,
    titleAccent,
    description,
    primaryCta,
    secondaryCta,
}: CTASectionProps) {
    return (
        <section className="relative py-24 md:py-32 overflow-hidden bg-[#05070c]">
            {/* Background gradient — pointer-events-none so clicks reach buttons */}
            <div className="absolute inset-0 pointer-events-none">
                <div
                    className="absolute inset-0"
                    style={{
                        background:
                            'linear-gradient(180deg, rgba(5,7,12,0.88) 0%, rgba(5,7,12,0.98) 56%, rgba(4,6,10,1) 100%)',
                    }}
                />
                <div
                    className="absolute inset-0"
                    style={{
                        background:
                            'radial-gradient(90% 55% at 50% 42%, rgba(249,115,22,0.10) 0%, rgba(249,115,22,0.04) 30%, rgba(249,115,22,0.00) 65%)',
                    }}
                />
                <div
                    className="absolute inset-0 opacity-[0.025]"
                    style={{
                        backgroundImage:
                            'linear-gradient(rgba(255,255,255,0.05) 1px, transparent 1px), linear-gradient(90deg, rgba(255,255,255,0.05) 1px, transparent 1px)',
                        backgroundSize: '64px 64px',
                    }}
                />
            </div>

            {/* Top/bottom lines */}
            <div className="absolute top-0 left-0 right-0 h-px bg-gradient-to-r from-transparent via-white/10 to-transparent pointer-events-none" />
            <div className="absolute bottom-0 left-0 right-0 h-px bg-gradient-to-r from-transparent via-white/8 to-transparent pointer-events-none" />

            <Container className="relative z-10">
                <SectionReveal>
                    <div className="text-center max-w-3xl mx-auto">
                        <h2 className="text-3xl md:text-heading font-bold tracking-tight">
                            <span className="text-white">{title}</span>
                            {titleAccent && (
                                <span className="gradient-text"> {titleAccent}</span>
                            )}
                        </h2>
                        <p className="mt-6 text-lg text-gray-400 leading-relaxed">{description}</p>
                        <div className="mt-10 flex flex-col sm:flex-row items-center justify-center gap-4">
                            <Button variant="primary" size="lg" href={primaryCta.href}>
                                {primaryCta.label}
                            </Button>
                            {secondaryCta && (
                                <Button variant="outline" size="lg" href={secondaryCta.href}>
                                    {secondaryCta.label}
                                </Button>
                            )}
                        </div>
                    </div>
                </SectionReveal>
            </Container>
        </section>
    );
}
