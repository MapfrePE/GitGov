'use client';

import React, { useRef, useEffect, useCallback, useState } from 'react';
import { motion } from 'framer-motion';
import { Container } from '@/components/layout/Container';
import { Button } from '@/components/ui/Button';
import { Badge } from '@/components/ui/Badge';
import { siteConfig } from '@/lib/config/site';
import { HiOutlineArrowDown, HiOutlineDownload } from 'react-icons/hi';
import { useTranslation } from '@/lib/i18n';

/* ═══════════════════════════════════════════════
   GOVERNANCE UNIVERSE — Canvas-based 3D Engine
   60fps · Mouse-interactive · Particle systems
   ═══════════════════════════════════════════════ */

interface Vec3 {
    x: number;
    y: number;
    z: number;
}

interface SphereNode extends Vec3 {
    type: 'commit' | 'ci' | 'ticket' | 'audit' | 'default';
    pulsePhase: number;
}

interface FlowParticle {
    fromIdx: number;
    toIdx: number;
    progress: number;
    speed: number;
    color: string;
    trail: { x: number; y: number; alpha: number }[];
}

interface StarParticle {
    x: number;
    y: number;
    size: number;
    alpha: number;
    speed: number;
    twinkleSpeed: number;
    twinklePhase: number;
}

interface PulseRing {
    x: number;
    y: number;
    radius: number;
    maxRadius: number;
    alpha: number;
    color: string;
    speed: number;
}

const COLORS = {
    commit: { r: 0, g: 229, b: 218 },
    ci: { r: 34, g: 197, b: 94 },
    ticket: { r: 59, g: 130, b: 246 },
    audit: { r: 168, g: 85, b: 247 },
    default: { r: 255, g: 255, b: 255 },
    amber: { r: 255, g: 187, b: 26 },
};

function colorStr(c: { r: number; g: number; b: number }, a: number) {
    return `rgba(${c.r},${c.g},${c.b},${a})`;
}

function isFiniteNumber(value: number): boolean {
    return Number.isFinite(value);
}

function generateSphereNodes(count: number, radius: number): SphereNode[] {
    const nodes: SphereNode[] = [];
    const goldenAngle = Math.PI * (3 - Math.sqrt(5));
    for (let i = 0; i < count; i++) {
        const y = 1 - (i / (count - 1)) * 2;
        const radiusAtY = Math.sqrt(1 - y * y);
        const theta = goldenAngle * i;
        let type: SphereNode['type'] = 'default';
        if (i % 9 === 0) type = 'ci';
        else if (i % 11 === 0) type = 'ticket';
        else if (i % 14 === 0) type = 'audit';
        else if (i % 4 === 0) type = 'commit';
        nodes.push({
            x: Math.cos(theta) * radiusAtY * radius,
            y: y * radius,
            z: Math.sin(theta) * radiusAtY * radius,
            type,
            pulsePhase: Math.random() * Math.PI * 2,
        });
    }
    return nodes;
}

function buildEdges(nodes: Vec3[], maxDist: number): [number, number][] {
    const edges: [number, number][] = [];
    for (let i = 0; i < nodes.length; i++) {
        for (let j = i + 1; j < nodes.length; j++) {
            const dx = nodes[i].x - nodes[j].x;
            const dy = nodes[i].y - nodes[j].y;
            const dz = nodes[i].z - nodes[j].z;
            if (dx * dx + dy * dy + dz * dz < maxDist * maxDist) {
                edges.push([i, j]);
            }
        }
    }
    return edges;
}

function rotateY(p: Vec3, angle: number): Vec3 {
    const cos = Math.cos(angle);
    const sin = Math.sin(angle);
    return { x: p.x * cos + p.z * sin, y: p.y, z: -p.x * sin + p.z * cos };
}

function rotateX(p: Vec3, angle: number): Vec3 {
    const cos = Math.cos(angle);
    const sin = Math.sin(angle);
    return { x: p.x, y: p.y * cos - p.z * sin, z: p.y * sin + p.z * cos };
}

function GovernanceCanvas() {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const mouseRef = useRef({ x: 0.5, y: 0.5 });
    const frameRef = useRef(0);
    const dataRef = useRef<{
        nodes: SphereNode[];
        edges: [number, number][];
        stars: StarParticle[];
        flowParticles: FlowParticle[];
        pulseRings: PulseRing[];
        specialEdges: number[];
    } | null>(null);
    const [isReady, setIsReady] = useState(false);

    // Initialize data once
    useEffect(() => {
        const R = 200;
        const nodes = generateSphereNodes(140, R);
        const edges = buildEdges(nodes, R * 0.55);

        // Star field
        const stars: StarParticle[] = Array.from({ length: 250 }, () => ({
            x: Math.random(),
            y: Math.random(),
            size: 0.3 + Math.random() * 1.5,
            alpha: 0.1 + Math.random() * 0.5,
            speed: 0.0001 + Math.random() * 0.0003,
            twinkleSpeed: 0.5 + Math.random() * 2,
            twinklePhase: Math.random() * Math.PI * 2,
        }));

        // Flow particles along edges
        const specialEdges = edges
            .map((_, i) => i)
            .filter(() => Math.random() < 0.06);

        const flowParticles: FlowParticle[] = Array.from(
            { length: 30 },
            (_, i) => {
                const edgeIdx =
                    specialEdges[i % specialEdges.length] || Math.floor(Math.random() * edges.length);
                const colors = ['#00e5da', '#22c55e', '#3b82f6', '#a855f7', '#ffbb1a'];
                return {
                    fromIdx: edges[edgeIdx][0],
                    toIdx: edges[edgeIdx][1],
                    progress: Math.random(),
                    speed: 0.003 + Math.random() * 0.008,
                    color: colors[i % colors.length],
                    trail: [],
                };
            }
        );

        dataRef.current = {
            nodes,
            edges,
            stars,
            flowParticles,
            pulseRings: [],
            specialEdges,
        };
        setIsReady(true);
    }, []);

    const handleMouseMove = useCallback((e: MouseEvent) => {
        const canvas = canvasRef.current;
        if (!canvas) return;
        const rect = canvas.getBoundingClientRect();
        mouseRef.current = {
            x: (e.clientX - rect.left) / rect.width,
            y: (e.clientY - rect.top) / rect.height,
        };
    }, []);

    // Animation loop
    useEffect(() => {
        if (!isReady) return;
        const canvas = canvasRef.current;
        if (!canvas) return;
        const ctx = canvas.getContext('2d');
        if (!ctx) return;

        let animId: number;

        const render = () => {
            const data = dataRef.current;
            if (!data) return;

            const dpr = window.devicePixelRatio || 1;
            const rect = canvas.getBoundingClientRect();
            const W = rect.width;
            const H = rect.height;
            if (!isFiniteNumber(W) || !isFiniteNumber(H) || W <= 1 || H <= 1 || !isFiniteNumber(dpr) || dpr <= 0) {
                animId = requestAnimationFrame(render);
                return;
            }

            if (canvas.width !== W * dpr || canvas.height !== H * dpr) {
                canvas.width = W * dpr;
                canvas.height = H * dpr;
            }
            // Always reset transform before drawing to avoid cumulative scaling issues.
            ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

            ctx.clearRect(0, 0, W, H);
            const frame = frameRef.current++;
            const time = frame * 0.016;

            const mx = mouseRef.current.x;
            const my = mouseRef.current.y;
            const cx = W / 2;
            const cy = H / 2;
            const R = Math.min(W, H) * 0.34;
            const focalLength = R * 3.2;
            if (!isFiniteNumber(cx) || !isFiniteNumber(cy) || !isFiniteNumber(R) || R <= 0 || !isFiniteNumber(focalLength) || focalLength <= 0) {
                animId = requestAnimationFrame(render);
                return;
            }

            // ═══ STAR FIELD ═══
            for (const star of data.stars) {
                const twinkle =
                    0.3 +
                    0.7 *
                        Math.abs(
                            Math.sin(time * star.twinkleSpeed + star.twinklePhase)
                        );
                const sx = star.x * W;
                const sy = star.y * H;
                ctx.beginPath();
                ctx.arc(sx, sy, star.size, 0, Math.PI * 2);
                ctx.fillStyle = colorStr(COLORS.default, star.alpha * twinkle);
                ctx.fill();
            }

            // ═══ ROTATION (auto + mouse tilt) ═══
            const autoRotY = time * 0.15;
            const mouseTiltY = (mx - 0.5) * 0.4;
            const mouseTiltX = (my - 0.5) * 0.25;
            const totalRotY = autoRotY + mouseTiltY;
            const totalRotX = mouseTiltX;

            // Project nodes
            const projected: { x: number; y: number; depth: number; scale: number }[] = [];
            for (const node of data.nodes) {
                let p = rotateY(node, totalRotY);
                p = rotateX(p, totalRotX);
                const scale = focalLength / (focalLength + p.z);
                if (!isFiniteNumber(scale)) {
                    projected.push({ x: cx, y: cy, depth: 0, scale: 0 });
                    continue;
                }
                projected.push({
                    x: cx + p.x * scale,
                    y: cy + p.y * scale,
                    depth: (p.z + R) / (2 * R),
                    scale,
                });
            }

            // ═══ ORBITAL RINGS ═══
            const rings = [
                { tilt: 0.3, phase: time * 0.2, r: R * 1.3, color: COLORS.commit, alpha: 0.07, dash: [6, 8] },
                { tilt: -0.22, phase: -time * 0.15 + 1, r: R * 1.45, color: COLORS.amber, alpha: 0.04, dash: [4, 6] },
                { tilt: 0.5, phase: time * 0.12 + 2, r: R * 1.6, color: COLORS.audit, alpha: 0.035, dash: [3, 7] },
            ];

            for (const ring of rings) {
                ctx.save();
                ctx.translate(cx, cy);
                ctx.setLineDash(ring.dash);
                ctx.strokeStyle = colorStr(ring.color, ring.alpha);
                ctx.lineWidth = 1;
                ctx.beginPath();
                const steps = 120;
                for (let s = 0; s <= steps; s++) {
                    const angle = (s / steps) * Math.PI * 2 + ring.phase;
                    let rp: Vec3 = {
                        x: Math.cos(angle) * ring.r,
                        y: 0,
                        z: Math.sin(angle) * ring.r,
                    };
                    rp = rotateX(rp, ring.tilt);
                    rp = rotateY(rp, mouseTiltY * 0.3);
                    const rScale = focalLength / (focalLength + rp.z);
                    const rx = rp.x * rScale;
                    const ry = rp.y * rScale;
                    if (s === 0) ctx.moveTo(rx, ry);
                    else ctx.lineTo(rx, ry);
                }
                ctx.stroke();
                ctx.setLineDash([]);
                ctx.restore();
            }

            // ═══ EDGES ═══
            for (const [i, j] of data.edges) {
                const a = projected[i];
                const b = projected[j];
                const avgDepth = (a.depth + b.depth) / 2;
                const alpha = 0.02 + avgDepth * 0.1;
                const nodeI = data.nodes[i];
                const nodeJ = data.nodes[j];
                const edgeType =
                    nodeI.type !== 'default' ? nodeI.type : nodeJ.type !== 'default' ? nodeJ.type : 'commit';
                const c = COLORS[edgeType] || COLORS.commit;
                ctx.beginPath();
                ctx.moveTo(a.x, a.y);
                ctx.lineTo(b.x, b.y);
                ctx.strokeStyle = colorStr(c, alpha);
                ctx.lineWidth = 0.5 + avgDepth * 0.5;
                ctx.stroke();
            }

            // ═══ NODES ═══
            for (let i = 0; i < projected.length; i++) {
                const p = projected[i];
                const node = data.nodes[i];
                const c = COLORS[node.type] || COLORS.default;
                const isSpecial = node.type !== 'default';
                const baseAlpha = 0.1 + p.depth * 0.7;
                const pulse = isSpecial
                    ? 0.7 + 0.3 * Math.sin(time * 2 + node.pulsePhase)
                    : 1;
                const nodeR = isSpecial ? 1.5 + p.depth * 3 : 0.8 + p.depth * 1.5;
                if (!isFiniteNumber(p.x) || !isFiniteNumber(p.y) || !isFiniteNumber(nodeR) || nodeR <= 0) {
                    continue;
                }

                // Outer glow for special nodes
                if (isSpecial) {
                    const glowR = nodeR * 5;
                    if (!isFiniteNumber(glowR) || glowR <= 0) {
                        continue;
                    }
                    const grad = ctx.createRadialGradient(
                        p.x, p.y, 0,
                        p.x, p.y, glowR
                    );
                    grad.addColorStop(0, colorStr(c, baseAlpha * 0.25 * pulse));
                    grad.addColorStop(1, colorStr(c, 0));
                    ctx.beginPath();
                    ctx.arc(p.x, p.y, glowR, 0, Math.PI * 2);
                    ctx.fillStyle = grad;
                    ctx.fill();
                }

                // Core node
                ctx.beginPath();
                ctx.arc(p.x, p.y, nodeR, 0, Math.PI * 2);
                ctx.fillStyle = colorStr(c, baseAlpha * pulse);
                ctx.fill();

                // Bright center for special
                if (isSpecial) {
                    ctx.beginPath();
                    ctx.arc(p.x, p.y, nodeR * 0.5, 0, Math.PI * 2);
                    ctx.fillStyle = colorStr({ r: 255, g: 255, b: 255 }, baseAlpha * 0.5 * pulse);
                    ctx.fill();
                }
            }

            // ═══ FLOW PARTICLES with trails ═══
            for (const fp of data.flowParticles) {
                fp.progress += fp.speed;
                if (fp.progress > 1) {
                    fp.progress = 0;
                    // Pick a new random edge
                    const edgeIdx = Math.floor(Math.random() * data.edges.length);
                    fp.fromIdx = data.edges[edgeIdx][0];
                    fp.toIdx = data.edges[edgeIdx][1];
                    fp.trail = [];
                }

                const from = projected[fp.fromIdx];
                const to = projected[fp.toIdx];
                if (!from || !to) continue;

                const px = from.x + (to.x - from.x) * fp.progress;
                const py = from.y + (to.y - from.y) * fp.progress;
                const avgDepth = (from.depth + to.depth) / 2;
                if (!isFiniteNumber(px) || !isFiniteNumber(py) || !isFiniteNumber(avgDepth)) {
                    continue;
                }

                // Update trail
                fp.trail.push({ x: px, y: py, alpha: 0.8 });
                if (fp.trail.length > 12) fp.trail.shift();

                // Draw trail
                for (let t = 0; t < fp.trail.length; t++) {
                    const tp = fp.trail[t];
                    tp.alpha *= 0.88;
                    ctx.beginPath();
                    ctx.arc(tp.x, tp.y, 1 + (t / fp.trail.length) * 1.5, 0, Math.PI * 2);
                    ctx.fillStyle = fp.color.replace(')', `,${tp.alpha * avgDepth})`).replace('rgb', 'rgba');
                    // Direct color with alpha
                    const alphaVal = tp.alpha * avgDepth * 0.6;
                    if (!isFiniteNumber(alphaVal) || !isFiniteNumber(tp.x) || !isFiniteNumber(tp.y)) continue;
                    ctx.fillStyle = fp.color + Math.round(alphaVal * 255).toString(16).padStart(2, '0');
                    ctx.fill();
                }

                // Head
                ctx.beginPath();
                ctx.arc(px, py, 2 + avgDepth, 0, Math.PI * 2);
                ctx.fillStyle = fp.color;
                ctx.globalAlpha = avgDepth * 0.8;
                ctx.fill();
                ctx.globalAlpha = 1;
            }

            // ═══ PULSE RINGS ═══
            // Emit new pulse rings from random special nodes
            if (frame % 90 === 0) {
                const specialIndices = data.nodes
                    .map((n, i) => (n.type !== 'default' ? i : -1))
                    .filter((i) => i >= 0);
                if (specialIndices.length > 0) {
                    const idx = specialIndices[Math.floor(Math.random() * specialIndices.length)];
                    const p = projected[idx];
                    const node = data.nodes[idx];
                    const c = COLORS[node.type];
                    data.pulseRings.push({
                        x: p.x,
                        y: p.y,
                        radius: 3,
                        maxRadius: 30 + Math.random() * 25,
                        alpha: 0.5,
                        color: colorStr(c, 1),
                        speed: 0.4 + Math.random() * 0.3,
                    });
                }
            }

            // Draw and update pulse rings
            for (let r = data.pulseRings.length - 1; r >= 0; r--) {
                const ring = data.pulseRings[r];
                ring.radius += ring.speed;
                ring.alpha *= 0.97;
                if (ring.alpha < 0.01 || ring.radius > ring.maxRadius) {
                    data.pulseRings.splice(r, 1);
                    continue;
                }
                if (!isFiniteNumber(ring.x) || !isFiniteNumber(ring.y) || !isFiniteNumber(ring.radius)) {
                    data.pulseRings.splice(r, 1);
                    continue;
                }
                ctx.beginPath();
                ctx.arc(ring.x, ring.y, ring.radius, 0, Math.PI * 2);
                ctx.strokeStyle = ring.color.replace(/[\d.]+\)$/, `${ring.alpha})`);
                ctx.lineWidth = 1.5;
                ctx.stroke();
            }

            // ═══ CONNECTION BEAMS to governance labels ═══
            // Find the closest special node of each type to draw beams
            const beamTargets = [
                { type: 'ci' as const, targetX: W * 0.92, targetY: H * 0.12 },
                { type: 'ticket' as const, targetX: W * 0.9, targetY: H * 0.82 },
                { type: 'audit' as const, targetX: W * 0.08, targetY: H * 0.35 },
                { type: 'commit' as const, targetX: W * 0.1, targetY: H * 0.72 },
            ];

            for (const beam of beamTargets) {
                // Find best node of this type (most visible / frontmost)
                let bestIdx = -1;
                let bestDepth = -1;
                for (let i = 0; i < data.nodes.length; i++) {
                    if (data.nodes[i].type === beam.type && projected[i].depth > bestDepth) {
                        bestDepth = projected[i].depth;
                        bestIdx = i;
                    }
                }
                if (bestIdx < 0) continue;

                const from = projected[bestIdx];
                const c = COLORS[beam.type];
                const beamAlpha = 0.06 + bestDepth * 0.08;

                // Dashed line from node to label area
                ctx.save();
                ctx.setLineDash([3, 6]);
                ctx.beginPath();
                ctx.moveTo(from.x, from.y);
                ctx.lineTo(beam.targetX, beam.targetY);
                ctx.strokeStyle = colorStr(c, beamAlpha);
                ctx.lineWidth = 1;
                ctx.stroke();
                ctx.setLineDash([]);
                ctx.restore();

                // Small dot at the beam endpoint
                ctx.beginPath();
                ctx.arc(beam.targetX, beam.targetY, 2, 0, Math.PI * 2);
                ctx.fillStyle = colorStr(c, beamAlpha * 2);
                ctx.fill();
            }

            // ═══ AMBIENT FLOATING PARTICLES ═══
            for (let i = 0; i < 15; i++) {
                const angle = time * (0.1 + i * 0.03) + (i * Math.PI * 2) / 15;
                const dist = R * (1.1 + 0.4 * Math.sin(time * 0.3 + i));
                let fp3: Vec3 = {
                    x: Math.cos(angle) * dist,
                    y: Math.sin(angle * 0.7 + i) * dist * 0.3,
                    z: Math.sin(angle) * dist,
                };
                fp3 = rotateY(fp3, totalRotY * 0.5);
                const fScale = focalLength / (focalLength + fp3.z);
                if (!isFiniteNumber(fScale)) continue;
                const fpx = cx + fp3.x * fScale;
                const fpy = cy + fp3.y * fScale;
                const fpAlpha =
                    0.15 + 0.2 * Math.sin(time + i * 0.5);
                if (!isFiniteNumber(fpx) || !isFiniteNumber(fpy) || !isFiniteNumber(fpAlpha)) continue;
                const colors = [COLORS.commit, COLORS.ci, COLORS.ticket, COLORS.audit, COLORS.amber];
                const c = colors[i % colors.length];
                ctx.beginPath();
                ctx.arc(fpx, fpy, 1 + fScale * 0.5, 0, Math.PI * 2);
                ctx.fillStyle = colorStr(c, fpAlpha);
                ctx.fill();
            }

            animId = requestAnimationFrame(render);
        };

        animId = requestAnimationFrame(render);
        window.addEventListener('mousemove', handleMouseMove);

        return () => {
            cancelAnimationFrame(animId);
            window.removeEventListener('mousemove', handleMouseMove);
        };
    }, [isReady, handleMouseMove]);

    return (
        <div className="relative w-full h-full">
            <canvas
                ref={canvasRef}
                className="w-full h-full"
                style={{ display: 'block' }}
            />

            {/* ═══ FLOATING GOVERNANCE LABELS (HTML overlay) ═══ */}

            {/* Jenkins CI — top right */}
            <motion.div
                className="absolute top-[6%] right-[2%]"
                initial={{ opacity: 0, x: 30, scale: 0.8 }}
                animate={{ opacity: 1, x: 0, scale: 1 }}
                transition={{ delay: 1.2, duration: 0.8, type: 'spring' }}
            >
                <motion.div
                    className="glass-card rounded-xl px-4 py-2.5 border border-green-500/20 bg-green-950/30 backdrop-blur-md shadow-[0_0_20px_rgba(34,197,94,0.1)]"
                    animate={{ y: [0, -8, 0] }}
                    transition={{ duration: 5, repeat: Infinity, ease: 'easeInOut' }}
                >
                    <div className="flex items-center gap-2.5">
                        <div className="relative">
                            <span className="w-2 h-2 rounded-full bg-green-400 block" />
                            <span className="absolute inset-0 w-2 h-2 rounded-full bg-green-400 animate-ping opacity-40" />
                        </div>
                        <div>
                            <div className="text-[10px] text-green-400/60 font-mono uppercase tracking-wider">Pipeline</div>
                            <div className="text-xs font-mono font-semibold text-green-400">BUILD PASSING</div>
                        </div>
                    </div>
                </motion.div>
            </motion.div>

            {/* Jira Ticket — bottom right */}
            <motion.div
                className="absolute bottom-[10%] right-[4%]"
                initial={{ opacity: 0, x: 30, scale: 0.8 }}
                animate={{ opacity: 1, x: 0, scale: 1 }}
                transition={{ delay: 1.5, duration: 0.8, type: 'spring' }}
            >
                <motion.div
                    className="glass-card rounded-xl px-4 py-2.5 border border-blue-500/20 bg-blue-950/30 backdrop-blur-md shadow-[0_0_20px_rgba(59,130,246,0.1)]"
                    animate={{ y: [0, 6, 0] }}
                    transition={{ duration: 6, repeat: Infinity, ease: 'easeInOut', delay: 1 }}
                >
                    <div className="flex items-center gap-2.5">
                        <span className="w-2 h-2 rounded-full bg-blue-400" />
                        <div>
                            <div className="text-[10px] text-blue-400/60 font-mono uppercase tracking-wider">Jira</div>
                            <div className="text-xs font-mono font-semibold text-blue-400">GOV-1247 LINKED</div>
                        </div>
                    </div>
                </motion.div>
            </motion.div>

            {/* Audit Trail — left */}
            <motion.div
                className="absolute top-[28%] left-[0%]"
                initial={{ opacity: 0, x: -30, scale: 0.8 }}
                animate={{ opacity: 1, x: 0, scale: 1 }}
                transition={{ delay: 1.8, duration: 0.8, type: 'spring' }}
            >
                <motion.div
                    className="glass-card rounded-xl px-4 py-2.5 border border-purple-500/20 bg-purple-950/30 backdrop-blur-md shadow-[0_0_20px_rgba(168,85,247,0.1)]"
                    animate={{ y: [0, -5, 0] }}
                    transition={{ duration: 7, repeat: Infinity, ease: 'easeInOut', delay: 2 }}
                >
                    <div className="flex items-center gap-2.5">
                        <div className="relative">
                            <span className="w-2 h-2 rounded-full bg-purple-400 block" />
                            <span className="absolute inset-0 w-2 h-2 rounded-full bg-purple-400 animate-ping opacity-30" />
                        </div>
                        <div>
                            <div className="text-[10px] text-purple-400/60 font-mono uppercase tracking-wider">Audit</div>
                            <div className="text-xs font-mono font-semibold text-purple-400">IMMUTABLE LOG</div>
                        </div>
                    </div>
                </motion.div>
            </motion.div>

            {/* Commit hash — bottom left */}
            <motion.div
                className="absolute bottom-[20%] left-[2%]"
                initial={{ opacity: 0, x: -30, scale: 0.8 }}
                animate={{ opacity: 1, x: 0, scale: 1 }}
                transition={{ delay: 2.1, duration: 0.8, type: 'spring' }}
            >
                <motion.div
                    className="glass-card rounded-xl px-4 py-2.5 border border-brand-500/20 bg-brand-950/30 backdrop-blur-md shadow-[0_0_20px_rgba(0,229,218,0.1)]"
                    animate={{ y: [0, 5, 0] }}
                    transition={{ duration: 5.5, repeat: Infinity, ease: 'easeInOut', delay: 0.5 }}
                >
                    <div className="flex items-center gap-2.5">
                        <span className="w-2 h-2 rounded-full bg-brand-400" />
                        <div>
                            <div className="text-[10px] text-brand-400/60 font-mono uppercase tracking-wider">Commit</div>
                            <div className="text-xs font-mono font-semibold text-brand-400">a3f8c2e → main</div>
                        </div>
                    </div>
                </motion.div>
            </motion.div>

            {/* Policy Check — top left */}
            <motion.div
                className="absolute top-[55%] right-[0%]"
                initial={{ opacity: 0, x: 30, scale: 0.8 }}
                animate={{ opacity: 1, x: 0, scale: 1 }}
                transition={{ delay: 2.4, duration: 0.8, type: 'spring' }}
            >
                <motion.div
                    className="glass-card rounded-xl px-4 py-2.5 border border-amber-500/20 bg-amber-950/30 backdrop-blur-md shadow-[0_0_20px_rgba(255,187,26,0.08)]"
                    animate={{ y: [0, -4, 0] }}
                    transition={{ duration: 4.5, repeat: Infinity, ease: 'easeInOut', delay: 3 }}
                >
                    <div className="flex items-center gap-2.5">
                        <span className="w-2 h-2 rounded-full bg-amber-400" />
                        <div>
                            <div className="text-[10px] text-amber-400/60 font-mono uppercase tracking-wider">Policy</div>
                            <div className="text-xs font-mono font-semibold text-amber-400">COMPLIANT</div>
                        </div>
                    </div>
                </motion.div>
            </motion.div>

            {/* Center glows */}
            <div
                className="absolute inset-0 pointer-events-none"
                style={{
                    background:
                        'radial-gradient(ellipse at center, rgba(0,229,218,0.06) 0%, transparent 50%)',
                }}
            />
            <div
                className="absolute inset-0 pointer-events-none"
                style={{
                    background:
                        'radial-gradient(ellipse at 60% 30%, rgba(168,85,247,0.03) 0%, transparent 40%)',
                }}
            />
        </div>
    );
}

/* ════════════════════════════════════
   Hero Section
   ════════════════════════════════════ */

export function Hero() {
    const { t } = useTranslation();

    return (
        <section
            className="relative overflow-hidden"
            id="hero"
        >
            {/* Background grid */}
            <div className="absolute inset-0">
                <div
                    className="absolute inset-0 opacity-[0.03]"
                    style={{
                        backgroundImage: `
              linear-gradient(rgba(0, 229, 218, 0.2) 1px, transparent 1px),
              linear-gradient(90deg, rgba(0, 229, 218, 0.2) 1px, transparent 1px)
            `,
                        backgroundSize: '60px 60px',
                    }}
                />
            </div>

            {/* Content — split layout */}
            <div className="relative z-10 w-full">
                <Container>
                    <div className="flex flex-col lg:flex-row items-center gap-6 lg:gap-4 pt-24 pb-8 md:pt-32 md:pb-16">
                        {/* Left: Text */}
                        <div className="flex-1 text-center lg:text-left max-w-2xl lg:max-w-xl relative z-20">
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
                                transition={{
                                    delay: 0.35,
                                    duration: 0.7,
                                    ease: [0.25, 0.4, 0.25, 1],
                                }}
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
                                <Button variant="secondary" size="lg" href="/docs">
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
                                    {
                                        label: t('hero.stat.traceability'),
                                        value: t('hero.stat.full'),
                                    },
                                    {
                                        label: t('hero.stat.correlation'),
                                        value: 'Jenkins',
                                    },
                                    {
                                        label: t('hero.stat.audit'),
                                        value: t('hero.stat.immutable'),
                                    },
                                ].map((stat) => (
                                    <div
                                        key={stat.label as string}
                                        className="flex items-center gap-3"
                                    >
                                        <span className="text-sm font-semibold text-brand-400">
                                            {stat.value}
                                        </span>
                                        <span className="text-sm text-gray-500">
                                            {stat.label}
                                        </span>
                                    </div>
                                ))}
                            </motion.div>
                        </div>

                        {/* Right: Canvas 3D Governance Universe */}
                        <motion.div
                            className="flex-1 w-full max-w-[620px] aspect-square relative"
                            initial={{ opacity: 0, scale: 0.8 }}
                            animate={{ opacity: 1, scale: 1 }}
                            transition={{
                                delay: 0.3,
                                duration: 1.2,
                                ease: [0.25, 0.4, 0.25, 1],
                            }}
                        >
                            <GovernanceCanvas />
                        </motion.div>
                    </div>

                    {/* Scroll indicator */}
                    <motion.div
                        className="flex justify-center pb-8"
                        animate={{ y: [0, 8, 0] }}
                        transition={{ duration: 2, repeat: Infinity, ease: 'easeInOut' }}
                    >
                        <HiOutlineArrowDown className="text-gray-600" size={20} />
                    </motion.div>
                </Container>
            </div>
        </section>
    );
}
