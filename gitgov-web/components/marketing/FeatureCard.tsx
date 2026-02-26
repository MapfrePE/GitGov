'use client';

import React from 'react';
import { Card } from '@/components/ui/Card';
import { SectionReveal } from '@/components/ui/SectionReveal';

interface FeatureCardProps {
    icon: React.ReactNode;
    title: string;
    description: string;
    badge?: string;
    index?: number;
}

export function FeatureCard({ icon, title, description, badge, index = 0 }: FeatureCardProps) {
    return (
        <SectionReveal delay={index * 0.1}>
            <Card hover glow className="h-full group">
                <div className="flex flex-col h-full">
                    {/* Icon */}
                    <div className="w-12 h-12 rounded-xl bg-brand-500/10 border border-brand-500/20 flex items-center justify-center text-brand-400 mb-5 group-hover:bg-brand-500/15 group-hover:border-brand-500/30 transition-all duration-300">
                        {icon}
                    </div>

                    {/* Badge */}
                    {badge && (
                        <span className="inline-flex self-start px-2 py-0.5 text-[10px] font-semibold tracking-wider uppercase rounded bg-accent-400/10 text-accent-400 border border-accent-400/20 mb-3">
                            {badge}
                        </span>
                    )}

                    {/* Content */}
                    <h3 className="text-lg font-semibold text-white mb-2 group-hover:text-brand-300 transition-colors duration-300">
                        {title}
                    </h3>
                    <p className="text-sm text-gray-400 leading-relaxed flex-grow">
                        {description}
                    </p>
                </div>
            </Card>
        </SectionReveal>
    );
}
