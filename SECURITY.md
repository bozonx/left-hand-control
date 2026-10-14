# Security Policy

## Supported Versions

Only the latest release of the Slint application receives security fixes.

## Reporting a Vulnerability

Please do **not** open a public GitHub issue for security vulnerabilities.
Report vulnerabilities privately via **Security → Report a vulnerability**
in this repository. Include a description, reproduction steps and potential impact.

You will receive a response within 7 days. If the issue is confirmed, a patch
will be released and the reporter credited unless anonymity is requested.

## Security Architecture

Left Hand Control is a local desktop application. Its core handles configuration,
keyboard interception and system integration; the Slint shell handles presentation.

**Attack surface:**

- Local IPC used by CLI clients and popup workers.
- Linux evdev/uinput device access and virtual input generation.
- D-Bus, compositor and desktop portal integrations.
- JSON configuration and YAML layouts parsed by Rust code.
- User-defined commands and application launching.

**Controls in place:**

- Unix IPC uses a private `0700` runtime directory; Windows uses token-authenticated
  loopback TCP. There is no remote service.
- Layout names and Linux device paths are validated in `lhc-core`.
- Configuration writes validate changes and check for unread external edits before
  replacement. This optimistic check does not serialize writes by external editors.
- Files are synced before replacement; Unix writes also sync the parent directory.
  Unix config and layout files are created with private `0600` permissions.
- The mapper loads user-defined commands only when commands are enabled in configuration.
- Debug builds use isolated development storage; tests and UI scenarios use temporary directories.

**Limitations:**

Keyboard interception requires access to physical input devices. Granting that
access lets the application read keyboard input. Configured commands execute
with the application's user permissions. Review imported layouts and commands
before enabling them.

## Dependency Auditing

Rust dependencies are audited in CI through `rustsec/audit-check`.
Dependabot checks the root Cargo workspace weekly and GitHub Actions monthly.
