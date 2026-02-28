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
                            backgroundImage: `linear-gradient(rgba(0,229,218,0.2) 1px, transparent 1px), linear-gradient(90deg, rgba(0,229,218,0.2) 1px, transparent 1px)`,
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
                            <div
                                className="rounded-2xl p-8 md:p-10 border border-white/5 flex flex-col justify-between"
                                style={{ background: 'linear-gradient(145deg, rgba(0,229,218,0.07), rgba(0,229,218,0.01)), #0d1117' }}
                            >
                                {/* Top */}
                                <div>
                                    <div className="flex items-center gap-2 mb-6">
                                        <div className="w-9 h-9 rounded-xl bg-brand-500/15 flex items-center justify-center text-brand-400">
                                            <HiOutlineShieldCheck size={18} />
                                        </div>
                                        <span className="text-sm font-bold text-white tracking-wide">GitGov</span>
                                    </div>

                                    <h2 className="text-2xl font-bold text-white mb-3">
                                        {t('download.side.heading') as string}
                                    </h2>
                                    <p className="text-gray-500 text-sm leading-relaxed mb-8">
                                        {t('download.side.intro') as string}
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

                                {/* Bottom — system requirements */}
                                <div className="mt-10 pt-6 border-t border-white/5 flex items-center gap-3">
                                    <div className="w-2 h-2 rounded-full bg-brand-400 animate-pulse flex-shrink-0" />
                                    <p className="text-xs text-gray-600">
                                        {t('download.side.sysreq') as string}
                                    </p>
                                </div>
                            </div>

                            {/* Right — Download + Install */}
                            <div className="flex flex-col gap-5">
                                {/* Download card */}
                                <div
                                    className="rounded-2xl p-8 border border-white/5 flex flex-col"
                                    style={{ background: 'linear-gradient(145deg, rgba(255,255,255,0.03), rgba(255,255,255,0.01)), #0d1117' }}
                                >
                                    {/* Platform header */}
                                    <div className="flex flex-col items-center text-center mb-7">
                                        <div className="w-16 h-16 rounded-2xl bg-brand-500/15 border border-brand-500/20 flex items-center justify-center text-brand-400 mb-4">
                                            <FaWindows size={30} />
                                        </div>
                                        <h3 className="text-xl font-bold text-white mb-1.5">Windows</h3>
                                        <span className="inline-flex items-center px-2.5 py-1 rounded-full text-xs font-semibold bg-brand-500/15 text-brand-400 border border-brand-500/25">
                                            v{release.version}
                                        </span>
                                    </div>

                                    {/* Download button */}
                                    {release.available ? (
                                        <div className="space-y-2 mb-6">
                                            <a
                                                href={release.downloadUrl}
                                                className="flex items-center justify-center gap-2 w-full py-3.5 px-5 rounded-xl text-sm font-semibold bg-brand-500 text-surface-300 hover:bg-brand-400 shadow-glow hover:shadow-glow-lg transition-all duration-300"
                                            >
                                                <HiOutlineDownload size={18} />
                                                {t('download.button') as string}
                                            </a>
                                            {release.msiUrl && (
                                                <a
                                                    href={release.msiUrl}
                                                    className="flex items-center justify-center gap-2 w-full py-2.5 px-5 rounded-xl text-xs font-semibold bg-white/5 text-gray-300 border border-white/10 hover:bg-white/10 transition-all duration-300"
                                                >
                                                    <HiOutlineDownload size={14} />
                                                    {t('download.buttonMsi') as string}
                                                </a>
                                            )}
                                        </div>
                                    ) : (
                                        <div className="mb-6">
                                            <div className="flex items-center justify-center gap-2 w-full py-3.5 px-5 rounded-xl text-sm font-semibold bg-white/5 text-gray-500 border border-white/10 cursor-not-allowed">
                                                <HiOutlineDownload size={18} />
                                                {t('download.button') as string}
                                            </div>
                                        </div>
                                    )}

                                    {/* File info */}
                                    <div className="space-y-2.5 text-xs border-t border-white/5 pt-5">
                                        <div className="flex items-center justify-between">
                                            <span className="text-gray-600">{t('download.file') as string}</span>
                                            <span className="font-mono text-gray-400 truncate ml-4 max-w-[200px]">{exeFileName}</span>
                                        </div>
                                        <div className="flex items-center justify-between gap-2">
                                            <span className="text-gray-600 shrink-0">{t('download.checksum') as string}</span>
                                            <div className="flex items-center gap-1 min-w-0">
                                                <span className="font-mono text-gray-400 truncate max-w-[150px]">{release.checksum}</span>
                                                <button
                                                    type="button"
                                                    onClick={handleCopyChecksum}
                                                    title={t('download.copyChecksum') as string}
                                                    className="shrink-0 p-1 rounded text-gray-500 hover:text-gray-300 transition-colors"
                                                    aria-label={t('download.copyChecksum') as string}
                                                >
                                                    {copied
                                                        ? <HiOutlineCheck size={13} className="text-brand-400" />
                                                        : <HiOutlineClipboard size={13} />
                                                    }
                                                </button>
                                                {copied && (
                                                    <span className="text-brand-400 text-xs whitespace-nowrap">
                                                        {t('download.copiedChecksum') as string}
                                                    </span>
                                                )}
                                            </div>
                                        </div>
                                    </div>
                                </div>

                                {/* Other platforms */}
                                <div className="rounded-xl px-5 py-4 border border-white/5 flex items-center justify-between"
                                    style={{ background: 'rgba(255,255,255,0.02)' }}
                                >
                                    <p className="text-xs text-gray-600">
                                        {t('download.otherPlatforms') as string}
                                    </p>
                                    <div className="flex items-center gap-4 shrink-0 ml-4">
                                        <div className="flex items-center gap-1.5 text-gray-700">
                                            <FaApple size={14} />
                                            <span className="text-[10px] font-mono">{t('download.planned') as string}</span>
                                        </div>
                                        <div className="flex items-center gap-1.5 text-gray-700">
                                            <FaLinux size={14} />
                                            <span className="text-[10px] font-mono">{t('download.planned') as string}</span>
                                        </div>
                                    </div>
                                </div>

                                {/* Installation notes */}
                                <div
                                    className="rounded-2xl p-6 border border-white/5"
                                    style={{ background: 'linear-gradient(145deg, rgba(255,255,255,0.03), rgba(255,255,255,0.01)), #0d1117' }}
                                >
                                    <div className="flex items-start gap-3 mb-4">
                                        <div className="w-8 h-8 rounded-lg bg-brand-500/10 border border-brand-500/20 flex items-center justify-center flex-shrink-0">
                                            <HiOutlineInformationCircle className="text-brand-400" size={16} />
                                        </div>
                                        <h4 className="font-semibold text-white text-sm pt-1.5">{t('download.installNotes') as string}</h4>
                                    </div>
                                    <ol className="space-y-2.5 text-xs text-gray-400 pl-1">
                                        {[1, 2, 3, 4].map((step) => (
                                            <li key={step} className="flex items-start gap-2.5">
                                                <span className="text-brand-400 font-bold shrink-0 mt-0.5">{step}.</span>
                                                <span dangerouslySetInnerHTML={{ __html: t(`download.step${step}` as any) as string }} />
                                            </li>
                                        ))}
                                    </ol>

                                    {/* Hash verify */}
                                    {!hasPendingChecksum && (
                                        <div className="mt-5 pt-5 border-t border-white/5">
                                            <div className="flex items-center gap-2 mb-2">
                                                <HiOutlineShieldCheck size={13} className="text-brand-400 shrink-0" />
                                                <p className="text-xs text-gray-500">{t('download.verifyHash.command') as string}</p>
                                            </div>
                                            <pre className="text-[10px] font-mono text-brand-300 bg-brand-500/5 border border-brand-500/10 rounded-lg px-3 py-2 overflow-x-auto">
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
