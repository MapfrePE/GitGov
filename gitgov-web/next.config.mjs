/** @type {import('next').NextConfig} */
const nextConfig = {
    reactStrictMode: true,
    images: {
        formats: ['image/avif', 'image/webp'],
    },
    // Workaround: disable output file tracing which causes errors in App Router-only projects
    outputFileTracing: false,
};

export default nextConfig;
