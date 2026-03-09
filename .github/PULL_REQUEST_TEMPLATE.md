## Summary

<!-- Brief description of what this PR does (1-3 sentences) -->

## Changes

<!-- Bullet list of specific changes -->

-

## Type

<!-- Check one -->

- [ ] `feat` — New feature
- [ ] `fix` — Bug fix
- [ ] `refactor` — Code restructuring (no behavior change)
- [ ] `docs` — Documentation only
- [ ] `test` — Adding or updating tests
- [ ] `chore` — Build, CI, or tooling changes

## Golden Path Impact

<!-- Does this PR touch auth, tokens, API handlers, events, or dashboard? -->

- [ ] **No** — This PR does not affect the Golden Path
- [ ] **Yes** — I verified that Desktop → commit → push → /events → Dashboard still works

## Checklist

- [ ] `cargo test` passes (server)
- [ ] `cargo clippy -- -D warnings` passes (server + desktop)
- [ ] `npm run typecheck` passes (frontend)
- [ ] No new ESLint errors in changed files
- [ ] No secrets or credentials in the diff
- [ ] Shared structs stay in sync (`models.rs` ↔ `server.rs` ↔ `types.ts`)

## Testing

<!-- How was this tested? Manual steps, new unit tests, etc. -->

