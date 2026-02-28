'use client';

import React from 'react';
import { motion } from 'framer-motion';
import { Card } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { SectionReveal } from '@/components/ui/SectionReveal';
import { siteConfig } from '@/lib/config/site';
import {
    HiOutlineDownload,
    HiOutlineInformationCircle,
    HiOutlineShieldCheck,
    HiOutlineClipboard,
    HiOutlineCheck,
} from 'react-icons/hi';

import { useTranslation } from '@/lib/i18n';

interface DownloadCardProps {
    platform: string;
    icon: React.ReactNode;
    version: string;
    fileName: string;
    downloadPath: string;
    checksum: string;
    msiUrl?: string | null;
    primary?: boolean;
    available?: boolean;
}

export function DownloadCard({
    platform,
    icon,
    version,
    fileName,
    downloadPath,
    checksum,
    msiUrl,
    primary = false,
    available = false,
}: DownloadCardProps) {
    const { t } = useTranslation();
    const [showNotice, setShowNotice] = React.useState(false);
    const [copied, setCopied] = React.useState(false);

    function handleCopyChecksum() {
        navigator.clipboard.writeText(checksum).then(() => {
            setCopied(true);
            setTimeout(() => setCopied(false), 2000);
        });
    }

    return (
        <SectionReveal>
            <Card glow={primary} padding="lg" className={`${primary ? 'border-brand-500/20' : ''}`}>
                <div className="flex flex-col items-center text-center">
                    {/* Platform icon */}
                    <div className={`w-16 h-16 rounded-2xl flex items-center justify-center mb-5 ${primary ? 'bg-brand-500/15 text-brand-400' : 'bg-white/5 text-gray-300'
                        }`}>
                        {icon}
                    </div>

                    {/* Platform name */}
                    <h3 className="text-xl font-semibold text-white mb-2">{platform}</h3>
                    <Badge variant={primary ? 'brand' : 'default'} size="md">v{version}</Badge>

                    {/* Download button */}
                    {available ? (
                        <div className="mt-6 w-full space-y-2">
                            <Button
                                variant={primary ? 'primary' : 'secondary'}
                                size="lg"
                                href={downloadPath}
                                icon={<HiOutlineDownload size={20} />}
                                className="w-full"
                            >
                                {t('download.button')}
                            </Button>
                            {msiUrl && (
                                <Button
                                    variant="secondary"
                                    size="sm"
                                    href={msiUrl}
                                    icon={<HiOutlineDownload size={16} />}
                                    className="w-full"
                                >
                                    {t('download.buttonMsi' as any) as string}
                                </Button>
                            )}
                        </div>
                    ) : (
                        <div className="mt-6 w-full space-y-3">
                            <Button
                                variant={primary ? 'primary' : 'secondary'}
                                size="lg"
                                icon={<HiOutlineDownload size={20} />}
                                className="w-full"
                                onClick={() => setShowNotice(true)}
                            >
                                {t('download.button')}
                            </Button>
                            {showNotice && (
                                <div className="flex items-center gap-2 text-xs text-accent-400 bg-accent-400/10 border border-accent-400/20 rounded-xl px-3 py-2 text-left">
                                    <HiOutlineInformationCircle size={16} className="flex-shrink-0" />
                                    <span>{t('download.notice')}</span>
                                </div>
                            )}
                        </div>
                    )}

                    {/* File info */}
                    <div className="mt-4 space-y-2 text-xs text-gray-500 w-full">
                        <div className="flex justify-between">
                            <span>{t('download.file' as any) as string}</span>
                            <span className="font-mono text-gray-400 truncate ml-2 max-w-[200px]">{fileName}</span>
                        </div>
                        <div className="flex items-center justify-between gap-2">
                            <span className="shrink-0">{t('download.checksum' as any) as string}</span>
                            <div className="flex items-center gap-1 min-w-0">
                                <span className="font-mono text-gray-400 truncate max-w-[160px]">{checksum}</span>
                                <button
                                    type="button"
                                    onClick={handleCopyChecksum}
                                    title={t('download.copyChecksum' as any) as string}
                                    className="shrink-0 p-1 rounded text-gray-500 hover:text-gray-300 transition-colors"
                                    aria-label={t('download.copyChecksum' as any) as string}
                                >
                                    {copied
                                        ? <HiOutlineCheck size={13} className="text-brand-400" />
                                        : <HiOutlineClipboard size={13} />
                                    }
                                </button>
                                {copied && (
                                    <span className="text-brand-400 text-xs whitespace-nowrap">
                                        {t('download.copiedChecksum' as any) as string}
                                    </span>
                                )}
                            </div>
                        </div>
                    </div>
                </div>
            </Card>
        </SectionReveal>
    );
}

interface HashVerifyBlockProps {
    fileName: string;
    checksum: string;
}

export function HashVerifyBlock({ fileName, checksum }: HashVerifyBlockProps) {
    const { t } = useTranslation();
    const hasPendingChecksum = checksum.includes('pending');

    return (
        <SectionReveal delay={0.3}>
            <Card padding="lg" className="mt-8">
                <div className="flex items-start gap-4">
                    <div className="w-10 h-10 rounded-xl bg-brand-500/10 flex items-center justify-center flex-shrink-0">
                        <HiOutlineShieldCheck className="text-brand-400" size={22} />
                    </div>
                    <div className="flex-1 min-w-0">
                        <h4 className="font-semibold text-white mb-3">
                            {t('download.verifyHash.title' as any) as string}
                        </h4>
                        <p className="text-sm text-gray-400 mb-2">
                            {t('download.verifyHash.command' as any) as string}
                        </p>
                        <pre className="text-xs font-mono text-brand-300 bg-brand-500/5 border border-brand-500/10 rounded-lg px-3 py-2 overflow-x-auto">
                            {`Get-FileHash .\\${fileName} -Algorithm SHA256`}
                        </pre>
                        {!hasPendingChecksum && (
                            <>
                                <p className="text-sm text-gray-400 mt-3 mb-2">
                                    {t('download.verifyHash.example' as any) as string}
                                </p>
                                <pre className="text-xs font-mono text-gray-400 bg-white/5 border border-white/5 rounded-lg px-3 py-2 overflow-x-auto break-all">
                                    {checksum.replace('sha256:', '').toUpperCase()}
                                </pre>
                            </>
                        )}
                    </div>
                </div>
            </Card>
        </SectionReveal>
    );
}

export function ReleaseInfo() {
    const { t } = useTranslation();

    return (
        <SectionReveal delay={0.2}>
            <Card padding="lg" className="mt-8">
                <div className="flex items-start gap-4">
                    <div className="w-10 h-10 rounded-xl bg-accent-400/10 flex items-center justify-center flex-shrink-0">
                        <HiOutlineInformationCircle className="text-accent-400" size={22} />
                    </div>
                    <div className="flex-1">
                        <h4 className="font-semibold text-white mb-2">{t('download.installNotes')}</h4>
                        <ul className="space-y-2 text-sm text-gray-400">
                            {[1, 2, 3, 4].map((step) => (
                                <li key={step} className="flex items-start gap-2">
                                    <span className="text-brand-400 mt-1">{step}.</span>
                                    <span dangerouslySetInnerHTML={{ __html: t(`download.step${step}` as any) as string }} />
                                </li>
                            ))}
                        </ul>
                    </div>
                </div>
            </Card>
        </SectionReveal>
    );
}
