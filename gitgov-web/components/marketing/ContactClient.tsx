'use client';

import React, { useState } from 'react';
import { Container } from '@/components/layout';
import { SectionHeader } from '@/components/marketing';
import { Input, Textarea, Button, SectionReveal } from '@/components/ui';
import { HiOutlineMail, HiOutlineCheck, HiOutlineExclamation } from 'react-icons/hi';
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

        // Client-side validation
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

            if (res.ok) {
                setFormState('success');
            } else {
                setFormState('error');
            }
        } catch {
            setFormState('error');
        }
    }

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

            {/* Contact Form */}
            <section className="pb-24">
                <Container size="narrow">
                    <SectionReveal>
                        <div className="glass-card rounded-2xl p-8 md:p-10 max-w-2xl mx-auto">
                            {formState === 'success' ? (
                                <div className="text-center py-12">
                                    <div className="w-16 h-16 rounded-full bg-emerald-500/10 border border-emerald-500/20 flex items-center justify-center mx-auto mb-6">
                                        <HiOutlineCheck className="text-emerald-400" size={28} />
                                    </div>
                                    <h3 className="text-xl font-semibold text-white mb-2">{t('contact.success.title')}</h3>
                                    <p className="text-gray-400">
                                        {t('contact.success.description')}
                                    </p>
                                    <Button
                                        variant="ghost"
                                        className="mt-6"
                                        onClick={() => setFormState('idle')}
                                    >
                                        {t('contact.success.button')}
                                    </Button>
                                </div>
                            ) : (
                                <form onSubmit={handleSubmit} noValidate>
                                    <div className="flex items-center gap-3 mb-8">
                                        <div className="w-10 h-10 rounded-xl bg-brand-500/10 flex items-center justify-center">
                                            <HiOutlineMail className="text-brand-400" size={20} />
                                        </div>
                                        <div>
                                            <h3 className="text-lg font-semibold text-white">{t('contact.form.title')}</h3>
                                            <p className="text-xs text-gray-500">{t('contact.form.subtitle')}</p>
                                        </div>
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
                                            <span>{t('contact.error')}</span>
                                        </div>
                                    )}

                                    <div className="mt-8">
                                        <Button
                                            type="submit"
                                            variant="primary"
                                            size="lg"
                                            className="w-full"
                                            disabled={formState === 'loading'}
                                        >
                                            {formState === 'loading' ? t('contact.form.sending') : t('contact.form.send')}
                                        </Button>
                                    </div>
                                </form>
                            )}
                        </div>
                    </SectionReveal>
                </Container>
            </section>
        </>
    );
}
