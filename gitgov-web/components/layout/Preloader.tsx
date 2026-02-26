'use client';

import React, { useState, useEffect } from 'react';
import { motion, AnimatePresence } from 'framer-motion';

/**
 * Git-themed preloader — animated commit graph with branching lines
 * and pulsing commit nodes. Disappears after page load.
 */
export function Preloader() {
    const [isLoading, setIsLoading] = useState(true);

    useEffect(() => {
        // Wait for page to fully load, then animate out
        const handleLoad = () => {
            setTimeout(() => setIsLoading(false), 1200);
        };

        if (document.readyState === 'complete') {
            handleLoad();
        } else {
            window.addEventListener('load', handleLoad);
            // Fallback: max 3s load time
            const fallback = setTimeout(() => setIsLoading(false), 3000);
            return () => {
                window.removeEventListener('load', handleLoad);
                clearTimeout(fallback);
            };
        }
    }, []);

    return (
        <AnimatePresence>
            {isLoading && (
                <motion.div
                    className="fixed inset-0 z-[100] flex items-center justify-center bg-surface-300"
                    exit={{ opacity: 0, scale: 1.05 }}
                    transition={{ duration: 0.6, ease: [0.25, 0.4, 0.25, 1] }}
                >
                    {/* Background grid pattern */}
                    <div className="absolute inset-0 opacity-[0.03]"
                        style={{
                            backgroundImage: `linear-gradient(rgba(255,255,255,.1) 1px, transparent 1px),
                               linear-gradient(90deg, rgba(255,255,255,.1) 1px, transparent 1px)`,
                            backgroundSize: '40px 40px',
                        }}
                    />

                    {/* Floating orbs */}
                    <motion.div
                        className="absolute w-64 h-64 rounded-full blur-[100px] bg-brand-500/20"
                        animate={{
                            x: [0, 30, -20, 0],
                            y: [0, -20, 30, 0],
                            scale: [1, 1.1, 0.95, 1],
                        }}
                        transition={{ duration: 4, repeat: Infinity, ease: 'easeInOut' }}
                        style={{ top: '30%', left: '30%' }}
                    />
                    <motion.div
                        className="absolute w-48 h-48 rounded-full blur-[80px] bg-accent-400/15"
                        animate={{
                            x: [0, -30, 20, 0],
                            y: [0, 20, -30, 0],
                            scale: [1, 0.95, 1.1, 1],
                        }}
                        transition={{ duration: 5, repeat: Infinity, ease: 'easeInOut' }}
                        style={{ bottom: '30%', right: '30%' }}
                    />

                    <div className="relative flex flex-col items-center gap-8">
                        {/* Animated Git Graph */}
                        <div className="relative w-40 h-40">
                            <svg viewBox="0 0 160 160" fill="none" className="w-full h-full">
                                {/* Main branch line */}
                                <motion.line
                                    x1="80" y1="15" x2="80" y2="145"
                                    stroke="#00e5da"
                                    strokeWidth="2"
                                    strokeLinecap="round"
                                    initial={{ pathLength: 0, opacity: 0 }}
                                    animate={{ pathLength: 1, opacity: 0.4 }}
                                    transition={{ duration: 1, ease: 'easeInOut' }}
                                />

                                {/* Feature branch */}
                                <motion.path
                                    d="M80 55 Q95 55 110 75 Q125 95 110 115 Q95 135 80 115"
                                    stroke="#ffbb1a"
                                    strokeWidth="2"
                                    strokeLinecap="round"
                                    fill="none"
                                    initial={{ pathLength: 0, opacity: 0 }}
                                    animate={{ pathLength: 1, opacity: 0.4 }}
                                    transition={{ duration: 1.2, delay: 0.3, ease: 'easeInOut' }}
                                />

                                {/* Left branch */}
                                <motion.path
                                    d="M80 35 Q65 35 50 50 Q35 65 50 80"
                                    stroke="#00e5da"
                                    strokeWidth="1.5"
                                    strokeLinecap="round"
                                    fill="none"
                                    initial={{ pathLength: 0, opacity: 0 }}
                                    animate={{ pathLength: 1, opacity: 0.3 }}
                                    transition={{ duration: 1, delay: 0.5, ease: 'easeInOut' }}
                                />

                                {/* Commit nodes */}
                                {[
                                    { cx: 80, cy: 15, delay: 0.2, color: '#00e5da' },
                                    { cx: 80, cy: 55, delay: 0.4, color: '#00e5da' },
                                    { cx: 110, cy: 75, delay: 0.6, color: '#ffbb1a' },
                                    { cx: 50, cy: 50, delay: 0.7, color: '#00e5da' },
                                    { cx: 80, cy: 95, delay: 0.8, color: '#00e5da' },
                                    { cx: 110, cy: 115, delay: 0.9, color: '#ffbb1a' },
                                    { cx: 80, cy: 115, delay: 1.0, color: '#00e5da' },
                                    { cx: 80, cy: 145, delay: 1.1, color: '#00e5da' },
                                ].map((node, i) => (
                                    <React.Fragment key={i}>
                                        {/* Glow */}
                                        <motion.circle
                                            cx={node.cx}
                                            cy={node.cy}
                                            r="8"
                                            fill={node.color}
                                            opacity="0"
                                            initial={{ opacity: 0, scale: 0 }}
                                            animate={{
                                                opacity: [0, 0.2, 0],
                                                scale: [0.5, 1.5, 0.5],
                                            }}
                                            transition={{
                                                duration: 2,
                                                delay: node.delay + 0.5,
                                                repeat: Infinity,
                                                ease: 'easeInOut',
                                            }}
                                        />
                                        {/* Node */}
                                        <motion.circle
                                            cx={node.cx}
                                            cy={node.cy}
                                            r="4"
                                            fill={node.color}
                                            initial={{ scale: 0, opacity: 0 }}
                                            animate={{ scale: 1, opacity: 1 }}
                                            transition={{ duration: 0.4, delay: node.delay }}
                                        />
                                    </React.Fragment>
                                ))}
                            </svg>
                        </div>

                        {/* Brand text */}
                        <motion.div
                            className="flex flex-col items-center gap-2"
                            initial={{ opacity: 0, y: 10 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ delay: 0.5, duration: 0.5 }}
                        >
                            <h1 className="text-2xl font-bold tracking-tight">
                                <span className="text-white">Git</span>
                                <span className="text-brand-400">Gov</span>
                            </h1>
                            <motion.div
                                className="flex items-center gap-1.5"
                                initial={{ opacity: 0 }}
                                animate={{ opacity: 1 }}
                                transition={{ delay: 0.8 }}
                            >
                                <div className="w-1 h-1 rounded-full bg-brand-500 animate-pulse" />
                                <span className="text-xs text-gray-500 font-mono tracking-widest uppercase">
                                    Initializing
                                </span>
                                <motion.span
                                    className="text-xs text-gray-500 font-mono"
                                    animate={{ opacity: [1, 0, 1] }}
                                    transition={{ duration: 1.2, repeat: Infinity }}
                                >
                                    ...
                                </motion.span>
                            </motion.div>
                        </motion.div>
                    </div>
                </motion.div>
            )}
        </AnimatePresence>
    );
}
