# mdglance

`mdglance` is a small native Markdown previewer for terminal-first workflows.

It opens an arbitrary Markdown file in a native macOS window, renders the document read-only, and refreshes when the source file changes. The intended loop is simple: write in a terminal editor, save, and glance at a rendered document without opening a full editor or browser workspace. The app is designed to stay keyboard-first: navigation, search, TOC use, link following, and history are all available without touching the mouse.

## Status

This is still an early prototype, but it is already usable as a keyboard-first Markdown viewer for terminal workflows.

Mermaid is bundled into the binary at compile time from `assets/mermaid.min.js`, so diagram rendering does not require runtime network access. Do not treat this as a hardened renderer for untrusted Markdown yet.

## Usage

Run from the repository:

```sh
cargo run -- path/to/file.md
```

Return the shell prompt immediately:

```sh
cargo run -- --detach path/to/file.md
```

Try the included test document:

```sh
cargo run -- examples/render-kitchen-sink.md
```

Build a local debug binary:

```sh
cargo build
./target/debug/mdglance examples/render-kitchen-sink.md
./target/debug/mdglance --detach examples/render-kitchen-sink.md
```

## Features

- Keyboard-first document viewer with no mouse required for core navigation.
- Native window with live reload on file save.
- Configurable keybindings and viewer settings via `mdglance.toml`.
- Table of contents sidebar with keyboard focus mode and section tracking.
- In-viewer navigation for relative `.md` links with back/forward history.
- Keyboard link hints for opening visible links quickly.
- Per-document scroll memory while moving between Markdown files.
- Syntax highlighting for fenced code blocks with explicit language tags.
- Mermaid diagram rendering without runtime network access.
- Local image support for common raster and SVG formats.

## Keybindings

| Key       | Action                                        |
| --------- | --------------------------------------------- |
| `j` / `k` | Scroll down / up in document mode             |
| `h` / `l` | Back / forward through Markdown history       |
| `d` / `u` | Half page down / up                           |
| `Space`   | Page down                                     |
| `g` / `G` | Top / bottom                                  |
| `/`       | Open search                                   |
| `n` / `N` | Next / previous search hit                    |
| `f`       | Open keyboard link hints                      |
| `t`       | Toggle table of contents                      |
| `Tab`     | Switch focus between document and TOC         |
| `j` / `k` | TOC mode: next / previous heading             |
| `Enter`   | Accept search or jump to selected TOC heading |
| `?`       | Show help                                     |
| `Esc`     | Close search/help                             |
| `q`       | Quit                                          |

## Mermaid

Fenced Mermaid blocks are rendered in the preview:

````markdown
```mermaid
flowchart LR
  A[Markdown] --> B[Preview]
```
````

## Security Notes

`mdglance` renders Markdown inside a native WebView. That is useful, but it also means Markdown rendering needs a clear security model.

Before previewing untrusted Markdown, the project should harden these areas:

- Strip or escape raw HTML by default.
- Block dangerous link schemes such as `javascript:`.
- Keep vendored Mermaid pinned and reviewed.
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

## Configuration

`mdglance` resolves config from one of two locations:

1. `./mdglance.toml` in the directory where you invoked the CLI
2. `~/.config/mdglance/config.toml` if no project-local file is present

Defaults stay in the binary, so config is optional.

Example:

```toml
[toc]
visible_on_start = false
max_depth = 3

[window]
width = 1280
height = 900
fullscreen = false

[keybindings]
scroll_down = ["j"]
scroll_up = ["k"]
half_page_down = ["d"]
half_page_up = ["u"]
page_down = ["Space"]
top = ["g"]
bottom = ["Shift+G"]
open_search = ["/"]
accept_search = ["Enter"]
next_search_hit = ["n"]
previous_search_hit = ["Shift+N"]
show_help = ["?"]
close_overlay = ["Escape"]
toggle_toc = ["t"]
toggle_focus = ["Tab"]
back = ["h"]
forward = ["l"]
open_link_hints = ["f"]
toc_down = ["j"]
toc_up = ["k"]
activate_selection = ["Enter"]
quit = ["q", "Cmd+W", "Cmd+Q"]
```

When you set a keybinding entry, that action's default bindings are replaced by the list you provide.
