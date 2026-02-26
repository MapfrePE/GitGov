'use client';

import React from 'react';
import { motion } from 'framer-motion';

interface CardProps {
    children: React.ReactNode;
    className?: string;
    hover?: boolean;
    glow?: boolean;
    padding?: 'sm' | 'md' | 'lg';
}

const paddings = {
    sm: 'p-4',
    md: 'p-6',
    lg: 'p-8',
};

export function Card({
    children,
    className = '',
    hover = true,
    glow = false,
    padding = 'md',
}: CardProps) {
    return (
        <motion.div
            className={`
        glass-card rounded-2xl
        ${paddings[padding]}
        ${hover ? 'transition-all duration-300 hover:border-white/15 hover:-translate-y-1' : ''}
        ${glow ? 'glow-border' : ''}
        ${className}
      `}
            whileHover={hover ? { y: -4 } : undefined}
        >
            {children}
        </motion.div>
    );
}
