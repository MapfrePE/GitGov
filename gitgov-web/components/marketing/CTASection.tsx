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
        <section className="relative py-24 md:py-32 overflow-hidden">
            {/* Background gradient */}
            <div className="absolute inset-0">
                <div
                    className="absolute inset-0"
                    style={{
                        background: 'radial-gradient(ellipse at center, rgba(0, 229, 218, 0.06) 0%, transparent 70%)',
                    }}
                />
            </div>

            {/* Top/bottom lines */}
            <div className="absolute top-0 left-0 right-0 h-px bg-gradient-to-r from-transparent via-brand-500/20 to-transparent" />
            <div className="absolute bottom-0 left-0 right-0 h-px bg-gradient-to-r from-transparent via-brand-500/10 to-transparent" />

            <Container>
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
