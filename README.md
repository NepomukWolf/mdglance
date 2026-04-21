# mdglance

`mdglance` is a small native Markdown previewer for terminal-first workflows.

It opens an arbitrary Markdown file in a native macOS window, renders the document read-only, and refreshes when the source file changes. The intended loop is simple: write in a terminal editor, save, and glance at the rendered output without opening a full editor or browser workspace.

## Status

This is an early prototype.

Current behavior:

- Open a Markdown file from the CLI.
- Render common Markdown features with `pulldown-cmark`.
- Render fenced Mermaid blocks.
- Display local images.
- Show an optional table of contents sidebar.
- Refresh on save.
- Open external links in the default browser.
- Search rendered text with keyboard controls.
- Show keybindings in an in-app help overlay.

Mermaid is bundled into the binary at compile time from `assets/mermaid.min.js`, so diagram rendering does not require runtime network access. Do not treat this as a hardened renderer for untrusted Markdown yet.

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
./target/debug/mdglance examples/render-kitchen-sink.md
```

## Keybindings

| Key       | Action                                        |
| --------- | --------------------------------------------- |
| `j` / `k` | Scroll down / up in document mode             |
| `h` / `l` | Scroll left / right in document mode          |
| `d` / `u` | Half page down / up                           |
| `Space`   | Page down                                     |
| `g` / `G` | Top / bottom                                  |
| `/`       | Open search                                   |
| `n` / `N` | Next / previous search hit                    |
| `t`       | Toggle table of contents                      |
| `Tab`     | Switch focus between document and TOC         |
| `j` / `k` | TOC mode: next / previous heading             |
| `h` / `l` | TOC mode: parent / first child heading        |
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
scroll_left = ["h"]
scroll_right = ["l"]
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
toc_down = ["j"]
toc_up = ["k"]
toc_parent = ["h"]
toc_child = ["l"]
activate_selection = ["Enter"]
quit = ["q", "Cmd+W", "Cmd+Q"]
```

When you set a keybinding entry, that action's default bindings are replaced by the list you provide.
