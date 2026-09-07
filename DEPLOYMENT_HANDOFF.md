# Public deployment handoff

Updated 2026-09-07. Project: C:\Users\user\Downloads\MelodyPath. Branch: feature/public-deployment.

## Current stage

Deployment engineering and final checks complete. WAITING_FOR_USER_DEPLOYMENT_ACTION. No public URL, Render account/service/payment/source upload performed. README unchanged. No push/merge/rebase/force. Tested implementation commit: a3748125be5a4d995ed0aabc05bf2041a57215c9. Base commit: c1bb17fe96d3ded1adf65d703e83fbbc1239038e. This handoff is a subsequent documentation-only commit; use git rev-parse HEAD for its exact hash.

Primary architecture: Render Docker Web Service Starter, single instance, 1 GB persistent disk /var/data; Axum serves static frontend and API over one Render HTTPS origin. Linux OAuth files use AES-256-GCM with Environment master key. Browser workspaces isolate SQLite, histories, tasks, imports and previews. Signed Secure/HttpOnly/SameSite cookies, exact-origin writes, bounded request admission. Active OAuth states and Copy runs remain process-local; no cross-restart run recovery claim.

Production-only PUBLIC_DEMO_EXPIRES_AT drives /api/public-config and lightweight notice. Local does not show it. Notice explicitly distinguishes free-of-credentials public Demo use from self-host requiring own credentials; clone never shares deployer Secrets. Expiration does not stop service or delete data. README and course-message template are in docs/public_deployment.md pending real public success.

## Final verification

- cargo fmt --all --check: passed.
- cargo check --workspace: passed, existing dead-code warnings.
- cargo test --workspace: 135 passed, 0 failed, 1 ignored (opt-in real network probe).
- frontend lint/build and production bundle audit: passed.
- Windows cargo build --release --locked: passed.
- WSL Ubuntu cargo test --release --locked: 133 passed, 0 failed, 1 ignored; two Windows-only DPAPI tests excluded.
- WSL Ubuntu release build: passed.
- Windows and Linux production smoke: passed, including safe public config, static SPA routes, exact callback URLs, Secure cookie, CSRF, fail-closed configuration. Linux SIGTERM exits successfully.
- Frontend final test:e2e -- --workers=1: 14 passed. Earlier concurrent release builds caused one 30-second modal timeout; unchanged assertions passed with single worker. No Spotify logic changed.
- Final production bundle/tracked-source audit and git diff --check: passed. Pattern audit is not exhaustive.
- Earlier failed production path extraction test was fixed by clearing outer wildcard request extensions before inner routing. Isolation regression now passes.
- Initial new browser fixture returned an incorrect object for array endpoints; fixture corrected and final rerun passed. Windows smoke originally omitted SYSTEMROOT due to case filtering; corrected isolated environment allowlist and rerun passed.
- DOCKER_BUILD_NOT_LOCALLY_VERIFIED. Docker absent; not installed.

## Modified files

.env.example, .gitattributes, Cargo.lock, backend/Cargo.toml, backend/src/main.rs, backend/src/secure_store.rs, backend/src/writers/spotify.rs, backend/src/writers/youtube.rs, docs/deployment.md, frontend/src/api.ts, frontend/src/main.tsx, frontend/src/styles.css.

New: Dockerfile, .dockerignore, render.yaml, backend/src/deployment.rs, backend/src/public_host.rs, frontend/src/DemoNotice.tsx, frontend/tests/demo-notice.spec.ts, deploy/entrypoint.sh, deploy/audit-production.mjs, deploy/smoke-production.py, docs/public_deployment.md, DEPLOYMENT_HANDOFF.md.

## Real platform status

User-confirmed local Spotify and YouTube OAuth, identity, playlists and official URL import passed. Last.fm real provider passed. No new public OAuth acceptance yet. Real final Copy/write and Version Radar remain unconfirmed. All deployment smoke credentials are synthetic; no real exchange claimed. No manual OAuth currently underway.

ROTATE before public deployment: Spotify Client Secret; Google Client Secret. Old screenshot values must not be used. New Secrets only user-entered Render Environment, never sent to the assistant or put in files/logs.

## Next manual stage

WAITING_FOR_USER_DEPLOYMENT_ACTION. Cost estimate: Hobby workspace $0, Starter $7/month, 1 GB disk $0.25/month; approximately $3.39 for 14 days using 30-day estimate, plus overages/tax. User must confirm pricing before purchase. Full fields, official sources and steps: docs/public_deployment.md. Recommend entering controlled course-demo deployment; not unrestricted production acceptance. Real Docker build, actual Render memory/health and public OAuth still require verification.

1. User makes reviewed source available to Render (current local commit is not remotely available; assistant must not push).
2. User logs into Render, New Web Service, Docker, branch feature/public-deployment, Starter, single instance, /health, automatic deployment off, /var/data 1 GB disk.
3. Obtain actual HTTPS origin; set PUBLIC_BASE_URL and actual opening date + about 14 days as PUBLIC_DEMO_EXPIRES_AT.
4. Rotate provider Client Secrets; enter Environment variable values personally. Names and public settings are documented in docs/public_deployment.md and render.yaml.
5. Register actual-origin/api/spotify/callback and actual-origin/api/youtube/callback in official dashboards.
6. Send only public URL, health/build status and configured variable names; no credentials, authorization code, tokens or cookies.
7. Real public OAuth Allow and playlist tests then performed by user. Only after success propose small README update and publish course message with actual URL/date.

## Resume commands

```powershell
Set-Location C:\Users\user\Downloads\MelodyPath
git branch --show-current
git status --short
git diff --check
git log -5 --oneline
cargo fmt --all --check
cargo check --workspace
cargo test --workspace
cargo build --release --locked
npm --prefix frontend run lint
npm --prefix frontend run build
npm --prefix frontend run test:e2e
node deploy/audit-production.mjs
python deploy/smoke-production.py target/release/melody-path-api.exe frontend/dist
```

Linux uses WSL Ubuntu with target directory /tmp/melodypath-public-deployment-build. These checks are complete; do not repeat them without code changes or a new failure. Resume from the manual Render steps above, then real public acceptance. Do not rebuild the architecture or read actual credential stores.

Git status at handoff: implementation committed; only this new DEPLOYMENT_HANDOFF.md awaits its documentation commit. After that commit, expected clean working tree. No pending source edits, no real OAuth Allow currently underway. The final conversation reports the exact handoff commit hash.
