/**
 * Analytics scaffold – no-op implementation.
 * Replace with real provider (GA4, PostHog, etc.) when ready.
 */

type EventProperties = Record<string, string | number | boolean>;

export const analytics = {
    /** Track a page view */
    pageView(url: string): void {
        if (process.env.NODE_ENV === 'development') {
            console.log(`[Analytics] Page view: ${url}`);
        }
    },

    /** Track a custom event */
    event(name: string, properties?: EventProperties): void {
        if (process.env.NODE_ENV === 'development') {
            console.log(`[Analytics] Event: ${name}`, properties);
        }
    },

    /** Track a download intent */
    download(fileName: string, platform: string): void {
        if (process.env.NODE_ENV === 'development') {
            console.log(`[Analytics] Download: ${fileName} (${platform})`);
        }
    },

    /** Identify a user (future use) */
    identify(userId: string, traits?: EventProperties): void {
        if (process.env.NODE_ENV === 'development') {
            console.log(`[Analytics] Identify: ${userId}`, traits);
        }
    },
};
