'use client';

import React, { useRef, useEffect, useCallback, useState } from 'react';
import { motion } from 'framer-motion';

interface GitNode {
  x: number;
  y: number;
  radius: number;
  color: string;
  type: 'commit' | 'branch' | 'merge' | 'tag' | 'ci' | 'audit';
  pulse: number;
  pulseSpeed: number;
  connections: number[];
  velocity: { x: number; y: number };
}

interface FlowLine {
  from: number;
  to: number;
  progress: number;
  speed: number;
  color: string;
  width: number;
}

interface Particle {
  x: number;
  y: number;
  vx: number;
  vy: number;
  radius: number;
  color: string;
  life: number;
  maxLife: number;
}

export function GovernanceCanvasV2() {
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const animationRef = useRef<number>();
  const nodesRef = useRef<GitNode[]>([]);
  const flowLinesRef = useRef<FlowLine[]>([]);
  const particlesRef = useRef<Particle[]>([]);
  const mousePos = useRef({ x: 0, y: 0, isActive: false });
  const [isReady, setIsReady] = useState(false);

  // Initialize nodes and flow lines
  useEffect(() => {
    if (!canvasRef.current) return;

    const nodes: GitNode[] = [];
    const flowLines: FlowLine[] = [];
    const particles: Particle[] = [];

    // Create a git-like graph structure
    const colors = {
      commit: '#00e5da',
      branch: '#22c55e',
      merge: '#3b82f6',
      tag: '#a855f7',
      ci: '#f59e0b',
      audit: '#8b5cf6'
    };

    // Main timeline (central commits)
    for (let i = 0; i < 6; i++) {
      const commitNode: GitNode = {
        x: 0,
        y: 0,
        radius: 8 + Math.random() * 4,
        color: colors.commit,
        type: 'commit',
        pulse: Math.random() * Math.PI * 2,
        pulseSpeed: 0.05 + Math.random() * 0.05,
        connections: i > 0 ? [i - 1] : [],
        velocity: { x: 0, y: 0 }
      };
      nodes.push(commitNode);
    }

    // Branch nodes
    for (let i = 0; i < 4; i++) {
      const branchNode: GitNode = {
        x: 0,
        y: 0,
        radius: 6 + Math.random() * 3,
        color: colors.branch,
        type: 'branch',
        pulse: Math.random() * Math.PI * 2,
        pulseSpeed: 0.03 + Math.random() * 0.04,
        connections: [Math.floor(Math.random() * 3)], // Connect to random commit
        velocity: { x: 0, y: 0 }
      };
      nodes.push(branchNode);
    }

    // Special nodes (CI, Audit)
    const ciNode: GitNode = {
      x: 0,
      y: 0,
      radius: 10,
      color: colors.ci,
      type: 'ci',
      pulse: 0,
      pulseSpeed: 0.08,
      connections: [0, 2, 4],
      velocity: { x: 0, y: 0 }
    };
    nodes.push(ciNode);

    const auditNode: GitNode = {
      x: 0,
      y: 0,
      radius: 10,
      color: colors.audit,
      type: 'audit',
      pulse: Math.PI,
      pulseSpeed: 0.06,
      connections: [1, 3, 5],
      velocity: { x: 0, y: 0 }
    };
    nodes.push(auditNode);

    // Create flow lines between connected nodes
    nodes.forEach((node, i) => {
      node.connections.forEach(targetIdx => {
        if (targetIdx < nodes.length) {
          flowLines.push({
            from: i,
            to: targetIdx,
            progress: Math.random(),
            speed: 0.002 + Math.random() * 0.003,
            color: node.color,
            width: 1 + Math.random() * 1.5
          });
        }
      });
    });

    // Create particles
    for (let i = 0; i < 30; i++) {
      particles.push({
        x: Math.random() * 100,
        y: Math.random() * 100,
        vx: (Math.random() - 0.5) * 0.3,
        vy: (Math.random() - 0.5) * 0.3,
        radius: 1 + Math.random() * 2,
        color: ['#00e5da', '#22c55e', '#3b82f6', '#a855f7'][Math.floor(Math.random() * 4)],
        life: 50 + Math.random() * 100,
        maxLife: 150
      });
    }

    nodesRef.current = nodes;
    flowLinesRef.current = flowLines;
    particlesRef.current = particles;
    setIsReady(true);
  }, []);

  // Animation loop
  useEffect(() => {
    if (!isReady || !canvasRef.current) return;

    const canvas = canvasRef.current;
    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    let animationFrameId: number;
    let lastTime = 0;

    const animate = (currentTime: number) => {
      if (!ctx || !canvas) return;

      const deltaTime = currentTime - lastTime || 0;
      lastTime = currentTime;
      const time = currentTime * 0.001;

      // Update canvas size
      const dpr = window.devicePixelRatio || 1;
      const rect = canvas.getBoundingClientRect();
      const width = rect.width;
      const height = rect.height;

      if (canvas.width !== width * dpr || canvas.height !== height * dpr) {
        canvas.width = width * dpr;
        canvas.height = height * dpr;
        ctx.scale(dpr, dpr);
      }

      // Clear canvas with gradient
      const gradient = ctx.createLinearGradient(0, 0, width, height);
      gradient.addColorStop(0, '#0f172a');
      gradient.addColorStop(0.5, '#1e1b4b');
      gradient.addColorStop(1, '#0f172a');
      ctx.fillStyle = gradient;
      ctx.fillRect(0, 0, width, height);

      // Draw subtle grid
      ctx.strokeStyle = 'rgba(255, 255, 255, 0.03)';
      ctx.lineWidth = 0.5;
      const gridSize = 40;
      for (let x = 0; x < width; x += gridSize) {
        ctx.beginPath();
        ctx.moveTo(x, 0);
        ctx.lineTo(x, height);
        ctx.stroke();
      }
      for (let y = 0; y < height; y += gridSize) {
        ctx.beginPath();
        ctx.moveTo(0, y);
        ctx.lineTo(width, y);
        ctx.stroke();
      }

      const centerX = width / 2;
      const centerY = height / 2;
      const scale = Math.min(width, height) * 0.0008;

      // Update and position nodes in a circular layout
      nodesRef.current.forEach((node, i) => {
        const angle = (i / nodesRef.current.length) * Math.PI * 2 + time * 0.1;
        const radius = Math.min(width, height) * 0.3;
        
        // Target position
        const targetX = centerX + Math.cos(angle) * radius;
        const targetY = centerY + Math.sin(angle) * radius;
        
        // Smooth movement
        node.x += (targetX - node.x) * 0.05;
        node.y += (targetY - node.y) * 0.05;
        
        // Update pulse
        node.pulse += node.pulseSpeed;
      });

      // Draw flow lines
      flowLinesRef.current.forEach(line => {
        const fromNode = nodesRef.current[line.from];
        const toNode = nodesRef.current[line.to];
        
        if (!fromNode || !toNode) return;

        // Update progress
        line.progress += line.speed;
        if (line.progress > 1) line.progress = 0;

        // Calculate current position along the line
        const currentX = fromNode.x + (toNode.x - fromNode.x) * line.progress;
        const currentY = fromNode.y + (toNode.y - fromNode.y) * line.progress;

        // Draw the line
        ctx.beginPath();
        ctx.moveTo(fromNode.x, fromNode.y);
        ctx.lineTo(toNode.x, toNode.y);
        ctx.strokeStyle = line.color + '30';
        ctx.lineWidth = line.width;
        ctx.stroke();

        // Draw moving dot on the line
        ctx.beginPath();
        ctx.arc(currentX, currentY, 2, 0, Math.PI * 2);
        ctx.fillStyle = line.color;
        ctx.fill();

        // Glow effect
        const gradient = ctx.createRadialGradient(
          currentX, currentY, 0,
          currentX, currentY, 8
        );
        gradient.addColorStop(0, line.color + '80');
        gradient.addColorStop(1, line.color + '00');
        ctx.fillStyle = gradient;
        ctx.beginPath();
        ctx.arc(currentX, currentY, 8, 0, Math.PI * 2);
        ctx.fill();
      });

      // Draw nodes
      nodesRef.current.forEach(node => {
        const pulseScale = 1 + 0.2 * Math.sin(node.pulse);
        
        // Outer glow
        const gradient = ctx.createRadialGradient(
          node.x, node.y, 0,
          node.x, node.y, node.radius * 3
        );
        gradient.addColorStop(0, node.color + '40');
        gradient.addColorStop(1, node.color + '00');
        ctx.fillStyle = gradient;
        ctx.beginPath();
        ctx.arc(node.x, node.y, node.radius * 3, 0, Math.PI * 2);
        ctx.fill();

        // Main node
        ctx.beginPath();
        ctx.arc(node.x, node.y, node.radius * pulseScale, 0, Math.PI * 2);
        ctx.fillStyle = node.color;
        ctx.fill();

        // Inner highlight
        ctx.beginPath();
        ctx.arc(node.x, node.y, node.radius * 0.5 * pulseScale, 0, Math.PI * 2);
        ctx.fillStyle = '#ffffff80';
        ctx.fill();

        // Border
        ctx.beginPath();
        ctx.arc(node.x, node.y, node.radius * pulseScale, 0, Math.PI * 2);
        ctx.strokeStyle = '#ffffff30';
        ctx.lineWidth = 1;
        ctx.stroke();
      });

      // Update and draw particles
      particlesRef.current.forEach(particle => {
        particle.x += particle.vx;
        particle.y += particle.vy;
        particle.life--;

        // Wrap around edges
        if (particle.x < 0) particle.x = width;
        if (particle.x > width) particle.x = 0;
        if (particle.y < 0) particle.y = height;
        if (particle.y > height) particle.y = 0;

        // Respawn if dead
        if (particle.life <= 0) {
          particle.x = Math.random() * width;
          particle.y = Math.random() * height;
          particle.life = particle.maxLife;
        }

        // Draw particle with fade based on life
        const alpha = particle.life / particle.maxLife;
        ctx.beginPath();
        ctx.arc(particle.x, particle.y, particle.radius, 0, Math.PI * 2);
        ctx.fillStyle = particle.color.replace(')', `,${alpha})`).replace('rgb', 'rgba');
        ctx.fill();
      });

      // Draw center glow
      const centerGradient = ctx.createRadialGradient(
        centerX, centerY, 0,
        centerX, centerY, Math.min(width, height) * 0.4
      );
      centerGradient.addColorStop(0, 'rgba(0, 229, 218, 0.1)');
      centerGradient.addColorStop(1, 'rgba(0, 229, 218, 0)');
      ctx.fillStyle = centerGradient;
      ctx.beginPath();
      ctx.arc(centerX, centerY, Math.min(width, height) * 0.4, 0, Math.PI * 2);
      ctx.fill();

      animationFrameId = requestAnimationFrame(animate);
    };

    animationFrameId = requestAnimationFrame(animate);

    return () => {
      if (animationFrameId) {
        cancelAnimationFrame(animationFrameId);
      }
    };
  }, [isReady]);

  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const rect = e.currentTarget.getBoundingClientRect();
    mousePos.current = {
      x: e.clientX - rect.left,
      y: e.clientY - rect.top,
      isActive: true
    };
  };

  const handleMouseLeave = () => {
    mousePos.current.isActive = false;
  };

  return (
    <div className="relative w-full h-full">
      <canvas
        ref={canvasRef}
        onMouseMove={handleMouseMove}
        onMouseLeave={handleMouseLeave}
        className="w-full h-full rounded-xl border border-white/10 shadow-2xl"
      />
      
      {/* Overlay labels */}
      <div className="absolute inset-0 pointer-events-none">
        <motion.div
          className="absolute top-4 right-4 glass-card rounded-lg px-3 py-2 border border-green-500/20 bg-green-950/30 backdrop-blur-sm"
          initial={{ opacity: 0, y: -20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.5 }}
        >
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-green-400 animate-pulse" />
            <span className="text-xs font-mono text-green-400">CI/CD</span>
          </div>
        </motion.div>

        <motion.div
          className="absolute bottom-4 left-4 glass-card rounded-lg px-3 py-2 border border-blue-500/20 bg-blue-950/30 backdrop-blur-sm"
          initial={{ opacity: 0, y: 20 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.7 }}
        >
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-blue-400" />
            <span className="text-xs font-mono text-blue-400">Git Flow</span>
          </div>
        </motion.div>

        <motion.div
          className="absolute top-1/2 left-4 glass-card rounded-lg px-3 py-2 border border-purple-500/20 bg-purple-950/30 backdrop-blur-sm"
          initial={{ opacity: 0, x: -20 }}
          animate={{ opacity: 1, x: 0 }}
          transition={{ delay: 0.9 }}
        >
          <div className="flex items-center gap-2">
            <div className="w-2 h-2 rounded-full bg-purple-400" />
            <span className="text-xs font-mono text-purple-400">Audit</span>
          </div>
        </motion.div>
      </div>
    </div>
  );
}