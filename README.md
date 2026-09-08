# mdglance

`mdglance` is a small, keyboard-first Markdown and SVG previewer for terminal workflows.

It is intended for people who write in terminal editors and want to check the rendered result
without opening a full editor, browser workspace, or project-oriented Markdown application. Point
it at a file, keep writing, and glance at the native preview whenever you save.

The viewer is deliberately read-only and designed to stay out of the writing loop. Live reload,
navigation, search, the table of contents, link following, and SVG controls are all available from
the keyboard, so previewing does not require switching tools or reaching for the mouse.

> [!NOTE]
> This is an early prototype developed and tested on macOS. It is not yet hardened for untrusted
> Markdown.

## Features

- Live, read-only Markdown and SVG preview
- Keyboard navigation, search, table of contents, link hints, and Markdown history
- File queues supplied as newline-separated paths on standard input
- Offline Mermaid rendering and optional local PlantUML rendering
- Syntax highlighting and configurable light/dark themes
- Local image support

## Run from source

You need a current Rust toolchain.

```sh
git clone https://github.com/NepomukWolf/mdglance.git
cd mdglance
cargo run -- examples/render-kitchen-sink.md
```

Pass `--detach` to return the shell prompt immediately:

```sh
cargo run -- --detach README.md
```

With no path argument, `mdglance` reads a newline-separated file queue from standard input:

```sh
fd -e md -e svg | cargo run --
```

Use `[` and `]` to move through the queue.

PlantUML blocks require the `plantuml` executable to be available on `PATH`. Without it, those
blocks remain readable as source code. Mermaid is bundled and works offline.

## Keyboard shortcuts

| Keys | Action |
| --- | --- |
| `j` / `k` | Scroll down / up |
| `d` / `u` | Half-page down / up |
| `Space`, `g`, `G` | Page down, top, bottom |
| `/`, `n`, `N` | Search, next match, previous match |
| `t`, `Tab`, `Enter` | Toggle TOC, change focus, activate selection |
| `f` | Show link hints |
| `h` / `l` | Markdown history; pan left / right in SVG mode |
| `[` / `]` | Previous / next queued file |
| `+` / `-` / `0` | Zoom in / out / reset SVG view |
| `?`, `Esc`, `q` | Help, close overlay, quit |

Shortcuts can be replaced in the configuration file.

## Configuration

Configuration is optional. `mdglance` checks, in order:

1. `mdglance.toml` in the directory where it was launched
2. `~/.config/mdglance/config.toml`

A minimal example:

```toml
[toc]
visible_on_start = false
max_depth = 3

[window]
width = 1280
height = 900

[theme]
preset = "system"

[keybindings]
quit = ["q"]
```

Built-in theme presets are `system`, `light`, `dark`, `tokyo-night`, `gruvbox`,
`catppuccin-latte`, `catppuccin-mocha`, `solarized-light`, and `solarized-dark`. Semantic colors
and syntax themes can also be overridden independently for light and dark appearances; see the
configuration types and defaults in [`src/config.rs`](src/config.rs).

On macOS, option-click the green window button to use Zoom instead of borderless fullscreen.

## Security

`mdglance` renders Markdown in a native WebView. Only use it with documents you trust for now:

- raw HTML is not sanitized;
- Mermaid currently uses its `loose` security mode;
- PlantUML invokes a locally installed executable;
- external HTTP(S) links open in the default browser.

Hardening untrusted input is tracked in [`ROADMAP.md`](ROADMAP.md).

## Development

```sh
cargo fmt --check
cargo test
```

The larger files under [`examples/`](examples/) are rendering fixtures, not product documentation.
Mermaid and theme attribution is recorded in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

## License

Licensed under the [MIT License](LICENSE).
