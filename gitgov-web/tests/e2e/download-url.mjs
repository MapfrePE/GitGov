/**
 * E2E smoke test: verifies /api/release-metadata shape and that the download button
 * URL is external when NEXT_PUBLIC_DESKTOP_DOWNLOAD_URL is configured.
 *
 * Usage:
 *   # With a running dev/prod server:
 *   node gitgov-web/tests/e2e/download-url.mjs
 *
 *   # With a custom base URL:
 *   TEST_BASE_URL=https://git-gov.vercel.app node gitgov-web/tests/e2e/download-url.mjs
 *
 *   # To also check external URL matching:
 *   NEXT_PUBLIC_DESKTOP_DOWNLOAD_URL=https://github.com/.../GitGov_0.1.0_x64-setup.exe \
 *     node gitgov-web/tests/e2e/download-url.mjs
 */

const BASE_URL = process.env.TEST_BASE_URL ?? 'http://localhost:3000';
const CONFIGURED_URL = process.env.NEXT_PUBLIC_DESKTOP_DOWNLOAD_URL ?? '';

let passed = 0;
let failed = 0;

function assert(condition, message) {
    if (condition) {
        console.log(`  ✓ ${message}`);
        passed++;
    } else {
        console.error(`  ✗ ${message}`);
        failed++;
    }
}

async function main() {
    console.log(`\nGitGov download-url E2E test`);
    console.log(`Base URL : ${BASE_URL}`);
    console.log(`─────────────────────────────`);

    // ── 1. /api/release-metadata shape ──────────────────────────────────────
    console.log('\n[1] GET /api/release-metadata');

    let data;
    try {
        const res = await fetch(`${BASE_URL}/api/release-metadata`);
        assert(res.ok, `returns 2xx (got ${res.status})`);
        assert(
            res.headers.get('content-type')?.includes('application/json'),
            'content-type is application/json',
        );
        data = await res.json();
    } catch (err) {
        console.error(`  ✗ fetch failed: ${err.message}`);
        failed++;
        printSummary();
        return;
    }

    assert(typeof data.version === 'string' && data.version.length > 0, 'response.version is a non-empty string');
    assert(typeof data.downloadUrl === 'string' && data.downloadUrl.length > 0, 'response.downloadUrl is a non-empty string');
    assert(typeof data.checksum === 'string' && data.checksum.length > 0, 'response.checksum is a non-empty string');
    assert(typeof data.available === 'boolean', 'response.available is a boolean');
    assert('msiUrl' in data, 'response has msiUrl field (may be null)');

    // ── 2. External URL checks (only when env var is set) ───────────────────
    console.log('\n[2] External URL checks');

    if (CONFIGURED_URL) {
        assert(
            data.downloadUrl === CONFIGURED_URL,
            `downloadUrl matches NEXT_PUBLIC_DESKTOP_DOWNLOAD_URL`,
        );
        assert(
            /^https?:\/\//i.test(data.downloadUrl),
            'downloadUrl is an external http/https URL',
        );
        assert(data.available === true, 'available is true when external URL is configured');
    } else {
        console.log('  ~ NEXT_PUBLIC_DESKTOP_DOWNLOAD_URL not set; skipping external URL checks');
    }

    // ── 3. Checksum format ───────────────────────────────────────────────────
    console.log('\n[3] Checksum format');
    const checksumIsPlaceholder = data.checksum === 'sha256:pending-build';
    if (checksumIsPlaceholder) {
        console.log('  ~ checksum is placeholder (sha256:pending-build) — set NEXT_PUBLIC_DESKTOP_DOWNLOAD_CHECKSUM to populate');
    } else {
        assert(
            /^sha256:[a-f0-9]{64}$/.test(data.checksum),
            `checksum matches sha256:<64-hex-chars> format`,
        );
    }

    printSummary();
}

function printSummary() {
    console.log(`\n─────────────────────────────`);
    console.log(`Results: ${passed} passed, ${failed} failed\n`);
    if (failed > 0) process.exit(1);
}

main().catch((err) => {
    console.error('Unhandled error:', err);
    process.exit(1);
});
