'use client';

import React from 'react';
import { Container } from '@/components/layout';
import { SectionHeader } from '@/components/marketing';
import { DownloadCard, ReleaseInfo } from '@/components/download';
import { SectionReveal } from '@/components/ui';
import { siteConfig } from '@/lib/config/site';
import { FaWindows, FaApple, FaLinux } from 'react-icons/fa';
import { useTranslation } from '@/lib/i18n';

interface DownloadClientProps {
    windowsRelease: {
        available: boolean;
        checksum: string;
    };
}

export function DownloadClient({ windowsRelease }: DownloadClientProps) {
    const { t } = useTranslation();

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

            {/* Download Cards */}
            <section className="pb-20">
                <Container size="narrow">
                    <div className="grid md:grid-cols-1 gap-6 max-w-md mx-auto">
                        <DownloadCard
                            platform="Windows"
                            icon={<FaWindows size={32} />}
                            version={siteConfig.version}
                            fileName={siteConfig.downloadFileName}
                            downloadPath={siteConfig.downloadPath}
                            checksum={windowsRelease.checksum}
                            available={windowsRelease.available}
                            primary
                        />
                    </div>

                    {/* Other platforms */}
                    <SectionReveal delay={0.3}>
                        <div className="mt-8 text-center">
                            <p className="text-sm text-gray-500">
                                {t('download.otherPlatforms')}
                            </p>
                            <div className="flex items-center justify-center gap-6 mt-4">
                                <div className="flex items-center gap-2 text-gray-600">
                                    <FaApple size={18} />
                                    <span className="text-xs">{t('download.planned')}</span>
                                </div>
                                <div className="flex items-center gap-2 text-gray-600">
                                    <FaLinux size={18} />
                                    <span className="text-xs">{t('download.planned')}</span>
                                </div>
                            </div>
                        </div>
                    </SectionReveal>

                    {/* Installation Notes */}
                    <ReleaseInfo />
                </Container>
            </section>
        </>
    );
}
