'use client';

import React, { useState, useEffect } from 'react';

interface FoxIntroProps { onComplete: () => void; }

const PARTICLES = Array.from({ length: 16 }, (_, i) => {
    const angle = (i / 16) * Math.PI * 2;
    const dist  = 20 + (i % 4) * 7;
    return {
        x:     Math.cos(angle) * dist,
        y:     Math.sin(angle) * dist,
        size:  3 + (i % 3),
        color: i % 2 === 0 ? '#F97316' : '#FBBF24',
        ms:    i * 22,
    };
});

export default function FoxIntro({ onComplete }: FoxIntroProps) {
    // readiness
    const [imgReady, setImgReady] = useState(false);

    // animation states
    const [pulse, setPulse]     = useState(false);   // loader ring pulse
    const [burst, setBurst]     = useState(false);   // rings expand
    const [foxOn, setFoxOn]     = useState(false);   // fox visible
    const [foxIn, setFoxIn]     = useState(false);   // fox settled (scale/brightness)
    const [glowOn, setGlowOn]   = useState(false);
    const [ptOn, setPtOn]       = useState(false);   // particles
    const [out, setOut]         = useState(false);   // fade out

    // (loader ring removed)

    // Preload image, then fire animation
    useEffect(() => {
        let done = false;

        const run = () => {
            if (done) return;
            done = true;
            setImgReady(true);
            setPulse(false);

            // Fox impact: bright + big
            setTimeout(() => { setFoxOn(true); setBurst(true); }, 40);
            // Fox settle: normal brightness + scale
            setTimeout(() => { setFoxIn(true); setGlowOn(true); setPtOn(true); }, 120);
            // Fade out
            setTimeout(() => setOut(true), 1000);
            // Done
            setTimeout(onComplete, 1500);
        };

        const img = new window.Image();
        img.onload  = run;
        img.onerror = run;
        img.src = '/fox.png';

        // Hard fallback: 4s
        const fb = setTimeout(run, 4000);
        return () => clearTimeout(fb);
    }, [onComplete]);

    /* ── helpers ── */

    const ringStyle = (
        active: boolean,
        color: string,
        bw: number,
        dur: number,
        delay: string,
    ): React.CSSProperties => ({
        position:      'absolute',
        top:           '50%',
        left:          '50%',
        transform:     'translate(-50%, -50%)',
        width:         active ? '280vmin' : '5vmin',
        height:        active ? '280vmin' : '5vmin',
        borderRadius:  '50%',
        border:        `${bw}px solid ${color}`,
        opacity:       active ? 0 : 1,
        transition:    active
            ? `width ${dur}ms cubic-bezier(0.04,0,0.12,1) ${delay}, height ${dur}ms cubic-bezier(0.04,0,0.12,1) ${delay}, opacity ${dur}ms ease-out ${delay}`
            : 'none',
        pointerEvents: 'none',
    });

    /* ── render ── */
    return (
        <div style={{
            position:        'fixed',
            inset:           0,
            zIndex:          100,
            backgroundColor: '#000',
            overflow:        'hidden',
            opacity:         out ? 0 : 1,
            transition:      out ? 'opacity 0.45s ease-in' : 'none',
        }}>

            {/* Scanlines */}
            <div style={{
                position:        'absolute',
                inset:           0,
                zIndex:          20,
                pointerEvents:   'none',
                backgroundImage: 'repeating-linear-gradient(0deg,transparent,transparent 2px,rgba(255,255,255,.022) 2px,rgba(255,255,255,.022) 4px)',
            }} />

            {/* ── LOADED: full animation ── */}
            {imgReady && (
                <>
                    {/* Halo outer */}
                    <div style={{
                        position:     'absolute',
                        top:          '50%',
                        left:         '50%',
                        transform:    `translate(-50%, -50%) scale(${glowOn ? 1 : 0.4})`,
                        width:        '150vmin',
                        height:       '150vmin',
                        borderRadius: '50%',
                        background:   'radial-gradient(circle, rgba(249,115,22,0.09) 0%, transparent 62%)',
                        opacity:      glowOn ? 1 : 0,
                        transition:   'opacity 1s ease 0.15s, transform 1.8s ease 0.1s',
                        zIndex:       1,
                        pointerEvents: 'none',
                    }} />

                    {/* Glow core */}
                    <div style={{
                        position:     'absolute',
                        top:          '50%',
                        left:         '50%',
                        transform:    `translate(-50%, -50%) scale(${glowOn ? 1.15 : 0.5})`,
                        width:        '68vmin',
                        height:       '68vmin',
                        borderRadius: '50%',
                        background:   'radial-gradient(circle, rgba(249,115,22,0.48) 0%, rgba(251,191,36,0.2) 38%, transparent 66%)',
                        opacity:      glowOn ? 0.95 : 0,
                        transition:   'opacity 0.5s ease, transform 1.4s ease',
                        zIndex:       2,
                        pointerEvents: 'none',
                    }} />

                    {/* Shockwave rings — cascade */}
                    <div style={ringStyle(burst, 'rgba(249,115,22,0.95)', 2.5, 1500, '0ms')}   />
                    <div style={ringStyle(burst, 'rgba(251,191,36,0.7)',  1.5, 1700, '110ms')} />
                    <div style={ringStyle(burst, 'rgba(249,115,22,0.45)', 1,   1900, '240ms')} />

                    {/* Particles */}
                    {PARTICLES.map((p, i) => (
                        <div key={i} style={{
                            position:        'absolute',
                            top:             '50%',
                            left:            '50%',
                            width:           `${p.size}px`,
                            height:          `${p.size}px`,
                            borderRadius:    '50%',
                            backgroundColor: p.color,
                            boxShadow:       `0 0 ${p.size * 3}px ${p.color}`,
                            transform:       ptOn
                                ? `translate(calc(-50% + ${p.x}vmin), calc(-50% + ${p.y}vmin))`
                                : 'translate(-50%, -50%)',
                            opacity:         ptOn ? 0.88 : 0,
                            transition:      ptOn
                                ? `transform 0.75s cubic-bezier(0.08,0,0.15,1) ${p.ms}ms, opacity 0.38s ease ${p.ms}ms`
                                : 'none',
                            zIndex:          6,
                            pointerEvents:   'none',
                        }} />
                    ))}

                    {/* Fox */}
                    <img
                        src="/fox.png"
                        alt=""
                        draggable={false}
                        style={{
                            position:      'absolute',
                            top:           '50%',
                            left:          '50%',
                            transform:     `translate(-50%, -50%) scale(${foxIn ? 1 : 1.22})`,
                            maxHeight:     '85vh',
                            maxWidth:      '90vw',
                            objectFit:     'contain',
                            opacity:       foxOn ? 1 : 0,
                            filter:        `brightness(${foxIn ? 1 : 3.8})`,
                            transition:    'opacity 0.18s ease, transform 0.7s cubic-bezier(0.12,0,0.08,1), filter 0.65s ease',
                            zIndex:        5,
                            userSelect:    'none',
                            pointerEvents: 'none',
                        }}
                    />
                </>
            )}
        </div>
    );
}
