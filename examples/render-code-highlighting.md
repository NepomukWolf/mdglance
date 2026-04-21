# mdglance Code Highlighting Sample

This file is meant to exercise syntax highlighting for fenced code blocks with explicit language tags.

```rust
use std::collections::BTreeMap;

fn group_words(input: &[&str]) -> BTreeMap<char, Vec<&str>> {
    let mut groups = BTreeMap::new();
    for word in input {
        let key = word.chars().next().unwrap_or('_');
        groups.entry(key).or_insert_with(Vec::new).push(*word);
    }
    groups
}
```

```json
{
  "viewer": "mdglance",
  "features": {
    "watch": true,
    "toc": true,
    "syntax_highlighting": true
  },
  "languages": ["rust", "json", "python", "toml", "bash", "cpp", "yaml"]
}
```

```python
from pathlib import Path


def collect_markdown_files(root: Path) -> list[Path]:
    return sorted(path for path in root.rglob("*.md") if path.is_file())


if __name__ == "__main__":
    for markdown_file in collect_markdown_files(Path(".")):
        print(markdown_file)
```

```toml
[window]
width = 1200
height = 860
fullscreen = false

[toc]
visible_on_start = true
max_depth = 4
```

```bash
set -euo pipefail

cargo fmt
cargo check
cargo run -- examples/render-code-highlighting.md
```

```cpp
#include <iostream>
#include <string>
#include <vector>

int main() {
    std::vector<std::string> items{"markdown", "preview", "native"};
    for (const auto& item : items) {
        std::cout << item << std::endl;
    }
    return 0;
}
```

```yaml
name: preview-check
on:
  push:
    branches: [main]
jobs:
  check:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo check
```

```diff
- old highlight behavior
+ new syntect-based highlighting
```
