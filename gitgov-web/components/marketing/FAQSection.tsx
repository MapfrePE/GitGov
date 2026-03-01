'use client';

import React, { useState } from 'react';
import Link from 'next/link';
import { Container } from '@/components/layout';
import { SectionReveal } from '@/components/ui';
import { SectionHeader } from './SectionHeader';
import { useTranslation } from '@/lib/i18n';
import { HiOutlineChevronDown, HiOutlineArrowRight } from 'react-icons/hi';

interface FaqEntry {
    qEn: string;
    qEs: string;
    aEn: string;
    aEs: string;
}

const faqEntries: FaqEntry[] = [
    {
        qEn: 'Does GitGov read my source code?',
        qEs: '¿GitGov lee mi código fuente?',
        aEn: 'No. GitGov captures only metadata: event type, commit SHA, branch, author, timestamp, file count, and repo name. Source code, file contents, diffs, and commit messages never leave the developer workstation.',
        aEs: 'No. GitGov solo captura metadatos: tipo de evento, SHA del commit, rama, autor, timestamp, conteo de archivos y nombre del repo. El código fuente, contenido de archivos, diffs y mensajes de commit nunca abandonan la estación de trabajo del desarrollador.',
    },
    {
        qEn: 'Does GitGov monitor my screen, keystrokes, or apps?',
        qEs: '¿GitGov monitoriza mi pantalla, teclado o apps?',
        aEn: 'No. GitGov only observes Git operations (commit, push, branch creation). It has no access to your screen, clipboard, browser, or IDE.',
        aEs: 'No. GitGov solo observa operaciones Git (commit, push, creación de ramas). No tiene acceso a tu pantalla, portapapeles, navegador ni IDE.',
    },
    {
        qEn: 'Does GitGov make HR or disciplinary decisions?',
        qEs: '¿GitGov toma decisiones de RRHH o disciplinarias?',
        aEn: 'No. Signals are advisory observations — they flag that a policy rule was triggered. The deploying organization is fully responsible for any decisions made based on signals.',
        aEs: 'No. Las señales son observaciones consultivas — indican que una regla se activó. La organización es plenamente responsable de cualquier decisión basada en señales.',
    },
    {
        qEn: 'Is my data encrypted?',
        qEs: '¿Mis datos están cifrados?',
        aEn: 'Yes. TLS in transit, AES-256 at rest on Supabase databases, API keys stored as SHA-256 hashes, and workstation credentials stored in the OS keyring.',
        aEs: 'Sí. TLS en tránsito, AES-256 en reposo en bases de datos Supabase, API keys almacenadas como hashes SHA-256, y credenciales en el keyring del sistema operativo.',
    },
    {
        qEn: 'Can I self-host the Control Plane?',
        qEs: '¿Puedo self-hostear el Control Plane?',
        aEn: 'Yes. The Control Plane can be deployed on any server running Rust binaries with a PostgreSQL database.',
        aEs: 'Sí. El Control Plane puede desplegarse en cualquier servidor que ejecute binarios Rust con una base de datos PostgreSQL.',
    },
    {
        qEn: 'Does GitGov replace CI/CD?',
        qEs: '¿GitGov reemplaza CI/CD?',
        aEn: 'No. GitGov integrates with CI/CD tools (Jenkins, GitHub Actions) to correlate commits with pipeline results. It does not run builds, tests, or deployments.',
        aEs: 'No. GitGov se integra con herramientas CI/CD (Jenkins, GitHub Actions) para correlacionar commits con pipelines. No ejecuta builds, tests ni despliegues.',
    },
];

function AccordionItem({ entry, isOpen, onToggle, isEs }: {
    entry: FaqEntry;
    isOpen: boolean;
    onToggle: () => void;
    isEs: boolean;
}) {
    return (
        <div className="border-b border-white/[0.06] last:border-b-0">
            <button
                onClick={onToggle}
                className="w-full flex items-center justify-between py-5 px-1 text-left group"
                aria-expanded={isOpen}
            >
                <span className={`text-sm font-medium transition-colors ${isOpen ? 'text-white' : 'text-gray-300 group-hover:text-white'}`}>
                    {isEs ? entry.qEs : entry.qEn}
                </span>
                <HiOutlineChevronDown
                    className={`w-4 h-4 text-gray-500 transition-transform duration-200 flex-shrink-0 ml-4 ${isOpen ? 'rotate-180 text-brand-400' : ''}`}
                />
            </button>
            <div
                className={`overflow-hidden transition-all duration-200 ${isOpen ? 'max-h-40 pb-5' : 'max-h-0'}`}
            >
                <p className="text-sm text-gray-400 leading-relaxed px-1">
                    {isEs ? entry.aEs : entry.aEn}
                </p>
            </div>
        </div>
    );
}

export function FAQSection() {
    const { locale, t } = useTranslation();
    const isEs = locale === 'es';
    const [openIndex, setOpenIndex] = useState<number | null>(0);

    return (
        <section className="section-gap" id="faq">
            <Container>
                <SectionHeader
                    badge={isEs ? 'Preguntas Frecuentes' : 'FAQ'}
                    title={isEs ? 'Preguntas' : 'Frequently Asked'}
                    titleAccent={isEs ? 'Frecuentes' : 'Questions'}
                    description={
                        isEs
                            ? 'Las respuestas a lo que más nos preguntan sobre GitGov — especialmente lo que NO hace.'
                            : 'Answers to the most common questions about GitGov — especially what it does NOT do.'
                    }
                />

                <SectionReveal>
                    <div className="max-w-2xl mx-auto mt-12">
                        <div className="glass-card rounded-2xl p-6 md:p-8">
                            {faqEntries.map((entry, i) => (
                                <AccordionItem
                                    key={i}
                                    entry={entry}
                                    isOpen={openIndex === i}
                                    onToggle={() => setOpenIndex(openIndex === i ? null : i)}
                                    isEs={isEs}
                                />
                            ))}
                        </div>

                        <div className="text-center mt-8">
                            <Link
                                href="/docs/faq"
                                className="inline-flex items-center gap-2 text-sm text-brand-400 hover:text-brand-300 transition-colors font-medium"
                            >
                                {isEs ? 'Ver todas las preguntas frecuentes' : 'See all frequently asked questions'}
                                <HiOutlineArrowRight className="w-4 h-4" />
                            </Link>
                        </div>
                    </div>
                </SectionReveal>
            </Container>
        </section>
    );
}
