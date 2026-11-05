# Security Sanitization Check

This file exercises the trusted-mode HTML sanitizer (`ammonia`) and the trusted-mode
Content-Security-Policy added in `src/render.rs`. It must be opened as a **trusted**
workspace to actually test anything here — in restricted mode all raw HTML is already
escaped to plain text regardless of this file's content, so that path isn't exercised.

```sh
cargo run -- examples/security-sanitization-check.md
```

Click **Trust Folder** on the prompt, then check this document for two things:

1. None of the "Attack attempts" below should fire — no alert dialogs should ever appear
   while viewing this file normally (a couple require a manual click, noted below; even
   those should do nothing).
2. Everything in "Should still work" should render normally — these exercise legitimate
   raw-HTML formatting that the sanitizer must *not* break.

Every payload below is fully self-contained (no network calls) and uses `alert(...)` as
the "it executed" signal, since a real dialog box is impossible to miss if a sanitizer or
CSP regression lets something through.

## Attack attempts (should all be inert)

Script tag:

<script>alert('UNSAFE: <script> executed')</script>

`onerror` event handler on an image:

<img src="x" onerror="alert('UNSAFE: onerror executed')">

`onload` event handler on an inline SVG:

<svg onload="alert('UNSAFE: svg onload executed')"></svg>

`onmouseover` event handler (hover over the text below — should do nothing):

<div onmouseover="alert('UNSAFE: onmouseover executed')">hover me</div>

An iframe (should not even appear — `object-src`/`frame-src` are blocked, and `<iframe>`
isn't in the sanitizer's allowed tag list):

<iframe srcdoc="&lt;script&gt;alert('UNSAFE: iframe script executed')&lt;/script&gt;"></iframe>

A `javascript:` link — requires a manual click; clicking it should do nothing at all (no
navigation, no dialog):

<a href="javascript:alert('UNSAFE: javascript: link executed')">do not click-test link</a>

A `<form>` pointed nowhere useful (should not be able to submit anywhere — `form-action`
is blocked):

<form action="https://example.com/exfiltrate"><input type="submit" value="should be inert"></form>

## Should still work

A `<details>`/`<summary>` disclosure:

<details><summary>Click to expand</summary>This content should be revealed normally.</details>

A raw HTML table:

<table><thead><tr><th>Column A</th><th>Column B</th></tr></thead><tbody><tr><td>1</td><td>2</td></tr></tbody></table>

Basic inline formatting:

<p>This paragraph has <b>bold</b>, <i>italic</i>, and <code>inline code</code> written as raw HTML.</p>

A blockquote:

<blockquote>This should render as a normal blockquote.</blockquote>
