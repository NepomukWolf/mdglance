# mdglance

`mdglance` is a small, keyboard-first Markdown and SVG previewer for terminal workflows.

It is intended for people who write in terminal editors and want to check the rendered result
without opening a full editor, browser workspace, or project-oriented Markdown application. Point
it at a file, keep writing, and glance at the native preview whenever you save.

The viewer is deliberately read-only and designed to stay out of the writing loop. Live reload,
navigation, search, the table of contents, link following, and SVG controls are all available from
the keyboard, so previewing does not require switching tools or reaching for the mouse.

> [!NOTE]
> This is an early prototype developed and tested primarily on macOS.

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

Mermaid is bundled and works offline. PlantUML rendering is disabled by default because it invokes
a local executable. See the [configuration reference](docs/configuration.md#diagrams) to enable it
for trusted workspaces.

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
| `T` | Open workspace trust controls while restricted |
| `?`, `Esc`, `q` | Help, close overlay, quit |

Shortcuts can be replaced in the configuration file.

## Configuration

Configuration is optional. It can control the window, table of contents, themes, diagram rendering,
and every keyboard binding. `mdglance` checks, in order:

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

Project configuration is used only after its workspace has been trusted. For every option, accepted
value, default keybinding, and theme name, see the [complete configuration
reference](docs/configuration.md). A commented [example configuration](mdglance.example.toml) is
available as a copyable starting point.

On macOS, option-click the green window button to use Zoom instead of borderless fullscreen.

## Security

Unknown workspaces open with a trust prompt and can be viewed in restricted mode. Restricted mode
keeps ordinary Markdown viewing available while disabling active content, unsolicited network
requests, out-of-workspace image access, local process execution, and project configuration.

See [Security and workspace trust](docs/security.md) for the precise boundary, trust storage,
revocation instructions, and guidance on when to trust a folder.

## Development

```sh
cargo fmt --check
cargo test
```

The larger files under [`examples/`](examples/) are rendering fixtures, not product documentation.
Mermaid and theme attribution is recorded in [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

## License

Licensed under the [MIT License](LICENSE).
