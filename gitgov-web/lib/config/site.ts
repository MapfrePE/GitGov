export const siteConfig = {
    name: 'GitGov',
    description: 'Distributed Git Governance — Full traceability from commit to compliance.',
    tagline: 'Governance. Traceability. Compliance.',
    url: 'https://git-gov.vercel.app',
    ogImage: '/images/og/flag.jpg',

    version: '0.1.0',
    downloadFileName: 'GitGov_0.1.0_x64-setup.exe',
    downloadPath: '/downloads/GitGov_0.1.0_x64-setup.exe',
    downloadChecksum: 'sha256:pending-build',  // placeholder

    links: {
        docs: '/docs',
        download: '/download',
        features: '/features',
        contact: '/contact',
        pricing: '/pricing',
    },

    nav: [
        { label: 'Features', href: '/features' },
        { label: 'Download', href: '/download' },
        { label: 'Docs', href: '/docs' },
        { label: 'Pricing', href: '/pricing' },
        { label: 'Contact', href: '/contact' },
    ],

    footer: {
        product: [
            { label: 'Features', href: '/features' },
            { label: 'Download', href: '/download' },
            { label: 'Pricing', href: '/pricing' },
        ],
        resources: [
            { label: 'Documentation', href: '/docs' },
            { label: 'Installation Guide', href: '/docs/installation' },
            { label: 'Control Plane Setup', href: '/docs/control-plane' },
        ],
        company: [
            { label: 'Contact', href: '/contact' },
        ],
    },

    copy: {
        heroTitle: 'Git Governance,\nUnified.',
        heroSubtitle: 'Full traceability from commit to CI to compliance. One platform for engineering teams that take operational evidence seriously.',
        heroCta: 'Download Desktop',
        heroCtaSecondary: 'Explore Docs',
        whatIs: 'GitGov is a distributed governance system that connects every Git commit to its CI pipeline, Jira ticket, and compliance audit trail — giving CTOs, CISOs, and engineering managers the visibility they need.',
        problemTitle: 'The Problem',
        problemDescription: 'Engineering teams ship code without a clear audit trail. Commits happen, pipelines run, tickets close — but nobody can trace the full chain of evidence when compliance asks.',
        solutionTitle: 'The Solution',
        solutionDescription: 'GitGov captures every operation at the source — the developer\'s machine — and correlates it through your CI and project management tools, creating an immutable record of execution.',
    },
} as const;

export type SiteConfig = typeof siteConfig;
