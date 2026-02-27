'use client';

import React from 'react';
import { Container } from '@/components/layout';
import { CTASection } from '@/components/marketing';
import { SectionReveal } from '@/components/ui';
import { useTranslation } from '@/lib/i18n';
import {
    HiOutlineShieldCheck,
    HiOutlineDocumentSearch,
    HiOutlineLightningBolt,
    HiOutlineEye,
    HiOutlineLockClosed,
    HiOutlineClipboardCheck,
    HiOutlinePuzzle,
    HiOutlineTrendingUp,
    HiOutlineWifi,
    HiOutlineDesktopComputer,
    HiOutlineCheckCircle,
    HiOutlineArrowRight,
    HiOutlineCode,
} from 'react-icons/hi';

/* ─────────────────────────────────────────────────────────────────────────
   Section label divider
───────────────────────────────────────────────────────────────────────── */
function SectionLabel({
    icon, label, iconCls, textCls, lineCls,
}: {
    icon: React.ReactNode; label: string;
    iconCls: string; textCls: string; lineCls: string;
}) {
    return (
        <SectionReveal>
            <div className="flex items-center gap-4 mb-16">
                <div className={`w-8 h-8 rounded-lg border flex items-center justify-center shrink-0 ${iconCls}`}>
                    {icon}
                </div>
                <span className={`text-[11px] font-black tracking-[0.25em] uppercase ${textCls}`}>{label}</span>
                <div className={`flex-1 h-px bg-gradient-to-r ${lineCls} to-transparent`} />
            </div>
        </SectionReveal>
    );
}

/* ─────────────────────────────────────────────────────────────────────────
   Main
───────────────────────────────────────────────────────────────────────── */
export function FeaturesClient() {
    const { t } = useTranslation();

    return (
        <>
            {/* ══════════════════════════════════════════════════════
                HERO
            ══════════════════════════════════════════════════════ */}
            <section className="pt-32 md:pt-44 pb-0 relative overflow-hidden">
                {/* Fine grid */}
                <div
                    className="absolute inset-0 pointer-events-none"
                    style={{
                        opacity: 0.025,
                        backgroundImage: `linear-gradient(rgba(0,229,218,0.6) 1px, transparent 1px),
                                          linear-gradient(90deg, rgba(0,229,218,0.6) 1px, transparent 1px)`,
                        backgroundSize: '72px 72px',
                    }}
                />
                {/* Central bloom */}
                <div
                    className="absolute top-0 left-1/2 -translate-x-1/2 w-[1400px] h-[700px] pointer-events-none"
                    style={{
                        background:
                            'radial-gradient(ellipse at 50% 0%, rgba(0,229,218,0.1) 0%, rgba(0,229,218,0.04) 35%, transparent 70%)',
                    }}
                />

                <Container>
                    <div className="text-center max-w-5xl mx-auto">
                        <SectionReveal>
                            {/* Badge */}
                            <div className="inline-flex items-center gap-2.5 px-4 py-2 rounded-full bg-brand-500/10 border border-brand-500/25 mb-10">
                                <div className="w-1.5 h-1.5 rounded-full bg-brand-400 animate-pulse" />
                                <span className="text-brand-400 text-[11px] font-black tracking-[0.2em] uppercase">
                                    {t('features.badge') as string}
                                </span>
                            </div>

                            {/* Headline — maximum drama */}
                            <h1 className="font-black tracking-tight leading-[0.9] mb-8">
                                <span className="block text-white text-5xl md:text-6xl lg:text-7xl xl:text-[88px]">
                                    {t('features.title') as string}
                                </span>
                                <span className="block gradient-text text-5xl md:text-6xl lg:text-7xl xl:text-[88px]">
                                    {t('features.titleAccent') as string}
                                </span>
                            </h1>

                            <p className="text-lg md:text-xl text-gray-400 leading-relaxed max-w-2xl mx-auto mb-14">
                                {t('features.description') as string}
                            </p>
                        </SectionReveal>

                        {/* Anchor navigation */}
                        <SectionReveal delay={0.1}>
                            <div className="flex flex-wrap justify-center gap-3 mb-20">
                                {[
                                    { label: 'Core', href: '#core', cls: 'text-brand-400 border-brand-500/30 bg-brand-500/10 hover:bg-brand-500/20' },
                                    { label: 'Infrastructure', href: '#infra', cls: 'text-accent-400 border-accent-400/30 bg-accent-400/10 hover:bg-accent-400/20' },
                                    { label: 'Integrations', href: '#integrations', cls: 'text-purple-400 border-purple-400/30 bg-purple-400/10 hover:bg-purple-400/20' },
                                ].map((p) => (
                                    <a
                                        key={p.label}
                                        href={p.href}
                                        className={`flex items-center gap-2 px-5 py-2.5 rounded-full text-sm font-semibold border transition-all duration-200 ${p.cls}`}
                                    >
                                        {p.label}
                                        <HiOutlineArrowRight size={13} />
                                    </a>
                                ))}
                            </div>
                        </SectionReveal>
                    </div>

                    {/* Architecture flow */}
                    <SectionReveal delay={0.2}>
                        <div className="relative max-w-2xl mx-auto">
                            {/* Connector line */}
                            <div
                                className="absolute top-[44px] left-[calc(16.67%+28px)] right-[calc(16.67%+28px)] h-px pointer-events-none hidden sm:block"
                                style={{
                                    background:
                                        'linear-gradient(90deg, rgba(0,229,218,0.5) 0%, rgba(255,187,26,0.5) 50%, rgba(168,85,247,0.5) 100%)',
                                }}
                            />
                            <div className="grid grid-cols-3 gap-4 relative z-10">
                                {[
                                    {
                                        icon: <HiOutlineDesktopComputer size={22} />,
                                        label: 'Desktop',
                                        sub: 'Tauri · Rust · React',
                                        borderCls: 'border-brand-500/40',
                                        bgCls: 'bg-brand-500/10',
                                        textCls: 'text-brand-400',
                                        dotCls: 'bg-brand-400',
                                    },
                                    {
                                        icon: <HiOutlineShieldCheck size={22} />,
                                        label: 'Control Plane',
                                        sub: 'Axum · PostgreSQL',
                                        borderCls: 'border-accent-400/40',
                                        bgCls: 'bg-accent-400/10',
                                        textCls: 'text-accent-400',
                                        dotCls: 'bg-accent-400',
                                    },
                                    {
                                        icon: <HiOutlinePuzzle size={22} />,
                                        label: 'Integrations',
                                        sub: 'Jenkins · Jira · GitHub',
                                        borderCls: 'border-purple-400/40',
                                        bgCls: 'bg-purple-400/10',
                                        textCls: 'text-purple-400',
                                        dotCls: 'bg-purple-400',
                                    },
                                ].map((node, i) => (
                                    <div
                                        key={i}
                                        className={`glass-card rounded-2xl p-5 border ${node.borderCls} text-center relative overflow-hidden group cursor-default`}
                                    >
                                        <div
                                            className={`absolute inset-0 ${node.bgCls} opacity-0 group-hover:opacity-100 transition-opacity duration-300 pointer-events-none`}
                                        />
                                        <div
                                            className={`w-11 h-11 rounded-xl ${node.bgCls} border ${node.borderCls} flex items-center justify-center ${node.textCls} mx-auto mb-3 relative z-10`}
                                        >
                                            {node.icon}
                                        </div>
                                        <div className="text-white text-sm font-bold mb-0.5 relative z-10">{node.label}</div>
                                        <div className="text-gray-600 text-[10px] font-mono relative z-10">{node.sub}</div>
                                        <div className={`w-1.5 h-1.5 rounded-full ${node.dotCls} mx-auto mt-3 relative z-10 animate-pulse`} />
                                    </div>
                                ))}
                            </div>
                            {/* Arrow overlays */}
                            {[
                                'left-[calc(33.33%-6px)]',
                                'left-[calc(66.67%-6px)]',
                            ].map((pos, i) => (
                                <div
                                    key={i}
                                    className={`absolute top-[36px] ${pos} hidden sm:flex items-center pointer-events-none z-20`}
                                >
                                    <div className="w-3 h-px bg-white/20" />
                                    <HiOutlineArrowRight size={10} className="text-white/30 -ml-0.5" />
                                </div>
                            ))}
                        </div>
                    </SectionReveal>
                </Container>

                {/* ── Metrics strip ── */}
                <div className="mt-20 border-y border-white/[0.06]" style={{ background: 'rgba(255,255,255,0.015)' }}>
                    <Container>
                        <SectionReveal delay={0.3}>
                            <div className="grid grid-cols-2 md:grid-cols-4 divide-x divide-white/[0.06]">
                                {[
                                    { value: '100%', label: 'Event Capture', sub: 'commit · push · merge · rebase', valueCls: 'text-brand-400' },
                                    { value: '0', label: 'Data Overwrites', sub: 'append-only guarantee', valueCls: 'text-green-400' },
                                    { value: '30s', label: 'Auto-Refresh', sub: 'dashboard live updates', valueCls: 'text-accent-400' },
                                    { value: '4+', label: 'Integrations', sub: 'Jenkins · Jira · GitHub · API', valueCls: 'text-purple-400' },
                                ].map((m, i) => (
                                    <div key={i} className="text-center py-8 px-4">
                                        <div className={`text-4xl md:text-5xl font-black tracking-tight mb-1.5 ${m.valueCls}`}>
                                            {m.value}
                                        </div>
                                        <div className="text-xs font-bold text-white/70 uppercase tracking-widest mb-0.5">
                                            {m.label}
                                        </div>
                                        <div className="text-[10px] font-mono text-gray-600">{m.sub}</div>
                                    </div>
                                ))}
                            </div>
                        </SectionReveal>
                    </Container>
                </div>
            </section>

            {/* ══════════════════════════════════════════════════════
                CORE
            ══════════════════════════════════════════════════════ */}
            <section id="core" className="py-28 relative">
                {/* Section spotlight */}
                <div
                    className="absolute top-1/2 left-[-200px] -translate-y-1/2 w-[600px] h-[600px] rounded-full pointer-events-none"
                    style={{ background: 'radial-gradient(ellipse, rgba(0,229,218,0.07) 0%, transparent 65%)' }}
                />
                <div className="absolute inset-0 bg-gradient-to-b from-transparent via-brand-500/[0.02] to-transparent pointer-events-none" />

                <Container>
                    <SectionLabel
                        icon={<HiOutlineShieldCheck size={16} className="text-brand-400" />}
                        label="Core"
                        iconCls="text-brand-400 bg-brand-500/10 border-brand-500/25"
                        textCls="text-brand-400"
                        lineCls="from-brand-500/30"
                    />

                    {/* Bento grid — Row 1 */}
                    <div className="grid grid-cols-1 lg:grid-cols-3 gap-4 mb-4">

                        {/* [A] Terminal card — 2 cols */}
                        <SectionReveal className="lg:col-span-2">
                            <div className="glass-card rounded-2xl overflow-hidden glow-border h-full group hover:-translate-y-1 transition-transform duration-300 relative">
                                <div
                                    className="absolute -top-20 -right-20 w-80 h-80 rounded-full pointer-events-none"
                                    style={{ background: 'radial-gradient(ellipse, rgba(0,229,218,0.1) 0%, transparent 70%)' }}
                                />
                                <div className="p-7 md:p-8 relative z-10 flex flex-col h-full">
                                    <div className="w-14 h-14 rounded-xl bg-brand-500/15 border border-brand-500/30 flex items-center justify-center text-brand-400 mb-5 group-hover:bg-brand-500/20 transition-colors">
                                        <HiOutlineShieldCheck size={26} />
                                    </div>
                                    <h3 className="text-2xl font-black text-white mb-2">
                                        {t('features.commit.title') as string}
                                    </h3>
                                    <p className="text-gray-400 text-sm leading-relaxed mb-7 max-w-md">
                                        {t('features.commit.desc') as string}
                                    </p>

                                    {/* Terminal mockup */}
                                    <div className="mt-auto bg-black/60 rounded-xl border border-white/[0.07] overflow-hidden font-mono">
                                        {/* Title bar */}
                                        <div className="flex items-center gap-1.5 px-4 py-3 bg-white/[0.04] border-b border-white/[0.06]">
                                            <div className="flex gap-1.5">
                                                <div className="w-2.5 h-2.5 rounded-full bg-red-500/70" />
                                                <div className="w-2.5 h-2.5 rounded-full bg-yellow-500/70" />
                                                <div className="w-2.5 h-2.5 rounded-full bg-green-500/70" />
                                            </div>
                                            <span className="text-gray-600 ml-2.5 text-[10px] tracking-wide flex-1">
                                                gitgov · event log · live
                                            </span>
                                            <div className="flex items-center gap-1.5">
                                                <div className="w-1.5 h-1.5 rounded-full bg-brand-400 animate-pulse" />
                                                <span className="text-gray-600 text-[10px]">capturing</span>
                                            </div>
                                        </div>
                                        {/* Events */}
                                        <div className="p-4 space-y-3">
                                            {[
                                                { type: 'commit', sha: 'a3f8c2e', msg: 'feat: add Jenkins correlation widget', ago: '2ms', typeCls: 'text-brand-400', dotCls: 'bg-brand-400' },
                                                { type: 'push  ', sha: 'a3f8c2e', msg: '→ origin/main · 3 files changed', ago: '3ms', typeCls: 'text-green-400', dotCls: 'bg-green-400' },
                                                { type: 'commit', sha: 'b91d04f', msg: 'fix: offset default in pagination', ago: '12s', typeCls: 'text-brand-400', dotCls: 'bg-brand-400' },
                                                { type: 'stage ', sha: 'c44e71a', msg: 'src/handlers.rs · src/models.rs', ago: '28s', typeCls: 'text-accent-400', dotCls: 'bg-accent-400' },
                                            ].map((row, i) => (
                                                <div key={i} className="flex items-center gap-3 text-[11px] leading-none">
                                                    <div className={`w-1.5 h-1.5 rounded-full shrink-0 ${row.dotCls}`} />
                                                    <span className={`${row.typeCls} font-bold w-[46px] shrink-0`}>{row.type}</span>
                                                    <span className="text-yellow-500/60 shrink-0">{row.sha}</span>
                                                    <span className="text-gray-300 truncate flex-1">{row.msg}</span>
                                                    <span className="text-gray-600 shrink-0">{row.ago}</span>
                                                </div>
                                            ))}
                                        </div>
                                        {/* Status bar */}
                                        <div className="px-4 py-2.5 bg-white/[0.025] border-t border-white/[0.05] flex items-center gap-4">
                                            {[
                                                { key: 'events/min', val: '247', valCls: 'text-brand-400' },
                                                { key: 'queue', val: '0', valCls: 'text-green-400' },
                                                { key: 'uptime', val: '99.9%', valCls: 'text-accent-400' },
                                            ].map((stat, i) => (
                                                <React.Fragment key={stat.key}>
                                                    {i > 0 && <div className="w-px h-3 bg-white/10" />}
                                                    <div className="flex items-center gap-1.5">
                                                        <span className="text-[9px] text-gray-600 font-mono uppercase tracking-wider">{stat.key}</span>
                                                        <span className={`text-[10px] font-bold ${stat.valCls}`}>{stat.val}</span>
                                                    </div>
                                                </React.Fragment>
                                            ))}
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </SectionReveal>

                        {/* [B] Zero overwrites stat card — 1 col */}
                        <SectionReveal delay={0.1}>
                            <div className="glass-card rounded-2xl p-6 h-full hover:-translate-y-1 transition-transform duration-300 relative overflow-hidden group flex flex-col">
                                {/* Watermark number */}
                                <div className="absolute -right-3 -bottom-3 text-[120px] font-black leading-none select-none pointer-events-none text-green-400/[0.06]">
                                    0
                                </div>
                                <div className="relative z-10 flex flex-col h-full">
                                    <div className="w-10 h-10 rounded-xl bg-green-400/10 border border-green-400/20 flex items-center justify-center shrink-0 text-green-400 mb-5">
                                        <HiOutlineLockClosed size={20} />
                                    </div>
                                    <h3 className="text-base font-black text-white mb-2 leading-tight">
                                        {t('features.appendOnly.title') as string}
                                    </h3>
                                    <p className="text-xs text-gray-500 leading-relaxed mb-6">
                                        {t('features.appendOnly.desc') as string}
                                    </p>
                                    {/* Big stat */}
                                    <div className="mt-auto">
                                        <div className="flex items-baseline gap-2 mb-1">
                                            <span className="text-5xl font-black text-green-400 leading-none">0</span>
                                            <span className="text-sm font-bold text-white/60">overwrites</span>
                                        </div>
                                        <p className="text-[10px] font-mono text-gray-600">UUID dedup · append-only guarantee</p>
                                        <div className="mt-5 pt-5 border-t border-white/[0.06] flex flex-col gap-2">
                                            {['0 deletes', '0 updates', '∞ reads'].map((item, i) => (
                                                <div key={i} className="flex items-center gap-2">
                                                    <HiOutlineCheckCircle size={12} className="text-green-400 shrink-0" />
                                                    <span className="text-[11px] font-mono text-gray-500">{item}</span>
                                                </div>
                                            ))}
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </SectionReveal>
                    </div>

                    {/* Bento grid — Row 2 */}
                    <div className="grid grid-cols-1 md:grid-cols-2 gap-4">

                        {/* [C] Policy card with toml snippet */}
                        <SectionReveal delay={0.15}>
                            <div className="glass-card rounded-2xl p-6 hover:-translate-y-1 transition-transform duration-300 group">
                                <div className="flex items-start gap-4">
                                    <div className="w-10 h-10 rounded-xl bg-orange-400/10 border border-orange-400/20 flex items-center justify-center shrink-0 text-orange-400">
                                        <HiOutlineClipboardCheck size={20} />
                                    </div>
                                    <div className="flex-1 min-w-0">
                                        <div className="flex items-center gap-2 mb-2">
                                            <h3 className="text-sm font-black text-white">{t('features.policy.title') as string}</h3>
                                            <span className="px-1.5 py-0.5 text-[9px] font-black tracking-widest uppercase rounded bg-orange-400/10 text-orange-400 border border-orange-400/20">
                                                {t('advisory') as string}
                                            </span>
                                        </div>
                                        <p className="text-xs text-gray-500 leading-relaxed mb-4">
                                            {t('features.policy.desc') as string}
                                        </p>
                                        {/* gitgov.toml snippet */}
                                        <div className="bg-black/50 rounded-lg border border-white/[0.06] overflow-hidden">
                                            <div className="px-3 py-1.5 bg-white/[0.03] border-b border-white/[0.04] flex items-center gap-2">
                                                <div className="w-1.5 h-1.5 rounded-full bg-orange-400/50" />
                                                <span className="text-[9px] font-mono text-gray-600">gitgov.toml</span>
                                            </div>
                                            <div className="px-3 py-2.5 text-[10px] font-mono space-y-1">
                                                <div><span className="text-gray-600">[policy]</span></div>
                                                <div>
                                                    <span className="text-gray-500">branch_pattern</span>
                                                    <span className="text-gray-600"> = </span>
                                                    <span className="text-green-400/80">&quot;feat/*&quot;</span>
                                                </div>
                                                <div>
                                                    <span className="text-gray-500">require_ticket</span>
                                                    <span className="text-gray-600"> = </span>
                                                    <span className="text-brand-400/80">true</span>
                                                </div>
                                                <div>
                                                    <span className="text-gray-500">enforce_msg_format</span>
                                                    <span className="text-gray-600"> = </span>
                                                    <span className="text-brand-400/80">true</span>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </SectionReveal>

                        {/* [D] Offline resilience with retry bars */}
                        <SectionReveal delay={0.2}>
                            <div className="glass-card rounded-2xl p-6 hover:-translate-y-1 transition-transform duration-300 group">
                                <div className="flex items-start gap-4">
                                    <div className="w-10 h-10 rounded-xl bg-purple-400/10 border border-purple-400/20 flex items-center justify-center shrink-0 text-purple-400">
                                        <HiOutlineWifi size={20} />
                                    </div>
                                    <div className="flex-1">
                                        <h3 className="text-sm font-black text-white mb-2">{t('features.offline.title') as string}</h3>
                                        <p className="text-xs text-gray-500 leading-relaxed mb-5">
                                            {t('features.offline.desc') as string}
                                        </p>
                                        {/* Exponential backoff visualization */}
                                        <div className="bg-black/30 rounded-lg border border-white/[0.05] px-4 py-3">
                                            <div className="text-[9px] font-mono text-gray-600 uppercase tracking-wider mb-3">
                                                retry schedule · exponential backoff
                                            </div>
                                            <div className="flex items-end gap-2 h-10">
                                                {[
                                                    { delay: '1s', h: 18, cls: 'bg-red-400/50' },
                                                    { delay: '2s', h: 28, cls: 'bg-red-400/50' },
                                                    { delay: '5s', h: 38, cls: 'bg-yellow-400/50' },
                                                    { delay: '10s', h: 52, cls: 'bg-yellow-400/50' },
                                                    { delay: '20s', h: 68, cls: 'bg-green-400/60' },
                                                    { delay: '40s', h: 82, cls: 'bg-green-400/70' },
                                                ].map((bar, i) => (
                                                    <div key={i} className="flex-1 flex flex-col items-center gap-1">
                                                        <div
                                                            className={`w-full rounded-sm ${bar.cls}`}
                                                            style={{ height: `${bar.h}%` }}
                                                        />
                                                        <span className="text-[7px] font-mono text-gray-700">{bar.delay}</span>
                                                    </div>
                                                ))}
                                                <div className="flex items-end pb-3.5">
                                                    <span className="text-[9px] font-mono text-gray-600 whitespace-nowrap ml-1">→ ×32</span>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </div>
                        </SectionReveal>
                    </div>
                </Container>
            </section>

            {/* ══════════════════════════════════════════════════════
                INFRASTRUCTURE
            ══════════════════════════════════════════════════════ */}
            <section id="infra" className="py-28 relative">
                {/* Dot grid */}
                <div
                    className="absolute inset-0 pointer-events-none"
                    style={{
                        opacity: 0.025,
                        backgroundImage: `radial-gradient(circle at 1px 1px, rgba(255,187,26,0.9) 1px, transparent 0)`,
                        backgroundSize: '44px 44px',
                    }}
                />
                {/* Section spotlight */}
                <div
                    className="absolute top-1/2 right-[-200px] -translate-y-1/2 w-[600px] h-[600px] rounded-full pointer-events-none"
                    style={{ background: 'radial-gradient(ellipse, rgba(255,187,26,0.07) 0%, transparent 65%)' }}
                />

                <Container>
                    <SectionLabel
                        icon={<HiOutlineDocumentSearch size={16} className="text-accent-400" />}
                        label="Infrastructure"
                        iconCls="text-accent-400 bg-accent-400/10 border-accent-400/25"
                        textCls="text-accent-400"
                        lineCls="from-accent-400/30"
                    />

                    <div className="grid lg:grid-cols-5 gap-12 items-start">
                        {/* Left — sticky */}
                        <SectionReveal className="lg:col-span-2" direction="left">
                            <div className="lg:sticky lg:top-28">
                                <h2 className="text-3xl md:text-4xl lg:text-5xl font-black text-white mb-4 leading-[0.95]">
                                    {t('features.infra.title') as string}{' '}
                                    <span className="gradient-text">{t('features.infra.titleAccent') as string}</span>
                                </h2>
                                <p className="text-gray-400 leading-relaxed mb-10 text-sm max-w-xs">
                                    {t('features.infra.description') as string}
                                </p>

                                {/* Spec table */}
                                <div className="border-t border-white/[0.07] pt-6 space-y-3.5">
                                    {[
                                        { label: 'Event ingestion', val: 'Batch · deduplicated' },
                                        { label: 'Auto-refresh', val: '30 s interval' },
                                        { label: 'Access roles', val: 'Admin · Dev · PM · Architect' },
                                        { label: 'Backend', val: 'Rust · Axum · PostgreSQL' },
                                        { label: 'Event scoping', val: 'Admin sees all · Dev sees own' },
                                    ].map((row) => (
                                        <div key={row.label} className="flex items-center gap-3">
                                            <HiOutlineCheckCircle size={13} className="text-accent-400 shrink-0" />
                                            <span className="text-gray-500 text-xs">{row.label}</span>
                                            <span className="text-gray-300 font-semibold ml-auto text-right text-xs font-mono">
                                                {row.val}
                                            </span>
                                        </div>
                                    ))}
                                </div>
                            </div>
                        </SectionReveal>

                        {/* Right — feature cards */}
                        <div className="lg:col-span-3 space-y-4">

                            {/* Centralized store with event table */}
                            <SectionReveal delay={0} direction="right">
                                <div className="glass-card rounded-2xl p-6 hover:-translate-y-1 transition-transform duration-300 group">
                                    <div className="flex items-start gap-4">
                                        <div className="w-11 h-11 rounded-xl border border-accent-400/20 bg-accent-400/10 flex items-center justify-center shrink-0 text-accent-400">
                                            <HiOutlineDocumentSearch size={22} />
                                        </div>
                                        <div className="flex-1 min-w-0">
                                            <h3 className="text-base font-black text-white mb-1.5 group-hover:text-accent-300 transition-colors">
                                                {t('features.centralized.title') as string}
                                            </h3>
                                            <p className="text-sm text-gray-400 leading-relaxed mb-4">
                                                {t('features.centralized.desc') as string}
                                            </p>
                                            {/* Mini event table */}
                                            <div className="bg-black/40 rounded-lg border border-white/[0.06] overflow-hidden">
                                                <div
                                                    className="grid px-3 py-2 bg-white/[0.03] border-b border-white/[0.05]"
                                                    style={{ gridTemplateColumns: '76px 1fr 48px' }}
                                                >
                                                    {['event_type', 'user_login', 'ts'].map((h) => (
                                                        <span key={h} className="text-[9px] font-mono text-gray-600 uppercase tracking-widest">
                                                            {h}
                                                        </span>
                                                    ))}
                                                </div>
                                                {[
                                                    { type: 'commit', user: 'carlos.dev', ts: 'now' },
                                                    { type: 'push', user: 'carlos.dev', ts: '3s' },
                                                    { type: 'stage', user: 'ana.pm', ts: '9s' },
                                                ].map((row, i) => (
                                                    <div
                                                        key={i}
                                                        className="grid px-3 py-1.5 border-b border-white/[0.03] last:border-0"
                                                        style={{ gridTemplateColumns: '76px 1fr 48px' }}
                                                    >
                                                        <span className="text-[10px] font-mono text-brand-400">{row.type}</span>
                                                        <span className="text-[10px] font-mono text-gray-400">{row.user}</span>
                                                        <span className="text-[10px] font-mono text-gray-600">{row.ts}</span>
                                                    </div>
                                                ))}
                                            </div>
                                            <div className="flex items-center gap-1.5 mt-2.5">
                                                <div className="w-1 h-1 rounded-full bg-accent-400" />
                                                <span className="text-[10px] font-mono text-gray-600">
                                                    PostgreSQL · Supabase · append-only · UUID dedup
                                                </span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </SectionReveal>

                            {/* Real-time visibility with filter pills */}
                            <SectionReveal delay={0.1} direction="right">
                                <div className="glass-card rounded-2xl p-6 hover:-translate-y-1 transition-transform duration-300 group">
                                    <div className="flex items-start gap-4">
                                        <div className="w-11 h-11 rounded-xl border border-brand-500/20 bg-brand-500/10 flex items-center justify-center shrink-0 text-brand-400">
                                            <HiOutlineEye size={22} />
                                        </div>
                                        <div className="flex-1">
                                            <h3 className="text-base font-black text-white mb-1.5 group-hover:text-brand-300 transition-colors">
                                                {t('features.realtime.title') as string}
                                            </h3>
                                            <p className="text-sm text-gray-400 leading-relaxed mb-4">
                                                {t('features.realtime.desc') as string}
                                            </p>
                                            {/* Active filter pills */}
                                            <div className="flex flex-wrap gap-1.5">
                                                {[
                                                    { label: 'author: carlos', cls: 'text-brand-400 border-brand-500/25 bg-brand-500/10' },
                                                    { label: 'branch: main', cls: 'text-accent-400 border-accent-400/25 bg-accent-400/10' },
                                                    { label: 'type: commit', cls: 'text-green-400 border-green-400/25 bg-green-400/10' },
                                                    { label: 'last 7d', cls: 'text-purple-400 border-purple-400/25 bg-purple-400/10' },
                                                ].map((f, i) => (
                                                    <span
                                                        key={i}
                                                        className={`inline-flex items-center gap-1 px-2.5 py-1 rounded-md text-[10px] font-mono font-semibold border ${f.cls}`}
                                                    >
                                                        <span className="opacity-60">×</span> {f.label}
                                                    </span>
                                                ))}
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </SectionReveal>

                            {/* Admin dashboard with 7-day bar chart */}
                            <SectionReveal delay={0.2} direction="right">
                                <div className="glass-card rounded-2xl p-6 hover:-translate-y-1 transition-transform duration-300 group">
                                    <div className="flex items-start gap-4">
                                        <div className="w-11 h-11 rounded-xl border border-purple-400/20 bg-purple-400/10 flex items-center justify-center shrink-0 text-purple-400">
                                            <HiOutlineDesktopComputer size={22} />
                                        </div>
                                        <div className="flex-1">
                                            <h3 className="text-base font-black text-white mb-1.5 group-hover:text-purple-300 transition-colors">
                                                {t('features.dashboard.title') as string}
                                            </h3>
                                            <p className="text-sm text-gray-400 leading-relaxed mb-4">
                                                {t('features.dashboard.desc') as string}
                                            </p>
                                            {/* 7-day pipeline health bars */}
                                            <div className="bg-black/30 rounded-lg border border-white/[0.05] px-4 py-3">
                                                <div className="flex items-center justify-between mb-2">
                                                    <span className="text-[9px] font-mono text-gray-600 uppercase tracking-wider">
                                                        pipeline health · 7 days
                                                    </span>
                                                    <span className="text-[9px] font-mono text-purple-400 font-bold">
                                                        30s auto-refresh
                                                    </span>
                                                </div>
                                                <div className="flex items-end gap-1.5 h-8">
                                                    {[68, 84, 58, 92, 76, 95, 81].map((pct, i) => (
                                                        <div
                                                            key={i}
                                                            className="flex-1 rounded-sm overflow-hidden flex flex-col justify-end"
                                                            style={{ background: 'rgba(168,85,247,0.12)' }}
                                                        >
                                                            <div
                                                                className="rounded-sm"
                                                                style={{
                                                                    height: `${pct}%`,
                                                                    background:
                                                                        pct > 80
                                                                            ? 'rgba(52,211,153,0.6)'
                                                                            : pct > 65
                                                                            ? 'rgba(168,85,247,0.55)'
                                                                            : 'rgba(248,113,113,0.55)',
                                                                }}
                                                            />
                                                        </div>
                                                    ))}
                                                </div>
                                                <div className="flex justify-between mt-1.5">
                                                    {['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'].map((d) => (
                                                        <span key={d} className="text-[8px] font-mono text-gray-700 flex-1 text-center">
                                                            {d}
                                                        </span>
                                                    ))}
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </SectionReveal>
                        </div>
                    </div>
                </Container>
            </section>

            {/* ══════════════════════════════════════════════════════
                INTEGRATIONS
            ══════════════════════════════════════════════════════ */}
            <section id="integrations" className="py-28 relative overflow-hidden">
                {/* Section spotlight */}
                <div
                    className="absolute top-1/3 left-1/2 -translate-x-1/2 w-[900px] h-[500px] rounded-full pointer-events-none"
                    style={{ background: 'radial-gradient(ellipse, rgba(168,85,247,0.06) 0%, transparent 65%)' }}
                />

                <Container>
                    <SectionLabel
                        icon={<HiOutlinePuzzle size={16} className="text-purple-400" />}
                        label="Integrations"
                        iconCls="text-purple-400 bg-purple-400/10 border-purple-400/25"
                        textCls="text-purple-400"
                        lineCls="from-purple-400/30"
                    />

                    <SectionReveal>
                        <div className="mb-16">
                            <h2 className="text-3xl md:text-4xl lg:text-5xl font-black text-white mb-3 leading-[0.95]">
                                {t('features.integrations.title') as string}{' '}
                                <span className="gradient-text">{t('features.integrations.titleAccent') as string}</span>
                            </h2>
                            <p className="text-gray-400 max-w-xl text-sm">
                                {t('features.integrations.description') as string}
                            </p>
                        </div>
                    </SectionReveal>

                    <div className="grid md:grid-cols-3 gap-5">

                        {/* Jenkins */}
                        <SectionReveal delay={0}>
                            <div className="group glass-card rounded-2xl overflow-hidden hover:-translate-y-2 transition-transform duration-300 h-full flex flex-col">
                                <div className="h-[3px] bg-gradient-to-r from-orange-600 via-amber-400 to-orange-500" />
                                <div className="p-6 flex flex-col flex-1">
                                    <div className="flex items-start justify-between mb-5">
                                        <div className="w-12 h-12 rounded-xl bg-orange-500/10 border border-orange-500/20 flex items-center justify-center">
                                            <HiOutlineLightningBolt size={24} className="text-orange-400" />
                                        </div>
                                        <span className="px-2.5 py-1 text-[9px] font-black tracking-widest uppercase rounded-full bg-orange-500/15 text-orange-400 border border-orange-500/25">
                                            Jenkins
                                        </span>
                                    </div>
                                    <h3 className="text-lg font-black text-white mb-2 group-hover:text-orange-300 transition-colors">
                                        {t('features.jenkins.title') as string}
                                    </h3>
                                    <p className="text-sm text-gray-400 leading-relaxed mb-5 flex-1">
                                        {t('features.jenkins.desc') as string}
                                    </p>

                                    {/* Pipeline stages visualization */}
                                    <div className="mb-5 bg-black/40 rounded-xl p-3.5 border border-white/[0.06]">
                                        <div className="text-[9px] font-mono text-gray-600 uppercase tracking-widest mb-3">
                                            pipeline · last run
                                        </div>
                                        <div className="flex items-stretch gap-1">
                                            {[
                                                { stage: 'Build', ok: true, time: '2m14s' },
                                                { stage: 'Test', ok: true, time: '4m32s' },
                                                { stage: 'Deploy', ok: true, time: '1m03s' },
                                            ].map((s, i) => (
                                                <React.Fragment key={i}>
                                                    <div className="flex-1 bg-white/[0.03] rounded-lg p-2.5 text-center border border-white/[0.04]">
                                                        <div
                                                            className={`text-[9px] font-mono font-bold mb-1 ${
                                                                s.ok ? 'text-green-400' : 'text-red-400'
                                                            }`}
                                                        >
                                                            {s.ok ? '●' : '✕'} {s.stage}
                                                        </div>
                                                        <div className="text-[8px] text-gray-600 font-mono">{s.time}</div>
                                                    </div>
                                                    {i < 2 && (
                                                        <div className="flex items-center shrink-0">
                                                            <div className="w-2 h-px bg-white/15" />
                                                            <HiOutlineArrowRight size={8} className="text-white/20" />
                                                        </div>
                                                    )}
                                                </React.Fragment>
                                            ))}
                                        </div>
                                        <div className="mt-2.5 flex items-center gap-2">
                                            <span className="text-[9px] font-mono text-gray-600">commit a3f8c2e</span>
                                            <div className="flex-1 h-px bg-white/[0.06]" />
                                            <span className="text-[9px] font-mono text-green-400">all passed</span>
                                        </div>
                                    </div>

                                    <div className="pt-4 border-t border-white/[0.06] space-y-1.5">
                                        <div className="flex items-center gap-2">
                                            <div className="w-2 h-2 rounded-full bg-green-400 animate-pulse" />
                                            <span className="text-xs text-gray-400 font-bold">Live · V1.2-A</span>
                                        </div>
                                        <p className="text-[11px] font-mono text-gray-600">
                                            POST /integrations/jenkins · admin Bearer
                                        </p>
                                    </div>
                                </div>
                            </div>
                        </SectionReveal>

                        {/* Jira */}
                        <SectionReveal delay={0.1}>
                            <div className="group glass-card rounded-2xl overflow-hidden hover:-translate-y-2 transition-transform duration-300 h-full flex flex-col">
                                <div className="h-[3px] bg-gradient-to-r from-blue-600 via-cyan-400 to-blue-500" />
                                <div className="p-6 flex flex-col flex-1">
                                    <div className="flex items-start justify-between mb-5">
                                        <div className="w-12 h-12 rounded-xl bg-blue-500/10 border border-blue-500/20 flex items-center justify-center">
                                            <HiOutlinePuzzle size={24} className="text-blue-400" />
                                        </div>
                                        <span className="px-2.5 py-1 text-[9px] font-black tracking-widest uppercase rounded-full bg-blue-500/15 text-blue-400 border border-blue-500/25">
                                            {t('preview') as string}
                                        </span>
                                    </div>
                                    <h3 className="text-lg font-black text-white mb-2 group-hover:text-blue-300 transition-colors">
                                        {t('features.jira.title') as string}
                                    </h3>
                                    <p className="text-sm text-gray-400 leading-relaxed mb-5 flex-1">
                                        {t('features.jira.desc') as string}
                                    </p>

                                    {/* Ticket coverage visualization */}
                                    <div className="mb-5 bg-black/40 rounded-xl p-3.5 border border-white/[0.06]">
                                        <div className="text-[9px] font-mono text-gray-600 uppercase tracking-widest mb-3">
                                            ticket coverage
                                        </div>
                                        <div className="space-y-1.5">
                                            {[
                                                { id: 'PROJ-123', msg: 'feat: auth refresh', linked: true },
                                                { id: 'PROJ-456', msg: 'fix: offset default', linked: true },
                                                { id: '–', msg: 'style: lint fixes', linked: false },
                                            ].map((item, i) => (
                                                <div key={i} className="flex items-center gap-2.5 text-[10px] font-mono">
                                                    <span
                                                        className={`shrink-0 w-16 ${
                                                            item.linked ? 'text-blue-400' : 'text-gray-600'
                                                        }`}
                                                    >
                                                        {item.id}
                                                    </span>
                                                    <span className="text-gray-500 truncate flex-1">{item.msg}</span>
                                                    <span
                                                        className={`shrink-0 text-[9px] px-1.5 py-0.5 rounded font-bold ${
                                                            item.linked
                                                                ? 'text-green-400 bg-green-400/10'
                                                                : 'text-red-400 bg-red-400/10'
                                                        }`}
                                                    >
                                                        {item.linked ? '✓' : '✗'}
                                                    </span>
                                                </div>
                                            ))}
                                        </div>
                                        <div className="mt-2.5 pt-2.5 border-t border-white/[0.05] flex items-center justify-between">
                                            <span className="text-[9px] font-mono text-gray-600">2 / 3 linked</span>
                                            <div className="flex-1 mx-3 h-1 rounded-full bg-white/[0.06] overflow-hidden">
                                                <div className="h-full w-2/3 rounded-full bg-blue-400/50" />
                                            </div>
                                            <span className="text-[9px] font-mono text-blue-400 font-bold">66%</span>
                                        </div>
                                    </div>

                                    <div className="pt-4 border-t border-white/[0.06] space-y-1.5">
                                        <div className="flex items-center gap-2">
                                            <div className="w-2 h-2 rounded-full bg-blue-400" />
                                            <span className="text-xs text-gray-400 font-bold">Preview · V1.2-B</span>
                                        </div>
                                        <p className="text-[11px] font-mono text-gray-600">
                                            ticket_coverage · correlation_batch
                                        </p>
                                    </div>
                                </div>
                            </div>
                        </SectionReveal>

                        {/* GitHub */}
                        <SectionReveal delay={0.2}>
                            <div className="group glass-card rounded-2xl overflow-hidden hover:-translate-y-2 transition-transform duration-300 h-full flex flex-col">
                                <div className="h-[3px] bg-gradient-to-r from-purple-600 via-violet-400 to-purple-500" />
                                <div className="p-6 flex flex-col flex-1">
                                    <div className="flex items-start justify-between mb-5">
                                        <div className="w-12 h-12 rounded-xl bg-purple-500/10 border border-purple-500/20 flex items-center justify-center">
                                            <HiOutlineTrendingUp size={24} className="text-purple-400" />
                                        </div>
                                        <span className="px-2.5 py-1 text-[9px] font-black tracking-widest uppercase rounded-full bg-purple-500/15 text-purple-400 border border-purple-500/25">
                                            {t('available') as string}
                                        </span>
                                    </div>
                                    <h3 className="text-lg font-black text-white mb-2 group-hover:text-purple-300 transition-colors">
                                        {t('features.github.title') as string}
                                    </h3>
                                    <p className="text-sm text-gray-400 leading-relaxed mb-5 flex-1">
                                        {t('features.github.desc') as string}
                                    </p>

                                    {/* Webhook events visualization */}
                                    <div className="mb-5 bg-black/40 rounded-xl p-3.5 border border-white/[0.06]">
                                        <div className="text-[9px] font-mono text-gray-600 uppercase tracking-widest mb-3">
                                            webhook events · hmac validated
                                        </div>
                                        <div className="flex flex-wrap gap-1.5 mb-3">
                                            {['push', 'create', 'pull_request', 'status', 'review'].map((ev, i) => (
                                                <span
                                                    key={i}
                                                    className="px-2 py-1 rounded-md text-[9px] font-mono font-semibold bg-purple-400/10 text-purple-400 border border-purple-400/20"
                                                >
                                                    {ev}
                                                </span>
                                            ))}
                                        </div>
                                        <div className="flex items-center gap-2">
                                            <div className="w-1.5 h-1.5 rounded-full bg-green-400 animate-pulse" />
                                            <span className="text-[9px] font-mono text-green-400">HMAC signature validated</span>
                                        </div>
                                    </div>

                                    <div className="pt-4 border-t border-white/[0.06] space-y-1.5">
                                        <div className="flex items-center gap-2">
                                            <div className="w-2 h-2 rounded-full bg-green-400 animate-pulse" />
                                            <span className="text-xs text-gray-400 font-bold">Available · HMAC</span>
                                        </div>
                                        <p className="text-[11px] font-mono text-gray-600">
                                            /webhooks/github · push · create
                                        </p>
                                    </div>
                                </div>
                            </div>
                        </SectionReveal>
                    </div>

                    {/* Auth note */}
                    <SectionReveal delay={0.3}>
                        <div className="mt-8 glass rounded-xl px-5 py-4 flex items-center gap-3 max-w-2xl mx-auto border border-white/[0.08]">
                            <HiOutlineCode size={16} className="text-brand-400 shrink-0" />
                            <p className="text-xs text-gray-500 leading-relaxed">
                                All integrations communicate with the Control Plane via authenticated REST API.{' '}
                                <span className="text-brand-400 font-semibold">Bearer token · no plaintext secrets in logs.</span>
                            </p>
                        </div>
                    </SectionReveal>
                </Container>
            </section>

            {/* CTA */}
            <CTASection
                title={t('features.cta.title') as string}
                titleAccent={t('features.cta.titleAccent') as string}
                description={t('features.cta.desc') as string}
                primaryCta={{ label: t('features.cta.primary') as string, href: '/download' }}
                secondaryCta={{ label: t('features.cta.secondary') as string, href: '/docs' }}
            />
        </>
    );
}
