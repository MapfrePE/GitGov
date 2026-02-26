'use client';

import React from 'react';
import Link from 'next/link';
import { motion } from 'framer-motion';

interface ButtonProps extends React.ButtonHTMLAttributes<HTMLButtonElement> {
    variant?: 'primary' | 'secondary' | 'ghost' | 'outline';
    size?: 'sm' | 'md' | 'lg';
    asChild?: boolean;
    href?: string;
    icon?: React.ReactNode;
}

const variantStyles = {
    primary:
        'bg-brand-500 text-surface-300 hover:bg-brand-400 shadow-glow hover:shadow-glow-lg font-semibold',
    secondary:
        'glass-card text-brand-300 hover:text-brand-200 hover:border-brand-500/30 font-medium',
    ghost:
        'bg-transparent text-gray-300 hover:text-white hover:bg-white/5 font-medium',
    outline:
        'border border-white/10 text-gray-300 hover:text-white hover:border-white/25 bg-transparent font-medium',
};

const sizes = {
    sm: 'px-4 py-2 text-sm rounded-lg gap-1.5',
    md: 'px-6 py-3 text-base rounded-xl gap-2',
    lg: 'px-8 py-4 text-lg rounded-xl gap-2.5',
};

export function Button({
    variant = 'primary',
    size = 'md',
    href,
    icon,
    children,
    className = '',
    ...props
}: ButtonProps) {
    const classes = `
    inline-flex items-center justify-center
    transition-all duration-300 ease-out
    focus-visible:ring-2 focus-visible:ring-brand-500 focus-visible:ring-offset-2 focus-visible:ring-offset-surface-300
    disabled:opacity-50 disabled:cursor-not-allowed
    active:scale-[0.98]
    ${variantStyles[variant]}
    ${sizes[size]}
    ${className}
  `.trim();

    const content = (
        <>
            {icon && <span className="flex-shrink-0">{icon}</span>}
            {children}
        </>
    );

    if (href) {
        // External links or download paths
        if (href.startsWith('http') || href.startsWith('/downloads/')) {
            return (
                <motion.a
                    href={href}
                    className={classes}
                    whileHover={{ scale: 1.02 }}
                    whileTap={{ scale: 0.98 }}
                >
                    {content}
                </motion.a>
            );
        }

        // Internal navigation via Next.js Link
        return (
            <Link href={href} className={classes}>
                {content}
            </Link>
        );
    }

    return (
        <motion.button
            className={classes}
            whileHover={{ scale: 1.02 }}
            whileTap={{ scale: 0.98 }}
            {...(props as React.ComponentProps<typeof motion.button>)}
        >
            {content}
        </motion.button>
    );
}
