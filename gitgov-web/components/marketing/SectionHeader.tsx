'use client';

import React from 'react';
import { motion } from 'framer-motion';
import { SectionReveal } from '@/components/ui/SectionReveal';

interface SectionHeaderProps {
    badge?: string;
    title: string;
    titleAccent?: string;
    description?: string;
    align?: 'center' | 'left';
}

export function SectionHeader({
    badge,
    title,
    titleAccent,
    description,
    align = 'center',
}: SectionHeaderProps) {
    return (
        <SectionReveal className={`mb-16 ${align === 'center' ? 'text-center' : 'text-left'}`}>
            {badge && (
                <motion.span
                    className="inline-block px-3 py-1 text-xs font-semibold tracking-wider uppercase rounded-full bg-brand-500/10 text-brand-400 border border-brand-500/20 mb-4"
                >
                    {badge}
                </motion.span>
            )}
            <h2 className="text-3xl md:text-heading font-bold tracking-tight text-white">
                {title}
                {titleAccent && (
                    <>
                        {' '}
                        <span className="gradient-text">{titleAccent}</span>
                    </>
                )}
            </h2>
            {description && (
                <p className={`mt-4 text-lg text-gray-400 leading-relaxed ${align === 'center' ? 'max-w-2xl mx-auto' : 'max-w-2xl'}`}>
                    {description}
                </p>
            )}
        </SectionReveal>
    );
}
