# Security Policy

## Supported Versions

Only the latest release receives security fixes.

## Reporting a Vulnerability

Please do **not** open a public GitHub issue for security vulnerabilities.

Report vulnerabilities privately via GitHub's security advisory feature:
**Security → Report a vulnerability** in this repository.

Include:
- Description of the vulnerability
- Steps to reproduce
- Potential impact

You will receive a response within 7 days. If the issue is confirmed, a patch will be released and the reporter credited (unless anonymity is requested).

## Security Architecture

Left Hand Control is a local-only desktop app (no network server, no cloud).

**Attack surface:**
- Tauri IPC between the Vue frontend and the Rust backend
- evdev device access (requires user to be in the `input` group or run with `udev` rules)
- D-Bus calls for keyboard layout detection (read-only)
- YAML layout files parsed with `js-yaml` (no `eval` mode)

**Controls in place:**
- Tauri capability model: only `core:default` and `core:window:allow-set-title` are granted
- All Tauri commands return `Result<T, String>` — errors never crash the backend silently
- Layout file names are validated server-side (`validate_layout_name`) to prevent path traversal
- Device paths validated server-side (`validate_device_path`) to allow only `/dev/input/*` paths
- Command scripts require explicit user approval per layout fingerprint before they can execute

**Known limitations:**
- CSP `'unsafe-inline'` in `script-src` is required by Nuxt's generated hydration scripts (color mode detection, `window.__NUXT__` config injection). This is standard for Nuxt SSG and the risk is low given the app loads only local files.
- `'wasm-unsafe-eval'` is required for WebAssembly support used by some Nuxt UI internals.

## Dependency Auditing

- Node dependencies: `pnpm audit` runs in CI on every push
- Rust dependencies: `cargo audit` (via `rustsec/audit-check`) runs in CI on every push
- Dependabot is configured to open PRs for outdated npm, Cargo, and GitHub Actions dependencies weekly
