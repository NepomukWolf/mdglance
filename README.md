# mdglance

`mdglance` is a small native document previewer for terminal-first workflows.

It opens a Markdown or SVG file in a native window, renders the document read-only, and refreshes when the source file changes. The intended loop is simple: write in a terminal editor, save, and glance at the rendered result without opening a full editor or browser workspace. The app is designed to stay keyboard-first: navigation, search, TOC use, link following, history, and SVG pan/zoom are all available without touching the mouse.

## Status

This is still an early prototype, but it is already usable as a keyboard-first Markdown and SVG viewer for terminal workflows.

[Mermaid](https://mermaid.js.org/) is bundled into the binary at compile time from `assets/mermaid.min.js`, so Mermaid rendering does not require runtime network access. [PlantUML](https://plantuml.com/) blocks are rendered locally through the `plantuml` CLI when it is available. Do not treat this as a hardened renderer for untrusted Markdown yet.

## Usage

Run from the repository:

```sh
cargo run -- path/to/file.md
```

Or pipe a newline-separated file list into the viewer queue:

```sh
fd -e svg | cargo run --
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
- System-aware light/dark themes plus configurable Tokyo Night and Gruvbox presets.
- Table of contents sidebar with keyboard focus mode and section tracking.
- In-viewer navigation for relative `.md` links with back/forward history.
- Optional stdin-driven file queue with previous/next navigation.
- Keyboard link hints for opening visible links quickly.
- Per-document scroll memory while moving between Markdown files.
- Native SVG preview mode with fit-to-window, pan, zoom, and reset view.
- Syntax highlighting for fenced code blocks with explicit language tags.
- [Mermaid](https://mermaid.js.org/) diagram rendering without runtime network access.
- Local [PlantUML](https://plantuml.com/) diagram rendering through the `plantuml` CLI, with graceful fallback to code blocks when unavailable or rendering fails.
- Local image support for common raster and SVG formats.

## Keybindings

| Key       | Action                                        |
| --------- | --------------------------------------------- |
| `j` / `k` | Scroll down / up in document mode             |
| `h` / `l` | Back / forward through Markdown history       |
| `[` / `]` | Previous / next file in the viewer queue      |
| `d` / `u` | Half page down / up                           |
| `Space`   | Page down                                     |
| `g` / `G` | Top / bottom                                  |
| `h` / `l` | SVG mode: pan left / right                    |
| `j` / `k` | SVG mode: pan down / up                       |
| `=` / `+` | SVG mode: zoom in                             |
| `-`       | SVG mode: zoom out                            |
| `0`       | SVG mode: reset fitted view                   |
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

## Diagrams

Fenced [Mermaid](https://mermaid.js.org/) blocks are rendered in the preview:

````markdown
```mermaid
flowchart LR
  A[Markdown] --> B[Preview]
```
````

Fenced [PlantUML](https://plantuml.com/) blocks are rendered locally when the `plantuml` CLI is installed:

````markdown
```plantuml
@startuml
Alice -> Bob: hello
@enduml
```
````

## SVG Preview

Open an SVG file directly to preview it with fit-to-window scaling:

```sh
cargo run -- examples/diagram.svg
```

In SVG mode, Markdown-specific features such as TOC, search, and link navigation are disabled. The dedicated SVG controls are pan with `h` `j` `k` `l`, zoom with `=`/`+` and `-`, and reset view with `0`.

## File Queue

When no file argument is provided and stdin is not a TTY, `mdglance` reads newline-separated file paths from stdin, opens the first file, and keeps the rest as a viewer queue:

```sh
fd -e svg | mdglance
```

Use `[` and `]` to move to the previous and next file in that queue. The window title shows the current queue position while you are on a queued file.

## Security Notes

`mdglance` renders Markdown inside a native WebView. That is useful, but it also means Markdown rendering needs a clear security model.

Before previewing untrusted Markdown, the project should harden these areas:

- Strip or escape raw HTML by default.
- Block dangerous link schemes such as `javascript:`.
- Keep vendored [Mermaid](https://mermaid.js.org/) pinned and reviewed.
- Treat local [PlantUML](https://plantuml.com/) execution as part of the trusted local toolchain.
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

## License

MIT. See [LICENSE](/Users/wolf/dev/mdview/LICENSE).

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
maximized = false
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
back = ["h"]
forward = ["l"]
previous_file = ["["]
next_file = ["]"]
open_link_hints = ["f"]
toc_down = ["j"]
toc_up = ["k"]
activate_selection = ["Enter"]
zoom_in = ["=", "Shift+="]
zoom_out = ["-"]
reset_view = ["0"]
quit = ["q"]
```

Set `maximized = true` to fill the usable desktop while keeping the native title bar and window
controls. The configured width and height are used when the window is restored. Maximized and
fullscreen modes cannot be enabled at the same time.

### Themes

The viewer follows the operating system's light or dark appearance by default. Choose a fixed
preset with `theme.preset`:

```toml
[theme]
preset = "tokyo-night" # system, light, dark, tokyo-night, or gruvbox
```

`tokyo-night` uses the Tokyo Night Night terminal palette, while `gruvbox` uses Gruvbox Dark
Medium. Theme colors and syntax highlighting can be overridden independently for the light and
dark variants. Overrides are applied on top of the selected preset:

```toml
[theme]
preset = "system"

[theme.light]
heading = "#24292f"
link = "#0969da"
syntax_theme = "inspired-github"

[theme.dark]
background = "#1a1b26"
text = "#c0caf5"
heading = "#bb9af7"
link = "#7aa2f7"
code_background = "#16161e"
syntax_theme = "tokyo-night"
```

Semantic color fields accept six-digit hexadecimal colors (`#RRGGBB`): `background`, `surface`,
`text`, `muted_text`, `heading`, `link`, `border`, `divider`, `code_background`,
`sidebar_background`, `accent`, `search_match`, and `error`.

Available syntax themes are `inspired-github`, `solarized-dark`, `solarized-light`,
`base16-eighties-dark`, `base16-mocha-dark`, `base16-ocean-dark`, `base16-ocean-light`,
`tokyo-night`, and `gruvbox`. Syntax highlighting applies to fenced code blocks whose language is
recognized; other code inherits the active theme's foreground and code background.

You can also use an external TextMate/Syntect `.tmTheme` file for code highlighting:

```toml
[theme.dark]
syntax_theme_file = "~/.config/mdglance/themes/custom-dark.tmTheme"
```

With the `system` preset, light and dark variants can load different files. Relative paths are
resolved from the directory containing the active `mdglance.toml` or `config.toml`; `~/` expands to
your home directory. `syntax_theme` and `syntax_theme_file` cannot both be set in the same variant.
The file controls code-token styling only—the semantic viewer colors continue to come from the
preset and the color overrides listed above.

On macOS, clicking the green window button normally enters Tao's borderless fullscreen mode, which
hides the native controls. Option-click the green button to use macOS Zoom instead.

When you set a keybinding entry, that action's default bindings are replaced by the list you provide.
On macOS, the built-in defaults also include `Cmd+W` and `Cmd+Q`.
