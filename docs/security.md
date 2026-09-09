# Security and workspace trust

`mdglance` renders documents in a native WebView. Markdown can contain active HTML, diagrams can
execute JavaScript or a local renderer, and images can access the network or local files. Workspace
trust lets you inspect unfamiliar documents without enabling those capabilities immediately.

Restricted mode reduces exposure, but it is not an operating-system sandbox. Keep `mdglance` and
the system WebView up to date, especially when viewing content from unknown sources.

## Workspace boundary

For an opened file, `mdglance` walks upward to the nearest directory containing a `.git` directory
or file. That directory is the workspace. If no Git marker is found, the file's immediate parent is
used.

Paths are canonicalized before the workspace and trust record are evaluated. This prevents a
symlink spelling from implicitly inheriting trust for a different target. Navigating to a Markdown
file or queued file in another workspace evaluates that workspace independently and may show a new
prompt.

## The trust prompt

An unknown workspace presents three choices:

- **Trust Folder** permanently enables trusted mode for that workspace.
- **Open Restricted** continues safely for this process only. The prompt returns on the next launch.
- **Close** exits without opening the workspace in trusted mode.

The prompt supports the mouse as well as Tab, Shift+Tab, arrow keys, Enter, Space, and Escape. In
restricted mode, select the persistent **Restricted — folder not trusted** control or press
`Shift+T` to reopen the trust controls.

Trust a folder only when you trust its contents and the people or tools that can modify them. A
trusted Git checkout can become unsafe later if unreviewed changes are introduced.

## Restricted mode

Restricted mode keeps the ordinary viewer workflow available:

- Markdown formatting and Rust-side syntax highlighting
- table of contents, search, scrolling, and link hints
- Markdown history and file queues
- live reload
- explicitly activated HTTP(S) links, opened in the system browser
- raster images whose canonical paths remain inside the workspace

It restricts capabilities that can execute content or access resources unexpectedly:

- raw and inline HTML are displayed as text;
- direct SVG documents are displayed as source;
- embedded SVG, data-URL, remote, and out-of-workspace images are blocked;
- Mermaid and PlantUML blocks remain code and the Mermaid runtime is not loaded;
- the local `plantuml` process is never invoked;
- project `mdglance.toml` files are ignored; and
- a restrictive Content Security Policy limits scripts and resource loading.

Unsupported navigation schemes such as `javascript:` and `file:` are not allowed to leave the
document. HTTP(S) URLs are accepted only through an explicit link activation.

## Trusted mode

Trusted mode enables raw HTML, direct and embedded SVG, Mermaid, remote images, data-URL images, and
eligible project configuration. These features should be treated with the same care as opening the
content in a browser.

PlantUML remains disabled unless `[diagrams].plantuml` is explicitly enabled in configuration and
the `plantuml` executable is available on `PATH`. Trust by itself never opts into local process
execution.

## Trust storage and revocation

Trust records are individual TOML files under the platform application-data directory:

- macOS: `~/Library/Application Support/mdglance/workspace-trust`
- Linux: `$XDG_DATA_HOME/mdglance/workspace-trust`, usually
  `~/.local/share/mdglance/workspace-trust`
- Windows: `%LOCALAPPDATA%\mdglance\workspace-trust`

Each filename is the SHA-256 hash of the canonical workspace path. The record itself contains the
path so it can be identified manually. Records contain no secret material.

To revoke trust, close `mdglance`, inspect the `path` values in that directory, and delete the TOML
record matching the workspace. The next time a file from that workspace is opened, `mdglance` asks
again. Deleting the entire `workspace-trust` directory revokes every saved decision.
