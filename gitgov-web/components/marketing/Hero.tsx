'use client';

import React, { useRef, useEffect, useCallback, useState } from 'react';
import { motion } from 'framer-motion';
import { Container } from '@/components/layout/Container';
import { Badge } from '@/components/ui/Badge';
import { siteConfig } from '@/lib/config/site';
import { HiOutlineArrowDown, HiOutlineDownload, HiOutlinePlay } from 'react-icons/hi';
import { useTranslation } from '@/lib/i18n';

/* ═══════════════════════════════════════════════════════
   GOVERNANCE NEXUS — Canvas 3D Engine v3
   Clean canvas-only (no HTML inside), directed flow
   ═══════════════════════════════════════════════════════ */

interface Vec3 { x: number; y: number; z: number }
interface SphereNode extends Vec3 {
    type: 'commit' | 'ci' | 'ticket' | 'audit' | 'default';
    pulsePhase: number;
}
interface FlowParticle {
    fromIdx: number; toIdx: number; progress: number; speed: number;
    color: string; trail: { x: number; y: number; alpha: number }[];
}
interface PulseRing {
    x: number; y: number; radius: number; maxRadius: number;
    alpha: number; color: string; speed: number;
}
interface NexusBurst { radius: number; maxRadius: number; alpha: number; color: string }

const COLORS = {
    commit: { r: 0, g: 229, b: 218 },
    ci: { r: 34, g: 197, b: 94 },
    ticket: { r: 59, g: 130, b: 246 },
    audit: { r: 168, g: 85, b: 247 },
    default: { r: 180, g: 195, b: 210 },
    amber: { r: 255, g: 187, b: 26 },
};
function clamp(v: number) { return Math.max(0, Math.min(1, v)); }
function colorStr(c: { r: number; g: number; b: number }, a: number) {
    return `rgba(${c.r},${c.g},${c.b},${clamp(a)})`;
}
function isOk(v: number) { return Number.isFinite(v); }

function generateSphereNodes(count: number, radius: number): SphereNode[] {
    const nodes: SphereNode[] = [];
    const ga = Math.PI * (3 - Math.sqrt(5));
    for (let i = 0; i < count; i++) {
        const y = 1 - (i / (count - 1)) * 2;
        const ry = Math.sqrt(1 - y * y);
        const th = ga * i;
        let type: SphereNode['type'] = 'default';
        if (i % 9 === 0) type = 'ci';
        else if (i % 11 === 0) type = 'ticket';
        else if (i % 14 === 0) type = 'audit';
        else if (i % 4 === 0) type = 'commit';
        nodes.push({ x: Math.cos(th) * ry * radius, y: y * radius, z: Math.sin(th) * ry * radius, type, pulsePhase: Math.random() * Math.PI * 2 });
    }
    return nodes;
}
function buildEdges(nodes: Vec3[], d: number): [number, number][] {
    const edges: [number, number][] = [];
    for (let i = 0; i < nodes.length; i++)
        for (let j = i + 1; j < nodes.length; j++) {
            const dx = nodes[i].x - nodes[j].x, dy = nodes[i].y - nodes[j].y, dz = nodes[i].z - nodes[j].z;
            if (dx * dx + dy * dy + dz * dz < d * d) edges.push([i, j]);
        }
    return edges;
}
function rotY(p: Vec3, a: number): Vec3 { const c = Math.cos(a), s = Math.sin(a); return { x: p.x * c + p.z * s, y: p.y, z: -p.x * s + p.z * c }; }
function rotX(p: Vec3, a: number): Vec3 { const c = Math.cos(a), s = Math.sin(a); return { x: p.x, y: p.y * c - p.z * s, z: p.y * s + p.z * c }; }

/* ─── Canvas-only component ─── */
function GovernanceCanvas() {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const mouseRef = useRef({ x: 0.5, y: 0.5 });
    const frameRef = useRef(0);
    const dataRef = useRef<{
        nodes: SphereNode[]; edges: [number, number][];
        flow: FlowParticle[]; pulseRings: PulseRing[]; bursts: NexusBurst[];
    } | null>(null);
    const [ready, setReady] = useState(false);

    useEffect(() => {
        const R = 200;
        const nodes = generateSphereNodes(130, R);
        const edges = buildEdges(nodes, R * 0.52);
        const cols = ['#00e5da', '#22c55e', '#3b82f6', '#a855f7', '#ffbb1a'];
        const flow: FlowParticle[] = Array.from({ length: 38 }, (_, i) => {
            const ei = Math.floor(Math.random() * edges.length);
            return { fromIdx: edges[ei][0], toIdx: edges[ei][1], progress: Math.random(), speed: 0.004 + Math.random() * 0.009, color: cols[i % cols.length], trail: [] };
        });
        dataRef.current = { nodes, edges, flow, pulseRings: [], bursts: [] };
        setReady(true);
    }, []);

    const onMouse = useCallback((e: MouseEvent) => {
        const cv = canvasRef.current; if (!cv) return;
        const r = cv.getBoundingClientRect();
        mouseRef.current = { x: (e.clientX - r.left) / r.width, y: (e.clientY - r.top) / r.height };
    }, []);

    useEffect(() => {
        if (!ready) return;
        const cv = canvasRef.current; if (!cv) return;
        const ctx = cv.getContext('2d'); if (!ctx) return;
        let animId: number;

        const draw = () => {
            const data = dataRef.current; if (!data) return;
            const dpr = window.devicePixelRatio || 1;
            const { width: W, height: H } = cv.getBoundingClientRect();
            if (!isOk(W) || !isOk(H) || W <= 1 || H <= 1) { animId = requestAnimationFrame(draw); return; }
            if (cv.width !== W * dpr || cv.height !== H * dpr) { cv.width = W * dpr; cv.height = H * dpr; }
            ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
            ctx.clearRect(0, 0, W, H);

            const frame = frameRef.current++;
            const t = frame * 0.016;
            const cx = W / 2, cy = H / 2;
            const R = Math.min(W, H) * 0.38;
            const fL = R * 3.2;
            if (!isOk(R) || R <= 0) { animId = requestAnimationFrame(draw); return; }

            const mx = mouseRef.current.x;
            const my = mouseRef.current.y;
            const rotYA = t * 0.13 + (mx - 0.5) * 0.4;
            const rotXA = (my - 0.5) * 0.22;

            // Project
            const proj: { x: number; y: number; depth: number; scale: number }[] = [];
            for (const n of data.nodes) {
                let p = rotY(n, rotYA); p = rotX(p, rotXA);
                const s = fL / (fL + p.z);
                if (!isOk(s)) { proj.push({ x: cx, y: cy, depth: 0, scale: 0 }); continue; }
                proj.push({ x: cx + p.x * s, y: cy + p.y * s, depth: (p.z + R) / (2 * R), scale: s });
            }

            // [1] Deep glow
            {
                const pulse = 0.5 + 0.5 * Math.sin(t * 0.7);
                const g = ctx.createRadialGradient(cx, cy, 0, cx, cy, R * 1.7);
                g.addColorStop(0, `rgba(0,229,218,${0.07 + pulse * 0.05})`);
                g.addColorStop(0.5, `rgba(0,229,218,${0.02})`);
                g.addColorStop(1, 'rgba(0,0,0,0)');
                ctx.fillStyle = g; ctx.fillRect(0, 0, W, H);
            }

            // [2] Orbital rings
            for (const ring of [
                { tilt: 0.30, phase: t * 0.20, r: R * 1.28, c: COLORS.commit, a: 0.14, d: [6, 8] as [number, number] },
                { tilt: -0.22, phase: -t * 0.15 + 1, r: R * 1.44, c: COLORS.amber, a: 0.09, d: [4, 7] as [number, number] },
                { tilt: 0.50, phase: t * 0.10 + 2, r: R * 1.62, c: COLORS.audit, a: 0.07, d: [3, 10] as [number, number] },
            ]) {
                ctx.save(); ctx.translate(cx, cy); ctx.setLineDash(ring.d);
                ctx.strokeStyle = colorStr(ring.c, ring.a); ctx.lineWidth = 1.2; ctx.beginPath();
                for (let s = 0; s <= 120; s++) {
                    const a = (s / 120) * Math.PI * 2 + ring.phase;
                    let p: Vec3 = { x: Math.cos(a) * ring.r, y: 0, z: Math.sin(a) * ring.r };
                    p = rotX(p, ring.tilt); p = rotY(p, (mx - 0.5) * 0.12);
                    const rs = fL / (fL + p.z);
                    if (s === 0) ctx.moveTo(p.x * rs, p.y * rs); else ctx.lineTo(p.x * rs, p.y * rs);
                }
                ctx.stroke(); ctx.setLineDash([]); ctx.restore();
            }

            // [3] Edges
            for (const [i, j] of data.edges) {
                const a = proj[i], b = proj[j];
                if (!isOk(a.x) || !isOk(b.x)) continue;
                const d = (a.depth + b.depth) * 0.5;
                const ni = data.nodes[i], nj = data.nodes[j];
                const et = ni.type !== 'default' ? ni.type : nj.type !== 'default' ? nj.type : 'commit';
                ctx.beginPath(); ctx.moveTo(a.x, a.y); ctx.lineTo(b.x, b.y);
                ctx.strokeStyle = colorStr(COLORS[et] || COLORS.commit, 0.04 + d * 0.24);
                ctx.lineWidth = 0.5 + d * 1.1; ctx.stroke();
            }

            // [4] Nodes
            for (let i = 0; i < proj.length; i++) {
                const p = proj[i]; const n = data.nodes[i];
                if (!isOk(p.x) || !isOk(p.y)) continue;
                const c = COLORS[n.type] || COLORS.default;
                const spec = n.type !== 'default';
                const ba = 0.15 + p.depth * 0.75;
                const pulse = spec ? 0.7 + 0.3 * Math.sin(t * 2 + n.pulsePhase) : 1;
                const nr = spec ? 2.5 + p.depth * 5.5 : 1.0 + p.depth * 2.2;
                if (!isOk(nr) || nr <= 0) continue;
                if (spec) {
                    const gr = ctx.createRadialGradient(p.x, p.y, 0, p.x, p.y, nr * 8);
                    gr.addColorStop(0, colorStr(c, ba * 0.38 * pulse)); gr.addColorStop(1, colorStr(c, 0));
                    ctx.beginPath(); ctx.arc(p.x, p.y, nr * 8, 0, Math.PI * 2); ctx.fillStyle = gr; ctx.fill();
                }
                ctx.beginPath(); ctx.arc(p.x, p.y, nr, 0, Math.PI * 2);
                ctx.fillStyle = colorStr(c, ba * pulse); ctx.fill();
                if (spec) {
                    ctx.beginPath(); ctx.arc(p.x, p.y, nr * 0.45, 0, Math.PI * 2);
                    ctx.fillStyle = colorStr({ r: 255, g: 255, b: 255 }, ba * 0.65 * pulse); ctx.fill();
                }
            }

            // [5] Flow particles
            for (const fp of data.flow) {
                fp.progress += fp.speed;
                if (fp.progress > 1) {
                    fp.progress = 0;
                    const ei = Math.floor(Math.random() * data.edges.length);
                    fp.fromIdx = data.edges[ei][0]; fp.toIdx = data.edges[ei][1]; fp.trail = [];
                    if (Math.random() < 0.4) data.bursts.push({ radius: 2, maxRadius: 20 + Math.random() * 16, alpha: 0.65, color: fp.color });
                }
                const fr = proj[fp.fromIdx], to = proj[fp.toIdx];
                if (!fr || !to || !isOk(fr.x) || !isOk(to.x)) continue;
                const px = fr.x + (to.x - fr.x) * fp.progress;
                const py = fr.y + (to.y - fr.y) * fp.progress;
                const d = (fr.depth + to.depth) * 0.5;
                if (!isOk(px) || !isOk(py)) continue;
                fp.trail.push({ x: px, y: py, alpha: 0.9 });
                if (fp.trail.length > 14) fp.trail.shift();
                for (let ti = 0; ti < fp.trail.length; ti++) {
                    const tp = fp.trail[ti]; tp.alpha *= 0.87;
                    const tA = clamp(tp.alpha * d * 0.75);
                    if (!isOk(tA) || !isOk(tp.x) || !isOk(tp.y)) continue;
                    ctx.beginPath(); ctx.arc(tp.x, tp.y, 0.7 + (ti / fp.trail.length) * 1.8, 0, Math.PI * 2);
                    ctx.fillStyle = fp.color + Math.round(tA * 255).toString(16).padStart(2, '0'); ctx.fill();
                }
                ctx.beginPath(); ctx.arc(px, py, 2.2 + d * 0.8, 0, Math.PI * 2);
                ctx.fillStyle = fp.color; ctx.globalAlpha = clamp(d * 0.9); ctx.fill(); ctx.globalAlpha = 1;
            }

            // [6] Central Nexus
            {
                const p2 = 0.6 + 0.4 * Math.sin(t * 1.1);
                const cr = R * 0.045;
                const outer = ctx.createRadialGradient(cx, cy, 0, cx, cy, R * 0.30);
                outer.addColorStop(0, `rgba(0,229,218,${0.20 + p2 * 0.12})`);
                outer.addColorStop(0.4, `rgba(0,229,218,${0.06 + p2 * 0.04})`);
                outer.addColorStop(1, 'rgba(0,229,218,0)');
                ctx.fillStyle = outer; ctx.beginPath(); ctx.arc(cx, cy, R * 0.30, 0, Math.PI * 2); ctx.fill();

                const inner = ctx.createRadialGradient(cx, cy, 0, cx, cy, R * 0.12);
                inner.addColorStop(0, `rgba(0,255,248,${0.60 + p2 * 0.25})`);
                inner.addColorStop(0.5, `rgba(0,229,218,${0.22 + p2 * 0.10})`);
                inner.addColorStop(1, 'rgba(0,229,218,0)');
                ctx.fillStyle = inner; ctx.beginPath(); ctx.arc(cx, cy, R * 0.12, 0, Math.PI * 2); ctx.fill();

                ctx.beginPath(); ctx.arc(cx, cy, cr, 0, Math.PI * 2);
                ctx.fillStyle = `rgba(220,255,252,${0.90 + p2 * 0.10})`; ctx.fill();

                for (let li = 0; li < 4; li++) {
                    const angle = t * 0.35 + (li * Math.PI) / 2;
                    const ll = R * 0.10 * (0.6 + 0.4 * Math.abs(Math.sin(t + li)));
                    ctx.beginPath();
                    ctx.moveTo(cx + Math.cos(angle) * cr * 1.6, cy + Math.sin(angle) * cr * 1.6);
                    ctx.lineTo(cx + Math.cos(angle) * ll, cy + Math.sin(angle) * ll);
                    ctx.strokeStyle = `rgba(0,255,248,${0.28 + p2 * 0.18})`; ctx.lineWidth = 1.1; ctx.stroke();
                }
            }

            // [7] Nexus bursts
            for (let b = data.bursts.length - 1; b >= 0; b--) {
                const bst = data.bursts[b]; bst.radius += 1.3; bst.alpha *= 0.93;
                if (bst.alpha < 0.02 || bst.radius > bst.maxRadius) { data.bursts.splice(b, 1); continue; }
                ctx.beginPath(); ctx.arc(cx, cy, bst.radius, 0, Math.PI * 2);
                ctx.strokeStyle = bst.color + Math.round(clamp(bst.alpha) * 255).toString(16).padStart(2, '0');
                ctx.lineWidth = 1.5; ctx.stroke();
            }

            // [8] Pulse rings
            if (frame % 70 === 0) {
                const specs = data.nodes.map((n, i) => n.type !== 'default' ? i : -1).filter(i => i >= 0);
                if (specs.length > 0) {
                    const idx = specs[Math.floor(Math.random() * specs.length)];
                    const p = proj[idx];
                    if (isOk(p.x) && isOk(p.y))
                        data.pulseRings.push({ x: p.x, y: p.y, radius: 4, maxRadius: 30 + Math.random() * 22, alpha: 0.65, color: colorStr(COLORS[data.nodes[idx].type], 1), speed: 0.5 + Math.random() * 0.3 });
                }
            }
            for (let r = data.pulseRings.length - 1; r >= 0; r--) {
                const rng = data.pulseRings[r]; rng.radius += rng.speed; rng.alpha *= 0.963;
                if (rng.alpha < 0.01 || rng.radius > rng.maxRadius) { data.pulseRings.splice(r, 1); continue; }
                if (!isOk(rng.x) || !isOk(rng.y)) { data.pulseRings.splice(r, 1); continue; }
                ctx.beginPath(); ctx.arc(rng.x, rng.y, rng.radius, 0, Math.PI * 2);
                ctx.strokeStyle = rng.color.replace(/[\d.]+\)$/, `${rng.alpha})`);
                ctx.lineWidth = 1.8; ctx.stroke();
            }

            // [9] Beam connections → label positions (edges of canvas)
            const beams = [
                { type: 'ci' as const, tx: W * 0.90, ty: H * 0.10 },
                { type: 'ticket' as const, tx: W * 0.90, ty: H * 0.85 },
                { type: 'audit' as const, tx: W * 0.04, ty: H * 0.25 },
                { type: 'commit' as const, tx: W * 0.04, ty: H * 0.78 },
            ];
            for (const beam of beams) {
                let best = -1, bestD = -1;
                for (let i = 0; i < data.nodes.length; i++)
                    if (data.nodes[i].type === beam.type && proj[i].depth > bestD) { bestD = proj[i].depth; best = i; }
                if (best < 0) continue;
                const fr = proj[best]; if (!isOk(fr.x) || !isOk(fr.y)) continue;
                const c = COLORS[beam.type];
                const grad = ctx.createLinearGradient(fr.x, fr.y, beam.tx, beam.ty);
                grad.addColorStop(0, colorStr(c, 0.6 * bestD));
                grad.addColorStop(1, colorStr(c, 0.25));
                ctx.beginPath(); ctx.moveTo(fr.x, fr.y); ctx.lineTo(beam.tx, beam.ty);
                ctx.strokeStyle = grad; ctx.lineWidth = 1.3; ctx.stroke();
                ctx.beginPath(); ctx.arc(beam.tx, beam.ty, 3.5, 0, Math.PI * 2);
                ctx.fillStyle = colorStr(c, 0.9); ctx.fill();
            }

            // [10] Ambient floaters
            for (let i = 0; i < 10; i++) {
                const a = t * (0.07 + i * 0.022) + (i * Math.PI * 2) / 10;
                const d2 = R * (1.1 + 0.3 * Math.sin(t * 0.22 + i));
                let p: Vec3 = { x: Math.cos(a) * d2, y: Math.sin(a * 0.55 + i) * d2 * 0.25, z: Math.sin(a) * d2 };
                p = rotY(p, rotYA * 0.5);
                const fs = fL / (fL + p.z); if (!isOk(fs)) continue;
                const fpx = cx + p.x * fs, fpy = cy + p.y * fs;
                if (!isOk(fpx) || !isOk(fpy)) continue;
                const cols2 = [COLORS.commit, COLORS.ci, COLORS.ticket, COLORS.audit, COLORS.amber];
                ctx.beginPath(); ctx.arc(fpx, fpy, 1.1 + fs * 0.5, 0, Math.PI * 2);
                ctx.fillStyle = colorStr(cols2[i % cols2.length], 0.17 + 0.15 * Math.sin(t + i * 0.4)); ctx.fill();
            }

            animId = requestAnimationFrame(draw);
        };

        animId = requestAnimationFrame(draw);
        window.addEventListener('mousemove', onMouse);
        return () => { cancelAnimationFrame(animId); window.removeEventListener('mousemove', onMouse); };
    }, [ready, onMouse]);

    return <canvas ref={canvasRef} className="w-full h-full block" />;
}

/* ════════════════════════════════════════════════════════
   SATELLITE CARD — unified dark glass, accent color only
════════════════════════════════════════════════════════ */
interface SatCardProps {
    subtitle: string;
    title: string;
    extra?: React.ReactNode;
    rgb: [number, number, number];
    delay: number;
    floatY: [number, number, number];
    floatDuration: number;
    className: string;
    from: { x?: number; y?: number };
    ping?: boolean;
}

function SatCard({ subtitle, title, extra, rgb, delay, floatY, floatDuration, className, from, ping }: SatCardProps) {
    const [r, g, b] = rgb;
    const solid = `rgb(${r},${g},${b})`;
    const c = (a: number) => `rgba(${r},${g},${b},${a})`;

    return (
        <motion.div
            className={`absolute z-30 pointer-events-none hidden lg:block ${className}`}
            initial={{ opacity: 0, ...from, scale: 0.88 }}
            animate={{ opacity: 1, x: 0, y: 0, scale: 1 }}
            transition={{ delay, duration: 0.9, type: 'spring', stiffness: 90 }}
        >
            <motion.div
                className="rounded-2xl px-3.5 py-3 border backdrop-blur-xl"
                style={{
                    background: 'rgba(3, 7, 12, 0.78)',
                    borderColor: c(0.22),
                    boxShadow: `0 0 28px ${c(0.12)}, inset 0 1px 0 rgba(255,255,255,0.04)`,
                    minWidth: '158px',
                    maxWidth: '192px',
                }}
                animate={{ y: floatY }}
                transition={{ duration: floatDuration, repeat: Infinity, ease: 'easeInOut' }}
            >
                <div className="flex items-center gap-2 mb-1.5">
                    <div className="relative shrink-0">
                        <span className="w-2 h-2 rounded-full block" style={{ background: solid }} />
                        {ping && <span className="absolute inset-0 w-2 h-2 rounded-full animate-ping opacity-35" style={{ background: solid }} />}
                    </div>
                    <span className="text-[9px] font-mono uppercase tracking-widest text-white/35">{subtitle}</span>
                </div>
                <div className="text-[14px] font-black leading-tight" style={{ color: solid }}>{title}</div>
                {extra && <div className="mt-1.5">{extra}</div>}
            </motion.div>
        </motion.div>
    );
}

/* ════════════════════════════════════════════════════════
   EVIDENCE CHAIN — animated commit-to-audit pipeline strip
════════════════════════════════════════════════════════ */
const CHAIN_NODES = [
    { id: 'commit', label: 'Commit', detail: 'a3f8c2e → main', rgb: [0, 229, 218] as [number, number, number] },
    { id: 'ci', label: 'CI Pipeline', detail: 'Build #142 ✓', rgb: [34, 197, 94] as [number, number, number] },
    { id: 'policy', label: 'Policy', detail: 'COMPLIANT', rgb: [255, 187, 26] as [number, number, number] },
    { id: 'jira', label: 'Jira', detail: 'GOV-1247 linked', rgb: [59, 130, 246] as [number, number, number] },
    { id: 'audit', label: 'Audit Log', detail: '4,821 events', rgb: [168, 85, 247] as [number, number, number] },
];

interface EvidenceChainProps {
    demoStep: number; // -1 = auto loop; 0–4 = demo active step; 5 = demo complete
}

function EvidenceChain({ demoStep }: EvidenceChainProps) {
    const [autoActive, setAutoActive] = useState(0);
    const isDemo = demoStep >= 0;
    const isComplete = demoStep === CHAIN_NODES.length;
    const active = isDemo ? Math.min(demoStep, CHAIN_NODES.length - 1) : autoActive;

    useEffect(() => {
        if (isDemo) return;
        const id = setInterval(() => setAutoActive(a => (a + 1) % CHAIN_NODES.length), 2200);
        return () => clearInterval(id);
    }, [isDemo]);

    return (
        <motion.div
            className="hidden md:block w-full"
            initial={{ opacity: 0, y: 20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 1.6, duration: 0.8 }}
        >
            {/* Container card */}
            <div
                className="rounded-2xl px-6 py-5 border"
                style={{
                    borderColor: 'rgba(255,255,255,0.07)',
                    background: 'rgba(255,255,255,0.018)',
                    backdropFilter: 'blur(8px)',
                }}
            >
                {/* Header row */}
                <div className="flex items-center justify-between mb-5">
                    <div>
                        <div className="text-[10px] font-mono uppercase tracking-[0.22em] text-white/50">
                            Evidence Chain
                        </div>
                        <div className="text-[9px] font-mono text-white/25 mt-0.5">
                            Commit → CI → Policy → Jira → Audit — end-to-end traceability
                        </div>
                    </div>
                    <div className="flex items-center gap-1.5">
                        <span className="w-1.5 h-1.5 rounded-full bg-[#00e5da] animate-pulse" />
                        <span className="text-[8px] font-mono text-[#00e5da]/60 uppercase tracking-wider">Live</span>
                    </div>
                </div>

                {/* Chain row */}
                <div className="flex items-start justify-center">
                    {CHAIN_NODES.map((node, i) => {
                        const [r, g, b] = node.rgb;
                        const isActive = i === active;
                        const isPast = i < active;
                        const solid = `rgb(${r},${g},${b})`;
                        const c = (a: number) => `rgba(${r},${g},${b},${a})`;
                        // In demo, traveling dot is on connector i→i+1 while i+1 is becoming active
                        const showDot = isDemo ? i === demoStep - 1 : i === autoActive - 1;

                        return (
                            <React.Fragment key={node.id}>
                                {/* Step node */}
                                <div className="flex flex-col items-center gap-2 w-[100px] sm:w-[112px]">
                                    <div
                                        className="w-10 h-10 rounded-xl border flex items-center justify-center transition-all duration-400"
                                        style={{
                                            borderColor: isActive ? c(0.7) : isPast ? c(0.35) : 'rgba(255,255,255,0.12)',
                                            background: isActive ? c(0.12) : isPast ? c(0.04) : 'rgba(255,255,255,0.03)',
                                            boxShadow: isActive ? `0 0 22px ${c(0.35)}, 0 0 8px ${c(0.2)}` : 'none',
                                        }}
                                    >
                                        <span
                                            className="w-2.5 h-2.5 rounded-full transition-all duration-400"
                                            style={{
                                                background: isActive ? solid : isPast ? c(0.65) : c(0.28),
                                                boxShadow: isActive ? `0 0 10px ${solid}, 0 0 4px ${solid}` : 'none',
                                            }}
                                        />
                                    </div>
                                    <div className="text-center">
                                        <div
                                            className="text-[11px] font-bold transition-colors duration-400"
                                            style={{ color: isActive ? solid : isPast ? c(0.65) : 'rgba(255,255,255,0.32)' }}
                                        >
                                            {node.label}
                                        </div>
                                        <div
                                            className="text-[9px] font-mono mt-0.5 whitespace-nowrap transition-colors duration-400"
                                            style={{ color: isActive ? c(0.7) : isPast ? c(0.4) : 'rgba(255,255,255,0.22)' }}
                                        >
                                            {node.detail}
                                        </div>
                                    </div>
                                </div>

                                {/* Connector */}
                                {i < CHAIN_NODES.length - 1 && (
                                    <div className="flex-1 relative h-10 flex items-center mx-0.5" style={{ maxWidth: '52px' }}>
                                        <div
                                            className="w-full h-px transition-all duration-500"
                                            style={{ background: isPast ? c(0.5) : 'rgba(255,255,255,0.13)' }}
                                        />
                                        {showDot && (
                                            <motion.div
                                                className="absolute top-1/2 -translate-y-1/2 w-2 h-2 rounded-full"
                                                style={{ background: solid, boxShadow: `0 0 8px ${solid}` }}
                                                initial={{ left: 0 }}
                                                animate={{ left: '100%' }}
                                                transition={{ duration: isDemo ? 0.45 : 0.7, ease: 'easeInOut' }}
                                            />
                                        )}
                                    </div>
                                )}
                            </React.Fragment>
                        );
                    })}
                </div>

                {/* Completion badge — only shown at end of demo */}
                <div className="h-10 flex items-center justify-center mt-2">
                    {isComplete && (
                        <motion.div
                            className="flex items-center gap-2.5 px-5 py-2 rounded-full border"
                            style={{ borderColor: 'rgba(0,229,218,0.4)', background: 'rgba(0,229,218,0.06)' }}
                            initial={{ opacity: 0, scale: 0.85, y: 8 }}
                            animate={{ opacity: 1, scale: 1, y: 0 }}
                            transition={{ duration: 0.5, type: 'spring', stiffness: 120 }}
                        >
                            <span className="w-2 h-2 rounded-full bg-[#00e5da] animate-pulse" />
                            <span className="text-[11px] font-black text-[#00e5da] tracking-wide">COMPLIANT</span>
                            <span className="text-[9px] font-mono text-white/35">· evidence recorded immutably</span>
                        </motion.div>
                    )}
                </div>
            </div>{/* end container card */}
        </motion.div>
    );
}

/* ════════════════════════════════════════════════════════
   SPATIAL BENTO CARD — 3D Tilt + Glassmorphism Spotlight
════════════════════════════════════════════════════════ */
interface SpatialBentoCardProps {
    node: typeof CHAIN_NODES[0];
    isActive: boolean;
    isPast: boolean;
    mousePos: { x: number; y: number };
    className?: string;
}

function SpatialBentoCard({ node, isActive, isPast, mousePos, className = '' }: SpatialBentoCardProps) {
    const cardRef = useRef<HTMLDivElement>(null);
    const [bounds, setBounds] = useState({ left: 0, top: 0, width: 0, height: 0 });

    useEffect(() => {
        if (!cardRef.current) return;
        const resizeObs = new ResizeObserver(() => {
            if (cardRef.current) {
                setBounds(cardRef.current.getBoundingClientRect());
            }
        });
        resizeObs.observe(cardRef.current);
        return () => resizeObs.disconnect();
    }, []);

    // Spotlight logic
    const relX = mousePos.x - bounds.left;
    const relY = mousePos.y - bounds.top;
    const isHovered = relX >= 0 && relX <= bounds.width && relY >= 0 && relY <= bounds.height;

    const [r, g, b] = node.rgb;
    const solidColor = `rgb(${r},${g},${b})`;
    const glowColor = `rgba(${r},${g},${b},${isActive ? 0.4 : isPast ? 0.15 : 0.05})`;

    // 3D Tilt Math (very subtle)
    const tiltX = isHovered ? ((relY / bounds.height) - 0.5) * -12 : 0;
    const tiltY = isHovered ? ((relX / bounds.width) - 0.5) * 12 : 0;

    return (
        <motion.div
            ref={cardRef}
            className={`relative rounded-2xl overflow-hidden backdrop-blur-3xl transition-all duration-500 ease-out border flex flex-col justify-center ${className}`}
            style={{
                // Ultra-light glassmorphism
                background: isActive ? 'rgba(255, 255, 255, 0.05)' : 'rgba(10, 15, 25, 0.15)',
                borderColor: isActive ? `rgba(${r},${g},${b},0.5)` : 'rgba(255,255,255,0.06)',
                boxShadow: `0 8px 32px 0 rgba(0, 0, 0, 0.3), inset 0 0 0 1px rgba(255,255,255,0.02), 0 0 40px ${glowColor}`,
            }}
            animate={{
                rotateX: tiltX,
                rotateY: tiltY,
                transformPerspective: 1000,
                scale: isActive ? 1.05 : 1,
                z: isActive ? 30 : 0
            }}
        >
            {/* Spotlight Gradient */}
            <motion.div
                className="pointer-events-none absolute -inset-px rounded-2xl opacity-0 transition duration-300 group-hover:opacity-100"
                style={{ opacity: isHovered ? 1 : 0 }}
                animate={{
                    background: `radial-gradient(400px circle at ${relX}px ${relY}px, rgba(255,255,255,0.08), transparent 40%)`,
                }}
            />
            {/* Ambient internal color glow */}
            {(isActive || isHovered) && (
                <motion.div
                    className="pointer-events-none absolute inset-0 opacity-20 mix-blend-screen"
                    animate={{
                        background: `radial-gradient(150px circle at ${bounds.width / 2}px ${bounds.height / 2}px, rgba(${r},${g},${b},0.6), transparent 100%)`,
                    }}
                />
            )}

            <div className="relative z-10 p-5 w-full">
                <div className="flex items-center gap-3 mb-3">
                    <div
                        className="w-8 h-8 rounded-xl flex items-center justify-center border transition-all duration-400"
                        style={{
                            borderColor: isActive ? `rgba(${r},${g},${b},0.8)` : isPast ? `rgba(${r},${g},${b},0.4)` : 'rgba(255,255,255,0.1)',
                            background: isActive ? `rgba(${r},${g},${b},0.15)` : 'rgba(255,255,255,0.03)',
                        }}
                    >
                        <span
                            className="w-2.5 h-2.5 rounded-full transition-all duration-400"
                            style={{
                                background: isActive ? solidColor : isPast ? `rgba(${r},${g},${b},0.7)` : 'rgba(255,255,255,0.2)',
                                boxShadow: isActive ? `0 0 15px ${solidColor}, 0 0 5px ${solidColor}` : 'none',
                            }}
                        />
                    </div>
                    <div>
                        <div className="text-[10px] font-mono uppercase tracking-widest text-white/50">{node.id}</div>
                        <div className="text-[14px] font-bold text-white tracking-wide">{node.label}</div>
                    </div>
                </div>
                <div className="text-[11px] font-mono leading-relaxed" style={{ color: isActive ? solidColor : 'rgba(255,255,255,0.5)' }}>
                    {node.detail}
                </div>
            </div>
        </motion.div>
    );
}

/* ════════════════════════════════════════════════════════
   EVIDENCE DATA TUNNEL — Draws connections between bento cards
════════════════════════════════════════════════════════ */
// We use a simplified SVG overlay to connect specific points.
// Paths updated for the new Horizontal Pipeline layout (Jira -> Commit -> Policy -> CI -> Audit)
interface SvgDataTunnelProps {
    demoStep: number;
}
function SvgDataTunnel({ demoStep }: SvgDataTunnelProps) {
    const isDemo = demoStep >= 0;

    // Grid coordinates (approximate horizontal centers of the 5 cards in a row):
    // C1: 10% | C2: 30% | C3: 50% | C4: 70% | C5: 90%
    // Y coordinate is 50% since they are in a straight line
    const paths = [
        // Simulated ending cycle (Audit back to Jira, hidden)
        { d: "M 10% 50% L 90% 50%", from: 0, to: 4, isHidden: true },

        // Step 1: Jira (10%) to Commit (30%)
        { d: "M 10% 50% L 30% 50%", from: 3, to: 0 },

        // Step 2: Commit (30%) to Policy (50%)
        { d: "M 30% 50% L 50% 50%", from: 0, to: 2 },

        // Step 3: Policy (50%) to CI (70%)
        { d: "M 50% 50% L 70% 50%", from: 2, to: 1 },

        // Step 4: CI (70%) to Audit Log (90%)
        { d: "M 70% 50% L 90% 50%", from: 1, to: 4 },
    ];

    return (
        <svg className="absolute inset-0 w-full h-full pointer-events-none z-10" style={{ filter: 'drop-shadow(0 0 8px rgba(255,255,255,0.1))' }}>
            {paths.map((p, i) => {
                const isActivePath = isDemo && (demoStep === p.to) && (demoStep > 0);
                const isPastPath = isDemo && demoStep > p.to;
                return (
                    <g key={i} style={{ display: p.isHidden ? 'none' : 'block' }}>
                        <path
                            d={p.d}
                            stroke={isPastPath ? "rgba(255,255,255,0.15)" : "rgba(255,255,255,0.05)"}
                            strokeWidth="2"
                            fill="none"
                            strokeDasharray="4 4"
                        />
                        {isActivePath && (
                            <motion.path
                                d={p.d}
                                stroke="rgba(0, 229, 218, 0.8)"
                                strokeWidth="3"
                                fill="none"
                                initial={{ pathLength: 0, opacity: 0 }}
                                animate={{ pathLength: 1, opacity: 1 }}
                                transition={{ duration: 0.5, ease: "easeInOut" }}
                            />
                        )}
                        {/* Moving packet dot */}
                        {isActivePath && (
                            <motion.circle
                                r="4"
                                fill="#00e5da"
                                style={{ filter: 'drop-shadow(0 0 10px #00e5da)' }}
                            >
                                <animateMotion dur="0.5s" repeatCount="1" fill="freeze" path={p.d} />
                            </motion.circle>
                        )}
                    </g>
                );
            })}
        </svg>
    );
}

/* ════════════════════════════════════════════════════════
   Hero Section - Immersive Spatial Layout
════════════════════════════════════════════════════════ */
export function Hero() {
    const { t } = useTranslation();
    const [demoStep, setDemoStep] = useState(-1);
    const demoActive = demoStep >= 0;
    const [mousePos, setMousePos] = useState({ x: 0, y: 0 });

    useEffect(() => {
        if (demoStep < 0) return;
        if (demoStep === CHAIN_NODES.length) {
            const id = setTimeout(() => setDemoStep(-1), 3000); // Wait longer on complete
            return () => clearTimeout(id);
        }
        const id = setTimeout(() => setDemoStep(s => s + 1), 700); // Slightly slower for dramatic effect
        return () => clearTimeout(id);
    }, [demoStep]);

    const handleMouseMove = (e: React.MouseEvent) => {
        setMousePos({ x: e.clientX, y: e.clientY });
    };

    return (
        <section
            className="relative overflow-hidden flex flex-col items-center justify-center bg-[#03070c]"
            id="hero"
            style={{ minHeight: '100vh', perspective: '1000px' }}
            onMouseMove={handleMouseMove}
        >
            {/* INMERSIVE BACKGROUND CANVAS (Now restricted conceptually, but visually spans to give atmosphere) */}
            {/* We'll let it span but mask it so it fades toward the text side */}
            <div className="absolute inset-0 z-0 pointer-events-none opacity-40 mix-blend-screen scale-110"
                style={{
                    maskImage: 'linear-gradient(to right, transparent 0%, black 50%, black 100%)',
                    WebkitMaskImage: 'linear-gradient(to right, transparent 0%, black 50%, black 100%)'
                }}>
            </div>

            {/* Environmental "Stardust" / Deep glow blobs for spatial depth */}
            <div className="absolute top-[20%] left-[-10%] w-[40%] h-[40%] bg-brand-500/10 blur-[120px] rounded-full pointer-events-none mix-blend-screen" />
            <div className="absolute bottom-[20%] right-[-10%] w-[50%] h-[50%] bg-accent-500/10 blur-[150px] rounded-full pointer-events-none mix-blend-screen" />

            {/* Sphere Background Glow Layer */}
            <div className="absolute top-[20%] right-[10%] w-[40%] h-[40%] bg-blue-500/10 blur-[150px] rounded-full pointer-events-none mix-blend-screen z-0" />


            {/* Background grid texture */}
            <div
                className="absolute inset-0 pointer-events-none z-0"
                style={{
                    opacity: 0.04,
                    backgroundImage: `
                        linear-gradient(rgba(255,255,255,0.2) 1px, transparent 1px),
                        linear-gradient(90deg, rgba(255,255,255,0.2) 1px, transparent 1px)
                    `,
                    backgroundSize: '40px 40px',
                }}
            />

            {/* Bottom blend gradient merging into the Pipeline row */}
            <div
                className="absolute bottom-40 left-0 right-0 h-40 pointer-events-none z-10"
                style={{ background: 'linear-gradient(to bottom, transparent, #03070c)' }}
            />

            {/* Global Spotlight tracking cursor for the whole section */}
            <motion.div
                className="pointer-events-none absolute inset-0 z-10 opacity-30"
                animate={{
                    background: `radial-gradient(800px circle at ${mousePos.x}px ${mousePos.y}px, rgba(0,229,218,0.06), transparent 60%)`,
                }}
            />

            {/* ═══ MAIN CONTENT FRAME ═══ */}
            <Container className="relative z-20 w-full pt-28 flex-1 flex flex-col justify-start">

                {/* ── TOP SECTION: SPLIT LAYOUT (Text / Sphere) ── */}
                <div className="grid grid-cols-12 gap-8 items-center w-full min-h-[500px]">

                    {/* ── Left Column: TEXT & CTA (6 cols) ── */}
                    <div className="col-span-12 lg:col-span-6 flex flex-col items-center lg:items-start text-center lg:text-left z-20">
                        <motion.div
                            initial={{ opacity: 0, y: 20 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ delay: 0.2, duration: 0.6 }}
                            className="relative"
                        >
                            {/* Futuristic glowing badge line */}
                            <div className="absolute -inset-1 bg-gradient-to-r from-brand-400 to-accent-600 rounded-full blur opacity-20 group-hover:opacity-40 transition duration-1000" />
                            <Badge variant="brand" size="md" className="relative bg-[#03070c]/80 backdrop-blur-md border-white/10">
                                <span className="flex items-center gap-2">
                                    <span className="w-1.5 h-1.5 rounded-full bg-brand-400 animate-pulse shadow-[0_0_8px_rgba(0,229,218,0.8)]" />
                                    v{siteConfig.version} — {t('hero.badge')}
                                </span>
                            </Badge>
                        </motion.div>

                        {/* Headline */}
                        <motion.h1
                            className="mt-8 font-bold tracking-tight leading-[1] md:leading-[0.95]"
                            initial={{ opacity: 0, y: 30 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ delay: 0.35, duration: 0.7, ease: [0.25, 0.4, 0.25, 1] }}
                        >
                            <span className="block text-white text-5xl sm:text-6xl md:text-[4rem] lg:text-[4.5rem] drop-shadow-xl">
                                {t('hero.title1')}
                            </span>
                            {/* Standard Brand Color Text */}
                            <span
                                className="block text-transparent bg-clip-text bg-gradient-to-r from-brand-400 to-[#00b3aa] pb-4 mt-2 h-[120%] text-5xl sm:text-6xl md:text-[4rem] lg:text-[4.5rem]"
                                style={{
                                    filter: 'drop-shadow(0px 4px 12px rgba(0,229,218,0.25))'
                                }}
                            >
                                {t('hero.title2')}
                            </span>
                        </motion.h1>

                        <motion.p
                            className="mt-6 text-lg md:text-xl text-gray-300 leading-relaxed max-w-lg font-medium tracking-wide"
                            initial={{ opacity: 0, y: 20 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ delay: 0.5, duration: 0.6 }}
                            style={{ textShadow: '0 2px 10px rgba(0,0,0,0.8)' }}
                        >
                            {t('hero.subtitle')}
                        </motion.p>

                        {/* CTAs with Shimmer Effect */}
                        <motion.div
                            className="mt-10 flex flex-col sm:flex-row items-center gap-5"
                            initial={{ opacity: 0, y: 20 }}
                            animate={{ opacity: 1, y: 0 }}
                            transition={{ delay: 0.65, duration: 0.6 }}
                        >
                            <button
                                onClick={() => { if (!demoActive) setDemoStep(0); }}
                                disabled={demoActive}
                                className="group relative overflow-hidden flex items-center justify-center gap-2.5 px-8 py-4 rounded-xl font-bold text-base transition-all disabled:opacity-80 disabled:cursor-wait"
                                style={{
                                    background: demoActive ? 'rgba(0,229,218,0.1)' : 'rgba(255,255,255,0.03)',
                                    color: demoActive ? '#00e5da' : '#fff',
                                    border: '1px solid',
                                    borderColor: demoActive ? 'rgba(0,229,218,0.4)' : 'rgba(255,255,255,0.15)',
                                    boxShadow: demoActive ? 'inset 0 0 20px rgba(0,229,218,0.1)' : '0 8px 32px rgba(0,0,0,0.4), inset 0 1px 0 rgba(255,255,255,0.1)',
                                    backdropFilter: 'blur(12px)'
                                }}
                            >
                                {/* Shimmer animation overlay */}
                                {!demoActive && (
                                    <div className="absolute top-0 bottom-0 left-0 w-[20%] bg-gradient-to-r from-transparent via-white/10 to-transparent [animation:shimmerSlide_3s_infinite_ease-in-out_2s]" />
                                )}

                                {demoActive ? (
                                    <>
                                        <span className="relative flex h-3 w-3">
                                            <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-brand-400 opacity-75"></span>
                                            <span className="relative inline-flex rounded-full h-3 w-3 bg-brand-500"></span>
                                        </span>
                                        <span>Tracking Event...</span>
                                    </>
                                ) : (
                                    <>
                                        <HiOutlinePlay className="text-brand-400 group-hover:scale-110 transition-transform" size={20} />
                                        <span>Iniciar Demo Espacial</span>
                                    </>
                                )}
                            </button>
                            <a
                                href="/download"
                                className="flex items-center gap-2 px-6 py-4 rounded-xl text-sm font-semibold transition-all hover:bg-white/5"
                                style={{
                                    color: 'rgba(255,255,255,0.6)',
                                    border: '1px solid rgba(255,255,255,0.05)',
                                }}
                            >
                                <HiOutlineDownload size={18} />
                                Descargar CLI
                            </a>
                        </motion.div>

                        {/* Social proof embedded inside layout */}
                        <motion.div
                            className="mt-8 flex items-center gap-4"
                            initial={{ opacity: 0 }}
                            animate={{ opacity: 1 }}
                            transition={{ delay: 0.78, duration: 0.6 }}
                        >
                            <div className="flex -space-x-2">
                                {['#1a3a2a', '#1a2d4a', '#2d1a4a'].map((bg, i) => (
                                    <div key={i} className="w-8 h-8 rounded-full border-2 border-[#03070c] flex items-center justify-center text-[10px] font-bold text-white/80 shadow-md" style={{ background: bg }}>
                                        {['MS', 'SB', 'GO'][i]}
                                    </div>
                                ))}
                            </div>
                            <div className="text-[11px] text-white/40 font-mono flex flex-col items-start leading-tight">
                                <span>Utilizado por +12 equipos</span>
                                <span>48,291 logs inmutables</span>
                            </div>
                        </motion.div>

                        {/* Compliance Complete Badge */}
                        {demoStep === CHAIN_NODES.length && (
                            <motion.div
                                className="mt-8 flex items-center gap-3 px-5 py-2.5 rounded-full border bg-brand-500/10 border-brand-400/30 backdrop-blur-md"
                                initial={{ opacity: 0, scale: 0.85, y: 10 }}
                                animate={{ opacity: 1, scale: 1, y: 0 }}
                                transition={{ type: 'spring', stiffness: 200, damping: 15 }}
                            >
                                <span className="w-2 h-2 rounded-full bg-[#00e5da] animate-pulse shadow-[0_0_10px_#00e5da]" />
                                <span className="text-xs font-black text-[#00e5da] tracking-widest uppercase">Certificado Inmutable Guardado</span>
                            </motion.div>
                        )}
                    </div>

                    {/* ── Right Column: 3D SPHERE CANVAS (6 cols) ── */}
                    <div className="col-span-12 lg:col-span-6 relative w-full h-[500px] lg:h-[650px] flex items-center justify-center mt-12 lg:mt-0 z-10 pointer-events-none">
                        <motion.div
                            className="w-[120%] h-[120%] relative"
                            initial={{ opacity: 0, scale: 0.9 }}
                            animate={{ opacity: 1, scale: 1 }}
                            transition={{ delay: 0.25, duration: 1.5, ease: [0.25, 0.4, 0.25, 1] }}
                        >
                            {/* The sphere itself, unrestricted visually */}
                            <GovernanceCanvas />
                        </motion.div>
                    </div>

                </div>

                {/* ── BOTTOM SECTION: INTERACTIVE DATA PIPELINE STRIP ── */}
                <div className="w-full relative mt-16 mb-24 z-20">
                    <motion.div
                        className="w-full relative min-h-[160px]"
                        initial={{ opacity: 0, y: 40 }}
                        animate={{ opacity: 1, y: 0 }}
                        transition={{ delay: 0.6, duration: 1.2, ease: [0.25, 0.4, 0.25, 1] }}
                    >
                        {/* Connecting Data Tunnel using our Horizontal SVG paths */}
                        <SvgDataTunnel demoStep={demoStep} />

                        {/* Horizontal Bento Pipeline
                                grid-cols-5 for 5 cards side-by-side
                             */}
                        <div className="grid grid-cols-1 sm:grid-cols-5 gap-4 w-full h-full relative z-10 px-2 items-center">

                            {/* Ordered: Jira -> Commit -> Policy -> CI -> Audit */}

                            {/* 1. Jira (Ticket Source) */}
                            <SpatialBentoCard
                                node={CHAIN_NODES[3]}
                                isActive={demoStep === 0}
                                isPast={demoStep > 0}
                                mousePos={mousePos}
                                className="col-span-1 min-h-[130px] shadow-xl"
                            />

                            {/* 2. Commit (Code Change) */}
                            <SpatialBentoCard
                                node={CHAIN_NODES[0]}
                                isActive={demoStep === 1}
                                isPast={demoStep > 1}
                                mousePos={mousePos}
                                className="col-span-1 min-h-[130px] shadow-xl"
                            />

                            {/* 3. Policy (Validation phase) */}
                            <SpatialBentoCard
                                node={CHAIN_NODES[2]}
                                isActive={demoStep === 2}
                                isPast={demoStep > 2}
                                mousePos={mousePos}
                                className="col-span-1 min-h-[130px] shadow-xl"
                            />

                            {/* 4. CI Pipeline (Build/Test) */}
                            <SpatialBentoCard
                                node={CHAIN_NODES[1]}
                                isActive={demoStep === 3}
                                isPast={demoStep > 3}
                                mousePos={mousePos}
                                className="col-span-1 min-h-[130px] shadow-xl"
                            />

                            {/* 5. Audit Log (Final Immutable storage) */}
                            <SpatialBentoCard
                                node={CHAIN_NODES[4]}
                                isActive={demoStep === 4 || demoStep === 5}
                                isPast={false}
                                mousePos={mousePos}
                                className="col-span-1 min-h-[130px] shadow-xl border-t-2"
                            />

                        </div>
                    </motion.div>
                </div>

            </Container>

            {/* Scroll indicator */}
            <motion.div
                className="absolute bottom-8 left-1/2 -translate-x-1/2 z-20"
                animate={{ y: [0, 10, 0], opacity: [0.4, 1, 0.4] }}
                transition={{ duration: 2.5, repeat: Infinity, ease: 'easeInOut' }}
            >
                <div className="w-[1px] h-12 bg-gradient-to-b from-brand-500/0 via-white/40 to-brand-500/0" />
            </motion.div>
        </section>
    );
}
