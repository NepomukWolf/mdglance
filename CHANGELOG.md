# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project
adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.3.0] - 2026-11-05

### Added

- Preview multiple files by passing them as extra command-line arguments
  (`mdglance README.md ROADMAP.md`), alongside the existing piped stdin queue.
- Open the current file in `$VISUAL`/`$EDITOR` with a single keypress (`e`), in the same terminal
  `mdglance` was launched from, returning focus to the preview when the editor exits.
- Copy the current selection (Cmd+C) and select all (Cmd+A) within the previewed document, on
  macOS.

### Security

- Trusted-mode raw HTML is now sanitized with an allowlist (`ammonia`) instead of being passed
  through unmodified: `<script>`, inline event handlers, `<iframe>`, `<object>`, `<form>`, and
  unrecognized URL schemes (e.g. `javascript:`) are stripped, while formatting tags such as
  `<details>` and tables are preserved.
- Trusted documents now ship a Content Security Policy. Scripts are limited to the exact, hashed
  content of mdglance's own bundled scripts (no `'unsafe-inline'`), and `object-src`, `frame-src`,
  and `form-action` are disabled.
- See [`docs/security.md`](docs/security.md) and
  [`examples/security-sanitization-check.md`](examples/security-sanitization-check.md) for details
  and a file that exercises these protections directly.

## [0.2.0] - 2026-09-09

## [0.1.0] - 2026-09-08

[Unreleased]: https://github.com/NepomukWolf/mdglance/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/NepomukWolf/mdglance/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/NepomukWolf/mdglance/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/NepomukWolf/mdglance/releases/tag/v0.1.0
