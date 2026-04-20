# Roadmap

This project should stay focused: a fast native Markdown previewer for people writing in terminal editors.

## Near Term

- Vendor Mermaid locally.
  - Remove the runtime CDN dependency.
  - Pin the Mermaid version.
  - Make diagrams work offline.

- Harden Markdown rendering.
  - Escape or strip raw HTML by default.
  - Add an explicit `--unsafe-html` flag if raw HTML is needed.
  - Block dangerous URL schemes such as `javascript:`.
  - Revisit Mermaid `securityLevel` and avoid permissive defaults where possible.

- Improve local asset handling.
  - Watch referenced local images and refresh when they change.
  - Consider size limits for embedded images.
  - Consider a custom asset protocol instead of embedding large files as data URLs.

- Improve search.
  - Preserve the current search query and selected hit across reloads.
  - Keep match count visible after accepting search.
  - Consider case-sensitive search only when the query contains uppercase letters.

- Add install documentation.
  - `cargo install --path .`
  - Optional shell alias.
  - Later, a Homebrew tap.

## Rendering

- Add syntax highlighting.
  - Prefer an offline approach, such as Rust-side `syntect`, if it keeps startup fast.

- Improve local Markdown links.
  - Open relative `.md` links inside `mdview`.
  - Keep external links in the default browser.
  - Add Markdown-only back/forward history.

- Polish styling.
  - Follow the system light/dark theme.
  - Add a small theme configuration surface.
  - Improve table, code block, and blockquote spacing.

- Add reload feedback.
  - Show a subtle "updated" timestamp or flash after successful refresh.
  - Show readable Mermaid/rendering errors inline.

## CLI

- Add useful flags.
  - `--no-watch`
  - `--no-open-external`
  - `--theme light|dark|system`
  - `--unsafe-html`
  - `--window-size WIDTHxHEIGHT`

- Improve path resolution.
  - Resolve `README` to `README.md` when obvious.
  - Provide clearer errors for missing files.

- Add release builds.
  - Document `cargo build --release`.
  - Add packaging notes for macOS.
  - Consider signed binaries later.

## Open Source Readiness

- Add a license.
  - MIT or dual MIT/Apache-2.0 would fit a small Rust utility.

- Add screenshots or a short demo GIF.

- Add a clear security section.
  - Explain the trusted/untrusted Markdown boundary.
  - Document how external links are handled.
  - Document the Mermaid dependency model.

- Add basic tests.
  - Markdown-to-HTML rendering tests.
  - Mermaid block conversion tests.
  - Local image rewriting tests.

## Things To Avoid For Now

- Editing.
- Workspaces.
- Vaults.
- Plugin systems.
- Full browser behavior inside the preview window.
- Turning the tool into a Markdown project manager.

