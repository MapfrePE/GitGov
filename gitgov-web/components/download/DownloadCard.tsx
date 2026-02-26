'use client';

import React from 'react';
import { motion } from 'framer-motion';
import { Card } from '@/components/ui/Card';
import { Badge } from '@/components/ui/Badge';
import { Button } from '@/components/ui/Button';
import { SectionReveal } from '@/components/ui/SectionReveal';
import { siteConfig } from '@/lib/config/site';
import { HiOutlineDownload, HiOutlineDesktopComputer, HiOutlineShieldCheck, HiOutlineInformationCircle } from 'react-icons/hi';
import { FaWindows } from 'react-icons/fa';

import { useTranslation } from '@/lib/i18n';

interface DownloadCardProps {
    platform: string;
    icon: React.ReactNode;
    version: string;
    fileName: string;
    downloadPath: string;
    checksum: string;
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
    primary = false,
    available = false,
}: DownloadCardProps) {
    const { t } = useTranslation();
    const [showNotice, setShowNotice] = React.useState(false);
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
                        <Button
                            variant={primary ? 'primary' : 'secondary'}
                            size="lg"
                            href={downloadPath}
                            icon={<HiOutlineDownload size={20} />}
                            className="mt-6 w-full"
                        >
                            {t('download.button')}
                        </Button>
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
                            <span>{t('contact.form.company') === 'Company' ? 'File' : 'Archivo'}</span>
                            <span className="font-mono text-gray-400">{fileName}</span>
                        </div>
                        <div className="flex justify-between">
                            <span>Checksum</span>
                            <span className="font-mono text-gray-400 truncate ml-2">{checksum}</span>
                        </div>
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

