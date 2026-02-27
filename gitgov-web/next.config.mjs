/** @type {import('next').NextConfig} */
const nextConfig = {
    reactStrictMode: true,
    images: {
        formats: ['image/avif', 'image/webp'],
    },
    poweredByHeader: false,
    experimental: {
        outputFileTracingIncludes: {
            '/docs/[[...slug]]': ['./content/**/*'],
        },
    },
};

export default nextConfig;
