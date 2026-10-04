# Ravel

A simple, read-only text document viewer built with Rust, GTK4 and Libadwaita.
The initial version focuses on JSON and JSON Lines.

## Initial interface

- Open `.json` and `.jsonl` documents from the native chooser or command line.
- Expand nested objects and arrays in a structural tree.
- Search keys, JSON Pointer paths, and complete scalar values (case-insensitive).
- Combine search with a value-type filter: object, array, string, number,
  boolean, or null. Ancestors remain visible to provide context.
- Read syntax-highlighted JSON for the selected value and copy it to the clipboard.
  GtkSourceView colors keys, strings, escapes, numbers, booleans, and null, with
  an Adwaita color scheme that follows the application's light/dark theme.
- Hover a key in the JSON preview to highlight same-name keys in the visible
  viewport. Hold Shift to match their structural path too, with array indices
  treated as wildcards. String values are never matched as keys.
- Collapse or expand any multiline JSON object or array with the preview's
  gutter arrows, including nested blocks and JSON Lines records.
- Get syntax errors with source locations; a failed open leaves the previous
  document intact. File loading is asynchronous and parsing runs off the UI thread.

Object member order and arbitrary-precision numbers are retained. Search prunes
unrelated descendants, even if a container itself matches. This is structural
search, not JSONPath, regex search, or a text editor.

## Requirements

- Rust **1.92+**, Cargo, a C compiler and `pkg-config`.
- GTK **4.12+** development libraries.
- Libadwaita **1.6+** development libraries.
- GtkSourceView **5.4+** development libraries.
- A graphical session (Wayland or X11) to run the application.

On a recent Fedora release:

```sh
sudo dnf install gcc pkgconf-pkg-config gtk4-devel libadwaita-devel gtksourceview5-devel
```

Debian/Ubuntu package names are `build-essential`, `pkg-config`, `libgtk-4-dev`
`libadwaita-1-dev` and `libgtksourceview-5-dev`; check the versions above, since
older releases such as Ubuntu 24.04 ship an insufficient Libadwaita version.

## Run

```sh
cargo run --locked
cargo run --locked -- examples/sample.json
cargo run --locked -- examples/events.jsonl
```

Multiple command-line files open in separate windows. Opening from a window's
chooser replaces its current document only after a successful parse.

| Shortcut | Action |
| --- | --- |
| Ctrl+O | Open document |
| Ctrl+F | Focus structural search |
| Ctrl+W | Close window |
| Ctrl+Q | Quit |

## Key hover

Try `cargo run --locked -- examples/hover.json`. Hovering a `name` key highlights
all visible `name` keys. Shift-hovering `/users/0/name` matches `/users/1/name`
but excludes `/metadata/name`. Only actual array indices are wildcarded; numeric
object member names remain literal. Names match exactly, including case.

Pressing or releasing either Shift key updates the highlight without requiring
mouse movement. Scrolling updates the visible matches under the pointer. Leaving
a key or changing the selected value clears the highlight. The interaction is
local to each JSON preview; it does not modify text or highlight the tree sidebar.

## Code folding

Click **▼** beside an object or array's opening line to collapse its body; click
**▶** to expand it again. Opening and closing lines remain visible, with their
original indentation. Empty inline containers and scalars have nothing to fold.

Nested blocks fold independently. Expanding a parent preserves any previously
collapsed children. Folds reset when the preview content is replaced, such as
when selecting another value or opening a document. This does not change the
sidebar tree's expansion state.

Folding only hides text: the document is unchanged. The Copy JSON button and
selection copying (Ctrl+C) include folded content. Hover matching excludes keys
inside collapsed blocks.

## JSON Lines conventions

The `.jsonl` extension is detected case-insensitively. Each nonblank line must
contain exactly one JSON value; scalar records are supported. Blank lines are
skipped and an empty file is valid. Errors report the physical, one-based source
line. A leading UTF-8 BOM is accepted for either format.

JSON Lines is displayed as a synthetic array. `/0/message`, for example, refers
to the first nonblank record's `message`, not to a physical line number. JSON
Pointers escape `/` as `~1` and `~` as `~0`; the root pointer is the empty string.

## Development

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

The core can be tested without GTK development libraries:

```sh
cargo test -p ravel-core --locked
```

The ignored GTK smoke test exercises real widgets. With Xvfb installed:

```sh
GDK_BACKEND=x11 GSK_RENDERER=cairo GTK_A11Y=none GIO_USE_VFS=local \
  xvfb-run -a cargo test -p ravel-desktop --locked -- --ignored
```

The workspace separates `ravel-core` (document behavior) from `ravel-desktop`
(native interface). Boardo's three Rust skills and their references are copied
into `.agents/skills`, exposed through relative `.pi/skills` symlinks, and
explained in `AGENTS.md`.

## Current scope

Documents and their structural index are held in memory. Filtering and
pretty-printing selections currently run on the UI thread; this first version
is for small to medium documents, not multi-gigabyte log streaming. Original
whitespace is not retained. Editing, JSONPath, pagination, packaging and other
text formats are not implemented yet.

`data/io.github.ravel.Ravel.desktop` is a starter desktop entry for an installed
`ravel` binary. The application ID is provisional and should be changed to an
owned reverse-DNS namespace before distribution.
