'use client';

import React, { useMemo } from 'react';
import { motion, useScroll, useTransform } from 'framer-motion';
import { Container } from '@/components/layout/Container';
import { Button } from '@/components/ui/Button';
import { Badge } from '@/components/ui/Badge';
import { siteConfig } from '@/lib/config/site';
import { HiOutlineArrowDown, HiOutlineDownload } from 'react-icons/hi';
import { useTranslation } from '@/lib/i18n';

/* ────────────────────────────────────
   3-D wireframe sphere (SVG)
   ──────────────────────────────────── */

function generateSpherePoints(count: number, radius: number) {
    const points: { x: number; y: number; z: number }[] = [];
    const goldenAngle = Math.PI * (3 - Math.sqrt(5));
    for (let i = 0; i < count; i++) {
        const y = 1 - (i / (count - 1)) * 2;
        const radiusAtY = Math.sqrt(1 - y * y);
        const theta = goldenAngle * i;
        points.push({
            x: Math.cos(theta) * radiusAtY * radius,
            y: y * radius,
            z: Math.sin(theta) * radiusAtY * radius,
        });
    }
    return points;
}

function NetworkSphere() {
    const cx = 280;
    const cy = 280;
    const R = 230;
    const nodeCount = 60;

    const { points, edges } = useMemo(() => {
        const pts = generateSpherePoints(nodeCount, R);
        const edgeList: [number, number][] = [];
        const maxDist = R * 0.7;
        for (let i = 0; i < pts.length; i++) {
            for (let j = i + 1; j < pts.length; j++) {
                const dx = pts[i].x - pts[j].x;
                const dy = pts[i].y - pts[j].y;
                const dz = pts[i].z - pts[j].z;
                const dist = Math.sqrt(dx * dx + dy * dy + dz * dz);
                if (dist < maxDist) edgeList.push([i, j]);
            }
        }
        return { points: pts, edges: edgeList };
    }, []);

    // project 3D → 2D (simple perspective)
    const focalLength = 600;
    const project = (p: { x: number; y: number; z: number }) => {
        const scale = focalLength / (focalLength + p.z);
        return { x: cx + p.x * scale, y: cy + p.y * scale, scale };
    };

    const projected = points.map(project);

    return (
        <div className="relative w-full h-full flex items-center justify-center">
            {/* Orbital rings */}
            <motion.div
                className="absolute"
                style={{ width: R * 2 + 60, height: R * 2 + 60 }}
                animate={{ rotate: 360 }}
                transition={{ duration: 60, repeat: Infinity, ease: 'linear' }}
            >
                <svg width="100%" height="100%" viewBox={`0 0 ${R * 2 + 60} ${R * 2 + 60}`}>
                    <ellipse
                        cx={(R * 2 + 60) / 2} cy={(R * 2 + 60) / 2}
                        rx={R + 25} ry={R * 0.35}
                        fill="none" stroke="rgba(0,229,218,0.08)" strokeWidth="1"
                    />
                </svg>
            </motion.div>
            <motion.div
                className="absolute"
                style={{ width: R * 2 + 100, height: R * 2 + 100 }}
                animate={{ rotate: -360 }}
                transition={{ duration: 90, repeat: Infinity, ease: 'linear' }}
            >
                <svg width="100%" height="100%" viewBox={`0 0 ${R * 2 + 100} ${R * 2 + 100}`}>
                    <ellipse
                        cx={(R * 2 + 100) / 2} cy={(R * 2 + 100) / 2}
                        rx={R + 45} ry={R * 0.25}
                        fill="none" stroke="rgba(255,187,26,0.06)" strokeWidth="1"
                        transform={`rotate(25 ${(R * 2 + 100) / 2} ${(R * 2 + 100) / 2})`}
                    />
                </svg>
            </motion.div>

            {/* Main sphere SVG */}
            <motion.svg
                viewBox={`0 0 ${cx * 2} ${cy * 2}`}
                className="w-full h-full max-w-[560px]"
                animate={{ rotate: [0, 360] }}
                transition={{ duration: 120, repeat: Infinity, ease: 'linear' }}
            >
                {/* Edges */}
                {edges.map(([i, j], idx) => {
                    const a = projected[i];
                    const b = projected[j];
                    const avgZ = (points[i].z + points[j].z) / 2;
                    const opacity = 0.04 + ((avgZ + R) / (2 * R)) * 0.12;
                    return (
                        <line
                            key={`e-${idx}`}
                            x1={a.x} y1={a.y} x2={b.x} y2={b.y}
                            stroke="rgba(0,229,218,1)"
                            strokeWidth="0.5"
                            opacity={opacity}
                        />
                    );
                })}

                {/* Nodes */}
                {projected.map((p, i) => {
                    const depth = (points[i].z + R) / (2 * R); // 0 = back, 1 = front
                    const nodeOpacity = 0.15 + depth * 0.6;
                    const nodeRadius = 1.2 + depth * 2;
                    const isBright = i % 7 === 0;
                    return (
                        <React.Fragment key={`n-${i}`}>
                            {isBright && (
                                <circle
                                    cx={p.x} cy={p.y} r={nodeRadius * 3}
                                    fill={i % 14 === 0 ? 'rgba(255,187,26,0.1)' : 'rgba(0,229,218,0.1)'}
                                    opacity={nodeOpacity * 0.5}
                                />
                            )}
                            <circle
                                cx={p.x} cy={p.y} r={nodeRadius}
                                fill={isBright ? (i % 14 === 0 ? '#ffbb1a' : '#00e5da') : 'rgba(255,255,255,0.6)'}
                                opacity={nodeOpacity}
                            />
                        </React.Fragment>
                    );
                })}
            </motion.svg>

            {/* Center glow */}
            <div
                className="absolute w-[300px] h-[300px] rounded-full blur-[80px] opacity-[0.06]"
                style={{
                    background: 'radial-gradient(circle, rgba(0,229,218,0.6) 0%, transparent 70%)',
                }}
            />
        </div>
    );
}

/* ────────────────────────────────────
   Hero Section
   ──────────────────────────────────── */

export function Hero() {
    const { scrollY } = useScroll();
    const opacity = useTransform(scrollY, [0, 400], [1, 0]);
    const scale = useTransform(scrollY, [0, 400], [1, 0.97]);
    const { t } = useTranslation();

    return (
        <section className="relative min-h-screen flex items-center overflow-hidden" id="hero">
            {/* Background grid */}
            <div className="absolute inset-0">
                <div
                    className="absolute inset-0 opacity-[0.04]"
                    style={{
                        backgroundImage: `
              linear-gradient(rgba(0, 229, 218, 0.15) 1px, transparent 1px),
              linear-gradient(90deg, rgba(0, 229, 218, 0.15) 1px, transparent 1px)
            `,
                        backgroundSize: '60px 60px',
                    }}
                />

                {/* Faint horizontal lines */}
                <motion.div
                    className="absolute top-[20%] left-0 w-full h-px"
                    style={{
                        background: 'linear-gradient(90deg, transparent 0%, rgba(0,229,218,0.08) 30%, rgba(0,229,218,0.15) 50%, rgba(0,229,218,0.08) 70%, transparent 100%)',
                    }}
                    animate={{ opacity: [0.3, 0.6, 0.3] }}
                    transition={{ duration: 4, repeat: Infinity, ease: 'easeInOut' }}
                />
                <motion.div
                    className="absolute bottom-[25%] left-0 w-full h-px"
                    style={{
                        background: 'linear-gradient(90deg, transparent 0%, rgba(255,187,26,0.06) 40%, rgba(255,187,26,0.1) 50%, rgba(255,187,26,0.06) 60%, transparent 100%)',
                    }}
                    animate={{ opacity: [0.2, 0.4, 0.2] }}
                    transition={{ duration: 6, repeat: Infinity, ease: 'easeInOut', delay: 2 }}
                />

                {/* Small floating squares (like in the reference) */}
                {[
                    { top: '12%', right: '8%', delay: 0 },
                    { top: '88%', left: '35%', delay: 1 },
                    { top: '75%', right: '15%', delay: 2.5 },
                    { bottom: '5%', right: '5%', delay: 1.5 },
                ].map((pos, i) => (
                    <motion.div
                        key={i}
                        className="absolute w-1.5 h-1.5 border border-white/10 rotate-45"
                        style={pos}
                        animate={{ opacity: [0.2, 0.5, 0.2], scale: [1, 1.3, 1] }}
                        transition={{ duration: 4 + i, repeat: Infinity, ease: 'easeInOut', delay: pos.delay }}
                    />
                ))}
            </div>

            {/* Content — split layout */}
            <motion.div style={{ opacity, scale }} className="relative z-10 w-full">
                <Container>
                    <div className="flex flex-col lg:flex-row items-center gap-12 lg:gap-8 pt-28 pb-16 md:pt-36 md:pb-24">
                        {/* Left: Text */}
                        <div className="flex-1 text-center lg:text-left max-w-2xl lg:max-w-xl">
                            {/* Version badge */}
                            <motion.div
                                initial={{ opacity: 0, y: 20 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ delay: 0.2, duration: 0.6 }}
                            >
                                <Badge variant="brand" size="md">
                                    <span className="flex items-center gap-2">
                                        <span className="w-1.5 h-1.5 rounded-full bg-brand-400 animate-pulse" />
                                        v{siteConfig.version} — {t('hero.badge')}
                                    </span>
                                </Badge>
                            </motion.div>

                            {/* Main heading */}
                            <motion.h1
                                className="mt-8 text-4xl sm:text-5xl md:text-display font-bold tracking-tight leading-[1.1]"
                                initial={{ opacity: 0, y: 30 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ delay: 0.35, duration: 0.7, ease: [0.25, 0.4, 0.25, 1] }}
                            >
                                <span className="text-white">{t('hero.title1')}</span>
                                <br />
                                <span className="gradient-text">{t('hero.title2')}</span>
                            </motion.h1>

                            {/* Subtitle */}
                            <motion.p
                                className="mt-6 text-lg md:text-xl text-gray-400 leading-relaxed"
                                initial={{ opacity: 0, y: 20 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ delay: 0.5, duration: 0.6 }}
                            >
                                {t('hero.subtitle')}
                            </motion.p>

                            {/* CTA Buttons */}
                            <motion.div
                                className="mt-10 flex flex-col sm:flex-row items-center lg:items-start gap-4"
                                initial={{ opacity: 0, y: 20 }}
                                animate={{ opacity: 1, y: 0 }}
                                transition={{ delay: 0.65, duration: 0.6 }}
                            >
                                <Button
                                    variant="primary"
                                    size="lg"
                                    href="/download"
                                    icon={<HiOutlineDownload size={20} />}
                                >
                                    {t('hero.cta')}
                                </Button>
                                <Button
                                    variant="secondary"
                                    size="lg"
                                    href="/docs"
                                >
                                    {t('hero.ctaSecondary')}
                                </Button>
                            </motion.div>

                            {/* Quick stats */}
                            <motion.div
                                className="mt-14 flex flex-wrap items-center justify-center lg:justify-start gap-x-10 gap-y-4"
                                initial={{ opacity: 0 }}
                                animate={{ opacity: 1 }}
                                transition={{ delay: 0.85, duration: 0.6 }}
                            >
                                {[
                                    { label: t('hero.stat.traceability'), value: t('hero.stat.full') },
                                    { label: t('hero.stat.correlation'), value: 'Jenkins' },
                                    { label: t('hero.stat.audit'), value: t('hero.stat.immutable') },
                                ].map((stat) => (
                                    <div key={stat.label as string} className="flex items-center gap-3">
                                        <span className="text-sm font-semibold text-brand-400">{stat.value}</span>
                                        <span className="text-sm text-gray-500">{stat.label}</span>
                                    </div>
                                ))}
                            </motion.div>
                        </div>

                        {/* Right: 3D Network Sphere */}
                        <motion.div
                            className="flex-1 w-full max-w-[560px] aspect-square"
                            initial={{ opacity: 0, scale: 0.85 }}
                            animate={{ opacity: 1, scale: 1 }}
                            transition={{ delay: 0.4, duration: 1, ease: [0.25, 0.4, 0.25, 1] }}
                        >
                            <NetworkSphere />
                        </motion.div>
                    </div>

                    {/* Scroll indicator — centered */}
                    <motion.div
                        className="flex justify-center pb-8"
                        animate={{ y: [0, 8, 0] }}
                        transition={{ duration: 2, repeat: Infinity, ease: 'easeInOut' }}
                    >
                        <HiOutlineArrowDown className="text-gray-600" size={20} />
                    </motion.div>
                </Container>
            </motion.div>
        </section>
    );
}

