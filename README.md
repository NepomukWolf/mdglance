# mdview

`mdview` is a small native Markdown previewer for terminal-first workflows.

It opens an arbitrary Markdown file in a native macOS window, renders the document read-only, and refreshes when the source file changes. The intended loop is simple: write in a terminal editor, save, and glance at the rendered output without opening a full editor or browser workspace.

## Status

This is an early prototype.

Current behavior:

- Open a Markdown file from the CLI.
- Render common Markdown features with `pulldown-cmark`.
- Render fenced Mermaid blocks.
- Display local images.
- Refresh on save.
- Open external links in the default browser.
- Search rendered text with keyboard controls.
- Show keybindings in an in-app help overlay.

Mermaid is currently loaded from a CDN. Do not treat this as a hardened renderer for untrusted Markdown yet.

## Usage

Run from the repository:

```sh
cargo run -- path/to/file.md
```

Try the included test document:

```sh
cargo run -- examples/render-kitchen-sink.md
```

Build a local debug binary:

```sh
cargo build
./target/debug/mdview examples/render-kitchen-sink.md
```

## Keybindings

| Key | Action |
| --- | --- |
| `j` / `k` | Scroll down / up |
| `h` / `l` | Scroll left / right |
| `d` / `u` | Half page down / up |
| `Space` | Page down |
| `g` / `G` | Top / bottom |
| `/` | Open search |
| `Enter` | Accept search |
| `n` / `N` | Next / previous search hit |
| `?` | Show help |
| `Esc` | Close search/help |
| `q` | Quit |

## Mermaid

Fenced Mermaid blocks are rendered in the preview:

````markdown
```mermaid
flowchart LR
  A[Markdown] --> B[Preview]
```
````

## Security Notes

`mdview` renders Markdown inside a native WebView. That is useful, but it also means Markdown rendering needs a clear security model.

Before previewing untrusted Markdown, the project should harden these areas:

- Strip or escape raw HTML by default.
- Block dangerous link schemes such as `javascript:`.
- Vendor Mermaid locally instead of loading it from a CDN.
- Keep app IPC minimal and validated.
- Keep external sites out of the preview WebView.

External `http` and `https` links are currently opened in the default browser instead of navigating inside the preview window.

## Development

Format and check:

```sh
cargo fmt
cargo check
```

Build:

```sh
cargo build
```

