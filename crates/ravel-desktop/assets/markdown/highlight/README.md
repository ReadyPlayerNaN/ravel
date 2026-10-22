# Vendored Highlight.js

Highlight.js **11.11.1**, BSD-3-Clause licensed (see `LICENSE`). The common
browser build and license are copied unchanged from the official CDN release
repository (this is a build artifact, not a runtime CDN dependency):

- Repository: https://github.com/highlightjs/cdn-release
- Tag: `11.11.1`
- Commit: `91724c0adaf7bea7e5c5c85e4ea1d672f6c0ed23`
- Runtime path upstream: `build/highlight.min.js`
- License path upstream: `LICENSE`

SHA-256:

```text
c4a399dd6f488bc97a3546e3476747b3e714c99c57b9473154c6fb8d259b9381  highlight.min.js
5f289f36595e0ef6c53d9f4b4e51d7cc1efc5e2b3ba6130a875d177c54789eaf  LICENSE
```

Bundled grammars: Bash, C, C++, C#, CSS, Diff, Go, GraphQL, INI, Java,
JavaScript, JSON, Kotlin, Less, Lua, Makefile, Markdown, Objective-C, Perl,
PHP, PHP templates, Plaintext, Python, Python REPL, R, Ruby, Rust, SCSS,
Shell sessions, SQL, Swift, TypeScript, VB.NET, WebAssembly and XML/HTML, YAML.
Upstream aliases (for example `js`, `ts`, `sh`, `yml`, `html`) are supported.

Ravel invokes only explicit-language highlighting: no automatic language
inference, dynamic grammar downloads, or code execution. `reader.js` caps work
at 16 KiB per block, 128 KiB per document and 128 blocks. Untagged, unknown,
oversized or failed blocks remain literal. Token colors in `reader.css` follow
the application's light/dark theme; inline code is not highlighted.

To update, select an official release, pin its commit, fetch these two files,
review upstream security/release notes, verify and record their checksums, and
retain the license in both source and binary packaging. Run the core Markdown
and GTK/WebKit tests (escaping, token colors, aliases, unchanged text/copying,
budgets and CSP). No npm/Node runtime or build-time download is required.
