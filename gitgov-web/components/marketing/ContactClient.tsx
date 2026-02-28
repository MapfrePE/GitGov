'use client';

import React, { useState } from 'react';
import { Container } from '@/components/layout';
import { SectionHeader } from '@/components/marketing';
import { Input, Textarea, Button, SectionReveal } from '@/components/ui';
import {
    HiOutlineMail,
    HiOutlineCheck,
    HiOutlineExclamation,
    HiOutlineShieldCheck,
    HiOutlineLightningBolt,
    HiOutlineSupport,
    HiOutlineGlobe,
} from 'react-icons/hi';
import { useTranslation } from '@/lib/i18n';

type FormState = 'idle' | 'loading' | 'success' | 'error';

export function ContactClient() {
    const { t } = useTranslation();
    const [formState, setFormState] = useState<FormState>('idle');
    const [errors, setErrors] = useState<Record<string, string>>({});

    async function handleSubmit(e: React.FormEvent<HTMLFormElement>) {
        e.preventDefault();
        setErrors({});

        const formData = new FormData(e.currentTarget);
        const data = {
            name: formData.get('name') as string,
            email: formData.get('email') as string,
            company: formData.get('company') as string,
            message: formData.get('message') as string,
        };

        const newErrors: Record<string, string> = {};
        if (!data.name.trim()) newErrors.name = t('contact.errors.name') as string;
        if (!data.email.trim()) newErrors.email = t('contact.errors.email') as string;
        else if (!/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(data.email)) newErrors.email = t('contact.errors.emailInvalid') as string;
        if (!data.message.trim()) newErrors.message = t('contact.errors.message') as string;

        if (Object.keys(newErrors).length > 0) {
            setErrors(newErrors);
            return;
        }

        setFormState('loading');

        try {
            const res = await fetch('/api/contact', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(data),
            });
            setFormState(res.ok ? 'success' : 'error');
        } catch {
            setFormState('error');
        }
    }

    const highlights = [
        {
            icon: <HiOutlineShieldCheck size={20} />,
            title: t('contact.side.h1title') as string,
            desc: t('contact.side.h1desc') as string,
        },
        {
            icon: <HiOutlineLightningBolt size={20} />,
            title: t('contact.side.h2title') as string,
            desc: t('contact.side.h2desc') as string,
        },
        {
            icon: <HiOutlineSupport size={20} />,
            title: t('contact.side.h3title') as string,
            desc: t('contact.side.h3desc') as string,
        },
        {
            icon: <HiOutlineGlobe size={20} />,
            title: t('contact.side.h4title') as string,
            desc: t('contact.side.h4desc') as string,
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
                        badge={t('contact.badge') as string}
                        title={t('contact.title') as string}
                        titleAccent={t('contact.titleAccent') as string}
                        description={t('contact.description') as string}
                    />
                </Container>
            </section>

            {/* Split Layout */}
            <section className="pb-28">
                <Container>
                    <SectionReveal>
                        <div className="max-w-5xl mx-auto grid md:grid-cols-2 gap-8 items-stretch">

                            {/* Left — Info panel */}
                            <div
                                className="rounded-2xl p-8 md:p-10 border border-white/5 flex flex-col justify-between"
                                style={{ background: 'linear-gradient(145deg, rgba(0,229,218,0.07), rgba(0,229,218,0.01)), #0d1117' }}
                            >
                                {/* Top */}
                                <div>
                                    <div className="flex items-center gap-2 mb-6">
                                        <div className="w-9 h-9 rounded-xl bg-brand-500/15 flex items-center justify-center text-brand-400">
                                            <HiOutlineMail size={18} />
                                        </div>
                                        <span className="text-sm font-bold text-white tracking-wide">GitGov</span>
                                    </div>

                                    <h2 className="text-2xl font-bold text-white mb-3">
                                        {t('contact.side.heading') as string}
                                    </h2>
                                    <p className="text-gray-500 text-sm leading-relaxed mb-8">
                                        {t('contact.side.intro') as string}
                                    </p>

                                    {/* Highlights */}
                                    <div className="space-y-5">
                                        {highlights.map((h, i) => (
                                            <div key={i} className="flex items-start gap-4">
                                                <div className="w-9 h-9 rounded-xl bg-white/5 border border-white/5 flex items-center justify-center text-brand-400 flex-shrink-0">
                                                    {h.icon}
                                                </div>
                                                <div>
                                                    <p className="text-sm font-semibold text-white mb-1">{h.title}</p>
                                                    <p className="text-xs text-gray-600 leading-relaxed">{h.desc}</p>
                                                </div>
                                            </div>
                                        ))}
                                    </div>
                                </div>

                                {/* Bottom — response time badge */}
                                <div className="mt-10 pt-6 border-t border-white/5 flex items-center gap-3">
                                    <div className="w-2 h-2 rounded-full bg-brand-400 animate-pulse flex-shrink-0" />
                                    <p className="text-xs text-gray-600">
                                        {t('contact.side.responseTime') as string}
                                    </p>
                                </div>
                            </div>

                            {/* Right — Form */}
                            <div>
                                <div
                                    className="rounded-2xl p-8 md:p-10 border border-white/5"
                                    style={{ background: 'linear-gradient(145deg, rgba(255,255,255,0.03), rgba(255,255,255,0.01)), #0d1117' }}
                                >
                                    {formState === 'success' ? (
                                        <div className="text-center py-14">
                                            <div className="w-16 h-16 rounded-full bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center mx-auto mb-6">
                                                <HiOutlineCheck className="text-emerald-400" size={28} />
                                            </div>
                                            <h3 className="text-xl font-semibold text-white mb-2">{t('contact.success.title') as string}</h3>
                                            <p className="text-gray-500 text-sm max-w-xs mx-auto">
                                                {t('contact.success.description') as string}
                                            </p>
                                            <Button variant="ghost" className="mt-6" onClick={() => setFormState('idle')}>
                                                {t('contact.success.button') as string}
                                            </Button>
                                        </div>
                                    ) : (
                                        <form onSubmit={handleSubmit} noValidate>
                                            <div className="mb-7">
                                                <h3 className="text-lg font-semibold text-white mb-1">
                                                    {t('contact.form.title') as string}
                                                </h3>
                                                <p className="text-xs text-gray-600">
                                                    {t('contact.form.subtitle') as string}
                                                </p>
                                            </div>

                                            <div className="space-y-5">
                                                <div className="grid sm:grid-cols-2 gap-5">
                                                    <Input
                                                        label={t('contact.form.name') as string}
                                                        name="name"
                                                        placeholder={t('contact.form.namePlaceholder') as string}
                                                        error={errors.name}
                                                        required
                                                    />
                                                    <Input
                                                        label={t('contact.form.email') as string}
                                                        name="email"
                                                        type="email"
                                                        placeholder="you@company.com"
                                                        error={errors.email}
                                                        required
                                                    />
                                                </div>
                                                <Input
                                                    label={t('contact.form.company') as string}
                                                    name="company"
                                                    placeholder={t('contact.form.companyPlaceholder') as string}
                                                />
                                                <Textarea
                                                    label={t('contact.form.message') as string}
                                                    name="message"
                                                    placeholder={t('contact.form.messagePlaceholder') as string}
                                                    rows={5}
                                                    error={errors.message}
                                                    required
                                                />
                                            </div>

                                            {formState === 'error' && (
                                                <div className="mt-4 flex items-center gap-2 text-sm text-red-400 bg-red-500/10 border border-red-500/20 rounded-xl px-4 py-3">
                                                    <HiOutlineExclamation size={18} />
                                                    <span>{t('contact.error') as string}</span>
                                                </div>
                                            )}

                                            <div className="mt-7">
                                                <Button
                                                    type="submit"
                                                    variant="primary"
                                                    size="lg"
                                                    className="w-full"
                                                    disabled={formState === 'loading'}
                                                >
                                                    {formState === 'loading'
                                                        ? t('contact.form.sending') as string
                                                        : t('contact.form.send') as string}
                                                </Button>
                                            </div>
                                        </form>
                                    )}
                                </div>
                            </div>
                        </div>
                    </SectionReveal>
                </Container>
            </section>
        </>
    );
}
