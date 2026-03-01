'use client';

import React, { useState, useEffect } from 'react';

export function Preloader() {
    const [phase, setPhase] = useState(0);
    // 0 = fox visible + glow fades in, 1 = fade out, 2 = done

    useEffect(() => {
        const t0 = setTimeout(() => setPhase(1), 550);
        const t1 = setTimeout(() => setPhase(2), 850);
        return () => [t0, t1].forEach(clearTimeout);
    }, []);

    if (phase >= 2) return null;

    return (
        <div style={{
            position: 'fixed',
            inset: 0,
            zIndex: 100,
            backgroundColor: '#000',
            overflow: 'hidden',
            opacity: phase >= 1 ? 0 : 1,
            transition: phase >= 1 ? 'opacity 0.45s ease-in' : 'none',
        }}>
            {/* Fox — rendered immediately, no JS preload gate */}
            <img
                src="/fox.png"
                alt=""
                draggable={false}
                style={{
                    position: 'absolute',
                    top: '50%',
                    left: '50%',
                    transform: 'translate(-50%, -50%)',
                    maxHeight: '85vh',
                    maxWidth: '90vw',
                    objectFit: 'contain',
                    zIndex: 5,
                    userSelect: 'none',
                    pointerEvents: 'none',
                }}
            />

            {/* Glow */}
            <div style={{
                position: 'absolute',
                top: '50%',
                left: '50%',
                transform: 'translate(-50%, -50%) scale(1.1)',
                width: '70vmin',
                height: '70vmin',
                borderRadius: '50%',
                background: 'radial-gradient(circle, rgba(249,115,22,0.45) 0%, rgba(251,191,36,0.18) 40%, transparent 68%)',
                opacity: 0.9,
                zIndex: 2,
                pointerEvents: 'none',
            }} />
        </div>
    );
}
