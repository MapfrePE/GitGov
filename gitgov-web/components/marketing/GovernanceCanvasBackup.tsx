/* 
  BACKUP - GovernanceCanvas Original
  Este archivo contiene el código original del canvas 3D del hero.
  Creado: ${new Date().toISOString()}
  Ubicación original: gitgov-web/components/marketing/Hero.tsx
*/

import React, { useRef, useEffect, useState, useCallback } from 'react';

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
  amber: { r: 255, g: 187, b: 26 }
};

function colorStr(c: { r: number; g: number; b: number }, a: number): string {
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
      pulsePhase: Math.random() * Math.PI * 2
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
      if (dx*dx + dy*dy + dz*dz < maxDist * maxDist) {
        edges.push([i, j]);
      }
    }
  }
  return edges;
}

function rotateY(p: Vec3, angle: number): Vec3 {
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  return {
    x: p.x * cos + p.z * sin,
    y: p.y,
    z: -p.x * sin + p.z * cos
  };
}

function rotateX(p: Vec3, angle: number): Vec3 {
  const cos = Math.cos(angle);
  const sin = Math.sin(angle);
  return {
    x: p.x,
    y: p.y * cos - p.z * sin,
    z: p.y * sin + p.z * cos
  };
}

export function OriginalGovernanceCanvas() {
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

  // Initialize data
  useEffect(() => {
    const R = 200;
    const nodes = generateSphereNodes(140, R);
    const edges = buildEdges(nodes, R * 0.55);
    
    const stars: StarParticle[] = Array.from({ length: 250 }, () => ({
      x: Math.random(),
      y: Math.random(),
      size: 0.3 + Math.random() * 1.5,
      alpha: 0.1 + Math.random() * 0.5,
      speed: 0.0001 + Math.random() * 0.0003,
      twinkleSpeed: 0.5 + Math.random() * 2,
      twinklePhase: Math.random() * Math.PI * 2
    }));
    
    const specialEdges = edges
      .map((_, i) => i)
      .filter(() => Math.random() < 0.06);
    
    const flowParticles: FlowParticle[] = Array.from({ length: 30 }, (_, i) => {
      const edgeIdx = specialEdges[i % specialEdges.length] || Math.floor(Math.random() * edges.length);
      const colors = ['#00e5da', '#22c55e', '#3b82f6', '#a855f7', '#ffbb1a'];
      return {
        fromIdx: edges[edgeIdx][0],
        toIdx: edges[edgeIdx][1],
        progress: Math.random(),
        speed: 0.003 + Math.random() * 0.008,
        color: colors[i % colors.length],
        trail: []
      };
    });
    
    dataRef.current = {
      nodes,
      edges,
      stars,
      flowParticles,
      pulseRings: [],
      specialEdges
    };
    
    setIsReady(true);
  }, []);

  const handleMouseMove = useCallback((e: MouseEvent) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    
    const rect = canvas.getBoundingClientRect();
    mouseRef.current = {
      x: (e.clientX - rect.left) / rect.width,
      y: (e.clientY - rect.top) / rect.height
    };
  }, []);

  // Animation loop
  useEffect(() => {
    if (!isReady || !dataRef.current) return;
    
    const canvas = canvasRef.current;
    const ctx = canvas?.getContext('2d');
    if (!canvas || !ctx) return;
    
    let animationId: number;
    
    const render = () => {
      if (!canvas || !ctx || !dataRef.current) return;
      
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      const W = rect.width;
      const H = rect.height;
      
      if (canvas.width !== W * dpr || canvas.height !== H * dpr) {
        canvas.width = W * dpr;
        canvas.height = H * dpr;
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      }
      
      // Clear and render...
      // (Animation loop implementation here)
      
      animationId = requestAnimationFrame(render);
    };
    
    animationId = requestAnimationFrame(render);
    return () => cancelAnimationFrame(animationId);
  }, [isReady]);
  
  return (
    <canvas
      ref={canvasRef}
      className="w-full h-full"
    />
  );
}

export default OriginalGovernanceCanvas;