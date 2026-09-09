# Configuration reference

`mdglance` works without configuration. A configuration file can change window behavior, the table
of contents, themes, diagram rendering, and keyboard shortcuts.

## File lookup and trust

`mdglance` uses the first existing configuration file in this order:

1. `mdglance.toml` in the directory where `mdglance` was launched
2. `$XDG_CONFIG_HOME/mdglance/config.toml`, when `XDG_CONFIG_HOME` is set
3. `~/.config/mdglance/config.toml`

The files are alternatives; their values are not merged. A project `mdglance.toml` is loaded only
when it is inside the current workspace and that workspace is trusted. The global configuration is
user-controlled and remains available in restricted mode.

Unknown options, invalid values, and conflicting keybindings produce an error instead of being
silently ignored. See [Security and workspace trust](security.md) for the trust boundary.

For a copyable file containing every section, see
[`mdglance.example.toml`](../mdglance.example.toml).

## Window

```toml
[window]
width = 1080
height = 860
maximized = false
fullscreen = false
```

`width` and `height` are the initial logical window dimensions. `maximized` and `fullscreen` cannot
both be `true`. On macOS, fullscreen uses the borderless native presentation; option-click the green
window button to use Zoom instead.

## Table of contents

```toml
[toc]
visible_on_start = false
max_depth = 3
```

`max_depth` controls which heading levels appear in the table of contents. Values below `1` are
treated as `1`; Markdown headings are available through level `6`.

## Diagrams

```toml
[diagrams]
plantuml = false
```

Mermaid support is bundled and requires no option. PlantUML is opt-in because rendering invokes the
`plantuml` executable installed on the system. A PlantUML block is rendered only when:

- `plantuml = true`;
- the current workspace is trusted; and
- `plantuml` is available on `PATH`.

Otherwise, `plantuml` and `puml` fences remain readable as code.

## Themes

Select a bundled or user-installed Alacritty-compatible palette:

```toml
[theme]
name = "system"
```

`system` is the default when `[theme]` or `theme.name` is omitted. It follows the operating-system
appearance automatically: light appearance uses the packaged `light.toml`, while dark appearance
uses the packaged `dark.toml`. These two default palettes are tuned for a GitHub-like Markdown
presentation: headings use the normal text color, links and interface accents use blue, and search
matches and errors retain distinct yellow and red colors.

Bundled themes are:

- `system`
- `light`
- `dark`
- `tokyo-night`
- `gruvbox`
- `catppuccin-latte`
- `catppuccin-mocha`
- `solarized-light`
- `solarized-dark`

Other themes infer light or dark appearance from their background. The legacy `preset` key remains
an alias for `name`; specifying both is an error.

Install additional `.toml` palettes in `$XDG_CONFIG_HOME/mdglance/themes`, falling back to
`~/.config/mdglance/themes`. Discovery is non-recursive and refreshes whenever the picker opens.
The filename stem is used verbatim as its name, and a user file overrides a bundled file with the
same name. A palette must contain Alacritty's `[colors.primary]`, `[colors.normal]`, and
`[colors.bright]` tables using `#RRGGBB`; unrelated Alacritty sections are tolerated. This makes it
possible to download an Alacritty palette from a collection such as TerminalColors and copy it
directly into the directory. Invalid files appear disabled in the picker. Explicitly configuring a
missing or invalid theme is a startup error.

Press `p` (`open_theme_picker`) to open the picker. Use `j`/`k`, arrow keys, Home/End, Tab, or the
mouse; Enter or Apply commits the preview, while Escape or Cancel restores the previous theme.
Selection is session-only and remains active across reloads, trust changes, and navigation.

### Variant colors

Any semantic color can be overridden independently under `[theme.light]` and `[theme.dark]`:

```toml
[theme.dark]
background = "#0d1117"
surface = "#161b22"
text = "#f0f6fc"
muted_text = "#9198a1"
heading = "#f0f6fc"
link = "#4493f8"
border = "#3d444d"
divider = "#3d444d"
code_background = "#161b22"
sidebar_background = "#151b23"
accent = "#4493f8"
search_match = "#d29922"
error = "#f85149"
```

Every color must use six-digit `#RRGGBB` notation. Unspecified colors continue to come from the
selected theme.

### Syntax themes

Unless overridden, the selected Alacritty palette controls both the application/Markdown colors
and syntax highlighting. mdglance generates a Syntect syntax theme from the same palette: primary
foreground is used for ordinary code, while bright black, green, yellow, magenta, blue, and cyan
are mapped to comments, strings, numbers, keywords, functions, types, and operators; normal red is
used for invalid syntax. Under `system`, separate syntax themes are generated from `light.toml` and
`dark.toml` and switch with the operating-system appearance.

To replace that generated syntax theme without changing the Markdown/application palette, choose a
bundled Syntect theme per appearance variant:

```toml
[theme.light]
syntax_theme = "inspired-github"

[theme.dark]
syntax_theme = "base16-ocean-dark"
```

Available syntax themes are:

- `inspired-github`
- `solarized-light`
- `solarized-dark`
- `base16-eighties-dark`
- `base16-mocha-dark`
- `base16-ocean-dark`
- `base16-ocean-light`
- `tokyo-night`
- `gruvbox`
- `catppuccin-latte`
- `catppuccin-mocha`

Any valid catalog theme name can also be used as `syntax_theme`; its ANSI colors generate the
syntax palette.

An external TextMate `.tmTheme` file can be used instead:

```toml
[theme.dark]
syntax_theme_file = "themes/my-theme.tmTheme"
```

Relative paths are resolved from the configuration file. Absolute paths and `~/...` are supported.
Only one of `syntax_theme` and `syntax_theme_file` may be set for a variant.

## Keybindings

Each entry maps an action name to a list of shortcuts. Setting an empty list disables that action's
default shortcuts.

```toml
[keybindings]
quit = ["q", "Cmd+W"]
open_search = ["/"]
toggle_toc = ["t"]
```

Modifiers are joined with `+`. Accepted aliases are `Cmd`, `Command`, `Meta`, or `Super`; `Ctrl` or
`Control`; `Alt` or `Option`; and `Shift`. Named keys include `Space`, `Escape`, `Enter`, `Tab`,
`Backspace`, `Delete`, `Home`, `End`, `PageUp`, `PageDown`, and the four arrow keys. `Esc`, `Return`,
`Up`, `Down`, `Left`, and `Right` are accepted aliases. Printable single-character keys are also
supported; an uppercase letter represents its shifted form.

Shortcuts cannot be assigned to two actions that are active in the same UI context. The complete
action list and defaults are:

| Action | Default shortcut(s) |
| --- | --- |
| `scroll_down` | `j` |
| `scroll_up` | `k` |
| `scroll_left` | `h` |
| `scroll_right` | `l` |
| `half_page_down` | `d` |
| `half_page_up` | `u` |
| `page_down` | `Space` |
| `top` | `g` |
| `bottom` | `Shift+G` |
| `open_search` | `/` |
| `accept_search` | `Enter` |
| `next_search_hit` | `n` |
| `previous_search_hit` | `Shift+N` |
| `show_help` | `?` |
| `close_overlay` | `Escape` |
| `toggle_toc` | `t` |
| `toggle_focus` | `Tab` |
| `back` | `h` |
| `forward` | `l` |
| `previous_file` | `[` |
| `next_file` | `]` |
| `open_link_hints` | `f` |
| `toc_down` | `j` |
| `toc_up` | `k` |
| `activate_selection` | `Enter` |
| `zoom_in` | `=`, `Shift+=` |
| `zoom_out` | `-` |
| `reset_view` | `0` |
| `manage_trust` | `Shift+T` |
| `open_theme_picker` | `p` |
| `quit` | `q`, plus `Cmd+W` and `Cmd+Q` on macOS |

Press `?` inside `mdglance` to see the effective bindings.
