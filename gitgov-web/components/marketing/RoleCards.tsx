'use client';

import React from 'react';
import { Card } from '@/components/ui/Card';
import { SectionReveal } from '@/components/ui/SectionReveal';

interface RoleCardData {
    icon: React.ReactNode;
    role: string;
    painPoint: string;
    solution: string;
}

interface RoleCardsProps {
    roles: RoleCardData[];
}

export function RoleCards({ roles }: RoleCardsProps) {
    return (
        <div className="grid md:grid-cols-2 lg:grid-cols-3 gap-6">
            {roles.map((role, i) => (
                <SectionReveal key={role.role} delay={i * 0.15}>
                    <Card hover glow padding="lg" className="h-full group">
                        <div className="flex flex-col h-full">
                            {/* Icon */}
                            <div className="w-14 h-14 rounded-2xl bg-gradient-to-br from-brand-500/15 to-accent-400/10 border border-white/[0.06] flex items-center justify-center text-2xl mb-6 group-hover:from-brand-500/25 group-hover:to-accent-400/15 transition-all duration-500">
                                {role.icon}
                            </div>

                            {/* Role name */}
                            <h3 className="text-lg font-semibold text-white mb-3">{role.role}</h3>

                            {/* Pain point */}
                            <div className="mb-4">
                                <span className="text-[10px] font-semibold tracking-wider uppercase text-red-400/70">Challenge</span>
                                <p className="text-sm text-gray-400 mt-1">{role.painPoint}</p>
                            </div>

                            {/* Solution */}
                            <div className="mt-auto pt-4 border-t border-white/[0.04]">
                                <span className="text-[10px] font-semibold tracking-wider uppercase text-brand-400/70">With GitGov</span>
                                <p className="text-sm text-gray-300 mt-1">{role.solution}</p>
                            </div>
                        </div>
                    </Card>
                </SectionReveal>
            ))}
        </div>
    );
}
