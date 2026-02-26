'use client';

import React from 'react';
import { motion } from 'framer-motion';
import { SectionReveal } from '@/components/ui/SectionReveal';
import { HiOutlineDesktopComputer, HiOutlineServer, HiOutlineLink } from 'react-icons/hi';
import { useTranslation } from '@/lib/i18n';

interface StepConfig {
    icon: React.ReactNode;
    labelKey: string;
    descKey: string;
    color: string;
    dotColor: string;
}

const stepConfigs: StepConfig[] = [
    {
        icon: <HiOutlineDesktopComputer size={28} />,
        labelKey: 'howItWorks.desktop',
        descKey: 'howItWorks.desktopDesc',
        color: 'brand',
        dotColor: 'bg-brand-500',
    },
    {
        icon: <HiOutlineServer size={28} />,
        labelKey: 'howItWorks.controlPlane',
        descKey: 'howItWorks.controlPlaneDesc',
        color: 'brand',
        dotColor: 'bg-brand-400',
    },
    {
        icon: <HiOutlineLink size={28} />,
        labelKey: 'howItWorks.integrations',
        descKey: 'howItWorks.integrationsDesc',
        color: 'accent',
        dotColor: 'bg-accent-400',
    },
];

export function FlowDiagram() {
    const { t } = useTranslation();

    const steps = stepConfigs.map((s) => ({
        ...s,
        label: t(s.labelKey as any) as string,
        description: t(s.descKey as any) as string,
    }));

    return (
        <SectionReveal>
            <div className="relative">
                {/* Desktop: horizontal flow */}
                <div className="hidden md:flex items-start justify-between relative">
                    {/* Connecting line */}
                    <div className="absolute top-10 left-[15%] right-[15%] h-px">
                        <motion.div
                            className="h-full bg-gradient-to-r from-brand-500/40 via-brand-400/30 to-accent-400/40"
                            initial={{ scaleX: 0 }}
                            whileInView={{ scaleX: 1 }}
                            viewport={{ once: true }}
                            transition={{ duration: 1.2, delay: 0.3, ease: 'easeInOut' }}
                            style={{ transformOrigin: 'left' }}
                        />
                    </div>

                    {/* Animated data dots on the line */}
                    <motion.div
                        className="absolute top-[38px] left-[15%] w-2 h-2 rounded-full bg-brand-400 shadow-glow"
                        animate={{ x: [0, 800, 0] }}
                        transition={{ duration: 4, repeat: Infinity, ease: 'linear' }}
                        style={{ opacity: 0.6 }}
                    />

                    {steps.map((step, i) => (
                        <motion.div
                            key={step.labelKey}
                            className="flex flex-col items-center text-center w-1/3 relative z-10"
                            initial={{ opacity: 0, y: 20 }}
                            whileInView={{ opacity: 1, y: 0 }}
                            viewport={{ once: true }}
                            transition={{ delay: i * 0.2, duration: 0.5 }}
                        >
                            {/* Node */}
                            <div className={`w-20 h-20 rounded-2xl glass-card flex items-center justify-center mb-5 ${step.color === 'brand' ? 'text-brand-400 border-brand-500/20' : 'text-accent-400 border-accent-400/20'} border`}>
                                {step.icon}
                            </div>

                            {/* Step indicator */}
                            <div className={`w-3 h-3 rounded-full ${step.dotColor} mb-4 shadow-glow`} />

                            <h3 className="text-base font-semibold text-white mb-2">{step.label}</h3>
                            <p className="text-sm text-gray-400 max-w-[200px]">{step.description}</p>
                        </motion.div>
                    ))}
                </div>

                {/* Mobile: vertical flow */}
                <div className="md:hidden space-y-6">
                    {steps.map((step, i) => (
                        <motion.div
                            key={step.labelKey}
                            className="flex items-start gap-4"
                            initial={{ opacity: 0, x: -20 }}
                            whileInView={{ opacity: 1, x: 0 }}
                            viewport={{ once: true }}
                            transition={{ delay: i * 0.15 }}
                        >
                            <div className="flex flex-col items-center">
                                <div className={`w-12 h-12 rounded-xl glass-card flex items-center justify-center ${step.color === 'brand' ? 'text-brand-400' : 'text-accent-400'}`}>
                                    {step.icon}
                                </div>
                                {i < steps.length - 1 && (
                                    <div className="w-px h-12 bg-gradient-to-b from-brand-500/30 to-transparent mt-2" />
                                )}
                            </div>
                            <div className="pt-2">
                                <h3 className="text-base font-semibold text-white">{step.label}</h3>
                                <p className="text-sm text-gray-400 mt-1">{step.description}</p>
                            </div>
                        </motion.div>
                    ))}
                </div>
            </div>
        </SectionReveal>
    );
}
