import type { Config } from 'tailwindcss';

const config: Config = {
    content: [
        './app/**/*.{ts,tsx}',
        './components/**/*.{ts,tsx}',
        './lib/**/*.{ts,tsx}',
    ],
    theme: {
        extend: {
            colors: {
                brand: {
                    50: '#e6fffe',
                    100: '#b3fffc',
                    200: '#80fff9',
                    300: '#4dfff7',
                    400: '#1afff4',
                    500: '#00e5da',
                    600: '#00b8ae',
                    700: '#008a83',
                    800: '#005c57',
                    900: '#002e2c',
                },
                accent: {
                    50: '#fff7e6',
                    100: '#ffe8b3',
                    200: '#ffd980',
                    300: '#ffca4d',
                    400: '#ffbb1a',
                    500: '#e6a200',
                    600: '#b37e00',
                    700: '#805a00',
                    800: '#4d3600',
                    900: '#1a1200',
                },
                surface: {
                    DEFAULT: '#0a0e1a',
                    50: '#1a1f2e',
                    100: '#141829',
                    200: '#0f1322',
                    300: '#0a0e1a',
                    400: '#070a14',
                    500: '#04060d',
                },
                glass: {
                    light: 'rgba(255, 255, 255, 0.06)',
                    medium: 'rgba(255, 255, 255, 0.1)',
                    heavy: 'rgba(255, 255, 255, 0.15)',
                    border: 'rgba(255, 255, 255, 0.08)',
                },
            },
            fontFamily: {
                sans: ['Inter', 'system-ui', '-apple-system', 'sans-serif'],
                mono: ['JetBrains Mono', 'Fira Code', 'monospace'],
            },
            fontSize: {
                'display-xl': ['4.5rem', { lineHeight: '1.1', letterSpacing: '-0.03em' }],
                'display': ['3.5rem', { lineHeight: '1.15', letterSpacing: '-0.02em' }],
                'heading': ['2.25rem', { lineHeight: '1.2', letterSpacing: '-0.01em' }],
            },
            animation: {
                'float': 'float 6s ease-in-out infinite',
                'float-delayed': 'float 6s ease-in-out 2s infinite',
                'float-slow': 'float 8s ease-in-out infinite',
                'pulse-glow': 'pulseGlow 3s ease-in-out infinite',
                'gradient-shift': 'gradientShift 8s ease infinite',
                'slide-up': 'slideUp 0.6s ease-out',
                'fade-in': 'fadeIn 0.8s ease-out',
                'spin-slow': 'spin 20s linear infinite',
            },
            keyframes: {
                float: {
                    '0%, 100%': { transform: 'translateY(0px)' },
                    '50%': { transform: 'translateY(-20px)' },
                },
                pulseGlow: {
                    '0%, 100%': { opacity: '0.4', transform: 'scale(1)' },
                    '50%': { opacity: '0.8', transform: 'scale(1.05)' },
                },
                gradientShift: {
                    '0%': { backgroundPosition: '0% 50%' },
                    '50%': { backgroundPosition: '100% 50%' },
                    '100%': { backgroundPosition: '0% 50%' },
                },
                slideUp: {
                    '0%': { opacity: '0', transform: 'translateY(30px)' },
                    '100%': { opacity: '1', transform: 'translateY(0)' },
                },
                fadeIn: {
                    '0%': { opacity: '0' },
                    '100%': { opacity: '1' },
                },
            },
            backdropBlur: {
                xs: '2px',
            },
            boxShadow: {
                'glow': '0 0 20px rgba(0, 229, 218, 0.15)',
                'glow-lg': '0 0 40px rgba(0, 229, 218, 0.2)',
                'glow-accent': '0 0 20px rgba(255, 187, 26, 0.15)',
                'glass': '0 8px 32px rgba(0, 0, 0, 0.3)',
            },
        },
    },
    plugins: [
        require('@tailwindcss/typography'),
    ],
};

export default config;
