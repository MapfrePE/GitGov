import React from 'react';

interface ContainerProps {
    children: React.ReactNode;
    className?: string;
    as?: React.ElementType;
    size?: 'default' | 'wide' | 'narrow';
}

const maxWidths = {
    default: 'max-w-7xl',
    wide: 'max-w-[1440px]',
    narrow: 'max-w-4xl',
};

export function Container({
    children,
    className = '',
    as: Component = 'div',
    size = 'default',
}: ContainerProps) {
    return (
        <Component className={`mx-auto px-5 sm:px-6 lg:px-8 ${maxWidths[size]} ${className}`}>
            {children}
        </Component>
    );
}
