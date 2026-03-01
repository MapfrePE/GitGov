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
                    50: '#FFF7ED',
                    100: '#FFEDD5',
                    200: '#FED7AA',
                    300: '#FDBA74',
                    400: '#FB923C',
                    500: '#F97316',
                    600: '#EA580C',
                    700: '#C2410C',
                    800: '#9A3412',
                    900: '#7C2D12',
                },
                accent: {
                    50: '#FFFBEB',
                    100: '#FEF3C7',
                    200: '#FDE68A',
                    300: '#FCD34D',
                    400: '#FBBF24',
                    500: '#F59E0B',
                    600: '#D97706',
                    700: '#B45309',
                    800: '#92400E',
                    900: '#78350F',
                },
                surface: {
                    DEFAULT: '#090909',
                    50: '#1A1A1A',
                    100: '#141414',
                    200: '#0F0F0F',
                    300: '#090909',
                    400: '#050505',
                    500: '#030303',
                },
                glass: {
                    light: 'rgba(255, 255, 255, 0.06)',
                    medium: 'rgba(255, 255, 255, 0.1)',
                    heavy: 'rgba(255, 255, 255, 0.15)',
                    border: 'rgba(255, 255, 255, 0.08)',
                },
            },
            fontFamily: {
                sans: ['Outfit', 'system-ui', '-apple-system', 'sans-serif'],
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
                'glow': '0 0 20px rgba(249, 115, 22, 0.25)',
                'glow-lg': '0 0 40px rgba(249, 115, 22, 0.3)',
                'glow-accent': '0 0 20px rgba(251, 191, 36, 0.2)',
                'glass': '0 8px 32px rgba(0, 0, 0, 0.5)',
            },
        },
    },
    plugins: [
        require('@tailwindcss/typography'),
    ],
};

export default config;
