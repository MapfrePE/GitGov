'use client';

import React from 'react';
import { Container } from '@/components/layout';
import { SectionHeader } from '@/components/marketing';
import { SectionReveal } from '@/components/ui';
import { siteConfig } from '@/lib/config/site';
import { FaWindows, FaApple, FaLinux } from 'react-icons/fa';
import {
    HiOutlineDownload,
    HiOutlineShieldCheck,
    HiOutlineClipboard,
    HiOutlineCheck,
    HiOutlineInformationCircle,
    HiOutlineLightningBolt,
    HiOutlineWifi,
    HiOutlineDesktopComputer,
    HiOutlineClipboardCheck,
} from 'react-icons/hi';
import { useTranslation } from '@/lib/i18n';
import type { ReleaseMetadata } from '@/lib/release';

interface DownloadClientProps {
    release: ReleaseMetadata;
}

export function DownloadClient({ release }: DownloadClientProps) {
    const { t } = useTranslation();
    const [copied, setCopied] = React.useState(false);

    const exeFileName =
        release.downloadUrl.split('/').pop() ?? siteConfig.downloadFileName;

    function handleCopyChecksum() {
        navigator.clipboard.writeText(release.checksum).then(() => {
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        });
    }

    const highlights = [
        {
            icon: <HiOutlineLightningBolt size={20} />,
            title: t('download.side.h1title') as string,
            desc: t('download.side.h1desc') as string,
        },
        {
            icon: <HiOutlineWifi size={20} />,
            title: t('download.side.h2title') as string,
            desc: t('download.side.h2desc') as string,
        },
        {
            icon: <HiOutlineDesktopComputer size={20} />,
            title: t('download.side.h3title') as string,
            desc: t('download.side.h3desc') as string,
        },
        {
            icon: <HiOutlineClipboardCheck size={20} />,
            title: t('download.side.h4title') as string,
            desc: t('download.side.h4desc') as string,
        },
    ];

    const hasPendingChecksum = release.checksum.includes('pending');

    return (
        <>
            {/* Hero */}
            <section className="pt-32 md:pt-40 pb-16 relative overflow-hidden">
                <div className="absolute inset-0">
                    <div
                        className="absolute inset-0 opacity-[0.03]"
                        style={{
                            backgroundImage: `linear-gradient(rgba(249,115,22,0.2) 1px, transparent 1px), linear-gradient(90deg, rgba(249,115,22,0.2) 1px, transparent 1px)`,
                            backgroundSize: '40px 40px',
                        }}
                    />
                </div>
                <Container>
                    <SectionHeader
                        badge={t('download.badge') as string}
                        title={t('download.title') as string}
                        titleAccent={t('download.titleAccent') as string}
                        description={t('download.description') as string}
                    />
                </Container>
            </section>

            {/* Split Layout */}
            <section className="pb-28">
                <Container>
                    <SectionReveal>
                        <div className="max-w-5xl mx-auto grid md:grid-cols-2 gap-8 items-stretch">

                            {/* Left — Info panel */}
                            <div className="relative group rounded-3xl p-[1px] overflow-hidden bg-gradient-to-b from-white/10 to-transparent transition-all duration-500 hover:from-white/20">
                                <div className="absolute inset-[1px] rounded-[23px] bg-gradient-to-br from-surface-400 to-surface-500 opacity-95 z-0" />
                                <div className="absolute inset-0 opacity-0 group-hover:opacity-100 transition-opacity duration-700 bg-[radial-gradient(circle_at_50%_0%,rgba(255,255,255,0.03),transparent_60%)] z-0" />
                                <div className="relative h-full rounded-[23px] p-8 md:p-10 flex flex-col justify-between shadow-2xl z-10">
                                    {/* Top */}
                                    <div>
                                        <div className="flex items-center gap-3 mb-8">
                                            <div className="w-10 h-10 rounded-xl bg-surface-300 border border-white/5 flex items-center justify-center text-white shadow-inner">
                                                <HiOutlineShieldCheck size={20} />
                                            </div>
                                            <span className="text-sm font-bold text-white tracking-widest uppercase">GitGov</span>
                                        </div>

                                        <h2 className="text-3xl font-bold font-sans text-white mb-4 tracking-tight">
                                            {t('download.side.heading') as string}
                                        </h2>
                                        <p className="text-gray-400 text-sm leading-relaxed mb-10">
                                            {t('download.side.intro') as string}
                                        </p>

                                        {/* Highlights */}
                                        <div className="space-y-6">
                                            {highlights.map((h, i) => (
                                                <div key={i} className="flex items-start gap-4 group/item">
                                                    <div className="w-10 h-10 rounded-xl bg-surface-300 border border-white/5 flex items-center justify-center text-brand-400 flex-shrink-0 transition-colors group-hover/item:border-brand-500/30 group-hover/item:text-brand-300">
                                                        {h.icon}
                                                    </div>
                                                    <div className="pt-0.5">
                                                        <p className="text-sm font-bold text-white mb-1 tracking-wide">{h.title}</p>
                                                        <p className="text-xs text-gray-400 leading-relaxed font-medium">{h.desc}</p>
                                                    </div>
                                                </div>
                                            ))}
                                        </div>
                                    </div>

                                    {/* Bottom — system requirements */}
                                    <div className="mt-12 pt-6 border-t border-white/5 flex items-center gap-3">
                                        <div className="relative flex items-center justify-center w-2 h-2">
                                            <div className="absolute w-2 h-2 rounded-full bg-brand-500 animate-ping opacity-75" />
                                            <div className="relative w-2 h-2 rounded-full bg-brand-500 shadow-[0_0_8px_rgba(249,115,22,0.8)]" />
                                        </div>
                                        <p className="text-xs font-mono text-gray-500 uppercase tracking-widest">
                                            {t('download.side.sysreq') as string}
                                        </p>
                                    </div>
                                </div>
                            </div>

                            {/* Right — Download + Install */}
                            <div className="flex flex-col gap-6">
                                {/* Download card */}
                                <div className="relative rounded-3xl p-[1px] bg-gradient-to-b from-brand-500/30 via-white/5 to-transparent overflow-hidden shadow-[0_0_40px_rgba(249,115,22,0.05)] group">
                                    <div className="absolute inset-[1px] rounded-[23px] bg-surface-400/90 backdrop-blur-xl z-0" />
                                    <div className="absolute inset-0 bg-[radial-gradient(circle_at_50%_0%,rgba(249,115,22,0.08),transparent_60%)] opacity-50 group-hover:opacity-100 transition-opacity duration-500 z-0" />
                                    <div className="relative h-full rounded-[23px] p-8 md:p-10 flex flex-col z-10">
                                        {/* Platform header */}
                                        <div className="flex flex-col items-center text-center mb-8">
                                            <div className="w-20 h-20 rounded-2xl bg-gradient-to-b from-surface-200 to-surface-400 border border-white/10 flex items-center justify-center text-white mb-5 shadow-xl relative overflow-hidden group/icon">
                                                <div className="absolute inset-0 bg-brand-500/10 opacity-0 group-hover/icon:opacity-100 transition-opacity duration-300" />
                                                <FaWindows size={36} className="relative z-10" />
                                            </div>
                                            <h3 className="text-2xl font-bold font-sans text-white mb-2 tracking-tight">Windows</h3>
                                            <span className="inline-flex items-center px-3 py-1 rounded-full text-xs font-bold font-mono bg-surface-200 text-brand-400 border border-brand-500/20 shadow-inner">
                                                v{release.version}
                                            </span>
                                        </div>

                                        {/* Download button */}
                                        {release.available ? (
                                            <div className="space-y-3 mb-8">
                                                <a
                                                    href={release.downloadUrl}
                                                    className="group/btn relative flex items-center justify-center gap-3 w-full py-4 px-6 rounded-xl font-bold text-white overflow-hidden transition-all duration-300 shadow-[0_0_20px_rgba(249,115,22,0.2)] hover:shadow-[0_0_30px_rgba(249,115,22,0.4)]"
                                                >
                                                    <div className="absolute inset-0 bg-brand-500 transition-transform duration-300 group-hover/btn:scale-105" />
                                                    <div className="absolute inset-0 bg-gradient-to-b from-white/20 to-transparent opacity-50" />
                                                    <HiOutlineDownload size={20} className="relative z-10" />
                                                    <span className="relative z-10">{t('download.button') as string}</span>
                                                </a>
                                                {release.msiUrl && (
                                                    <a
                                                        href={release.msiUrl}
                                                        className="flex items-center justify-center gap-2 w-full py-3 px-6 rounded-xl text-xs font-bold bg-surface-300 text-white border border-white/5 hover:bg-surface-200 hover:border-white/10 transition-all duration-300"
                                                    >
                                                        <HiOutlineDownload size={16} />
                                                        {t('download.buttonMsi') as string}
                                                    </a>
                                                )}
                                            </div>
                                        ) : (
                                            <div className="mb-8">
                                                <div className="flex items-center justify-center gap-3 w-full py-4 px-6 rounded-xl font-bold bg-surface-300 text-gray-500 border border-white/5 cursor-not-allowed">
                                                    <HiOutlineDownload size={20} />
                                                    {t('download.button') as string}
                                                </div>
                                            </div>
                                        )}

                                        {/* File info */}
                                        <div className="space-y-4 text-xs border-t border-white/5 pt-6 mt-auto">
                                            <div className="flex items-center justify-between">
                                                <span className="text-gray-500 font-semibold uppercase tracking-wider">{t('download.file') as string}</span>
                                                <span className="font-mono text-gray-300 truncate ml-4 max-w-[200px] bg-surface-300 px-2 py-1 rounded-md border border-white/5">{exeFileName}</span>
                                            </div>
                                            <div className="flex items-center justify-between gap-2">
                                                <span className="text-gray-500 font-semibold shrink-0 uppercase tracking-wider">{t('download.checksum') as string}</span>
                                                <div className="flex items-center gap-1.5 min-w-0 bg-surface-300 pl-2 pr-1 py-1 rounded-md border border-white/5">
                                                    <span className="font-mono text-gray-300 truncate max-w-[140px]">{release.checksum}</span>
                                                    <button
                                                        type="button"
                                                        onClick={handleCopyChecksum}
                                                        title={t('download.copyChecksum') as string}
                                                        className="shrink-0 p-1.5 rounded bg-surface-200 text-gray-400 hover:text-white hover:bg-surface-100 transition-all shadow-sm"
                                                        aria-label={t('download.copyChecksum') as string}
                                                    >
                                                        {copied
                                                            ? <HiOutlineCheck size={14} className="text-brand-400" />
                                                            : <HiOutlineClipboard size={14} />
                                                        }
                                                    </button>
                                                </div>
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                {/* Other platforms */}
                                <div className="rounded-2xl px-6 py-5 bg-surface-400 border border-white/5 flex items-center justify-between shadow-lg hover:border-white/10 transition-colors">
                                    <p className="text-xs font-semibold text-gray-400 uppercase tracking-widest">
                                        {t('download.otherPlatforms') as string}
                                    </p>
                                    <div className="flex items-center gap-5 shrink-0 ml-4">
                                        <div className="flex items-center gap-2 text-gray-600">
                                            <FaApple size={16} />
                                            <span className="text-[10px] font-mono font-bold">{t('download.planned') as string}</span>
                                        </div>
                                        <div className="flex items-center gap-2 text-gray-600">
                                            <FaLinux size={16} />
                                            <span className="text-[10px] font-mono font-bold">{t('download.planned') as string}</span>
                                        </div>
                                    </div>
                                </div>

                                {/* Installation notes */}
                                <div className="rounded-2xl p-8 bg-surface-400 border border-white/5 shadow-lg relative overflow-hidden group">
                                    <div className="absolute top-0 right-0 w-40 h-40 bg-brand-500/5 rounded-full blur-3xl -mr-20 -mt-20 pointer-events-none transition-all duration-700 group-hover:bg-brand-500/10" />
                                    <div className="flex items-center gap-4 mb-6 relative z-10">
                                        <div className="w-10 h-10 rounded-xl bg-surface-300 border border-white/5 flex items-center justify-center flex-shrink-0 shadow-inner">
                                            <HiOutlineInformationCircle className="text-white" size={20} />
                                        </div>
                                        <h4 className="font-bold font-sans text-white text-base tracking-tight">{t('download.installNotes') as string}</h4>
                                    </div>
                                    <ol className="space-y-4 text-sm text-gray-400 relative z-10">
                                        {[1, 2, 3, 4].map((step) => (
                                            <li key={step} className="flex items-start gap-4">
                                                <span className="flex items-center justify-center w-6 h-6 rounded-full bg-surface-300 border border-white/10 text-brand-400 font-bold text-[10px] shrink-0 mt-0.5 shadow-inner">{step}</span>
                                                <span className="leading-relaxed pt-0.5" dangerouslySetInnerHTML={{ __html: t(`download.step${step}` as any) as string }} />
                                            </li>
                                        ))}
                                    </ol>

                                    {/* Hash verify */}
                                    {!hasPendingChecksum && (
                                        <div className="mt-6 pt-6 border-t border-white/5 relative z-10">
                                            <div className="flex items-center gap-2 mb-3">
                                                <HiOutlineShieldCheck size={16} className="text-brand-400 shrink-0" />
                                                <p className="text-xs font-semibold text-gray-400 uppercase tracking-widest">{t('download.verifyHash.command') as string}</p>
                                            </div>
                                            <pre className="text-xs font-mono text-gray-300 bg-surface-300 shadow-inner border border-white/5 rounded-xl px-4 py-3 overflow-x-auto">
                                                {`Get-FileHash .\\${exeFileName} -Algorithm SHA256`}
                                            </pre>
                                        </div>
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
