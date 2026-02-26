import React from 'react';

interface BadgeProps {
    children: React.ReactNode;
    variant?: 'default' | 'brand' | 'accent' | 'success' | 'outline';
    size?: 'sm' | 'md';
    className?: string;
}

const badgeVariants = {
    default: 'bg-white/5 text-gray-300 border-white/10',
    brand: 'bg-brand-500/10 text-brand-400 border-brand-500/20',
    accent: 'bg-accent-400/10 text-accent-400 border-accent-400/20',
    success: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20',
    outline: 'bg-transparent text-gray-400 border-white/15',
};

const badgeSizes = {
    sm: 'px-2 py-0.5 text-xs',
    md: 'px-3 py-1 text-sm',
};

export function Badge({
    children,
    variant = 'default',
    size = 'sm',
    className = '',
}: BadgeProps) {
    return (
        <span
            className={`
        inline-flex items-center rounded-full border font-medium
        ${badgeVariants[variant]}
        ${badgeSizes[size]}
        ${className}
      `}
        >
            {children}
        </span>
    );
}
