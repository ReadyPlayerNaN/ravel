# Ravel

A simple, read-only structured document viewer built with Rust, GTK4 and Libadwaita.
Supports JSON, JSON Lines and YAML.

## Initial interface

- Open `.json`, `.jsonl`, `.yaml` and `.yml` documents from the native chooser
  or command line (extensions are case-insensitive).
- Explore nested objects and arrays in a full-width document view, without a
  separate navigation sidebar. The former sidebar search/type controls are removed.
- Read document statistics in the footer: value and type counts, object keys,
  maximum depth, JSON Lines record count, and YAML document count.
- Switch between **Structured** (native GTK nodes with animated, lazy expansion)
  and **Raw** (JSON preview) or **Raw YAML** (original YAML source).
  Structured is the default.
- Right-click any structured node to copy its JSON or JSON Pointer, or run common
  type-specific **jq** operations. Results open in a read-only, copyable dialog;
  the original document is never changed.
- Read syntax-highlighted JSON or YAML in Raw and copy it to the clipboard.
  GtkSourceView colors keys, strings, escapes, numbers, booleans, and null, with
  an Adwaita color scheme that follows the application's light/dark theme.
- Hover a key in the JSON preview to highlight same-name keys in the visible
  viewport. Hold Shift to match their structural path too, with array indices
  treated as wildcards. String values are never matched as keys.
- Collapse or expand any multiline JSON object or array with the preview's
  gutter arrows, including nested blocks and JSON Lines records.
- Get syntax errors with source locations; a failed open leaves the previous
  document intact. File loading is asynchronous and parsing runs off the UI thread.

Object member order is retained. JSON/JSONL numbers retain arbitrary precision;
YAML numeric limits are described below. Statistics
include the root and any synthetic document/record array; depth counts edges from the root
(a scalar or empty container has depth zero). Keys count object members, while
strings count string values only. Counts are independent of collapsed nodes.
This is not a text editor.

## Requirements

- Rust **1.92+**, Cargo, a C compiler and `pkg-config`.
- GTK **4.12+** development libraries.
- Libadwaita **1.6+** development libraries.
- GtkSourceView **5.4+** development libraries.
- `jq` on `PATH` for node operations and desktop integration tests; viewing works
  without it and operations report a clear error if it is missing.
- A graphical session (Wayland or X11) to run the application.

On a recent Fedora release:

```sh
sudo dnf install gcc pkgconf-pkg-config gtk4-devel libadwaita-devel gtksourceview5-devel jq
```

Debian/Ubuntu package names are `build-essential`, `pkg-config`, `libgtk-4-dev`
`libadwaita-1-dev`, `libgtksourceview-5-dev` and `jq`; check the versions above, since
older releases such as Ubuntu 24.04 ship an insufficient Libadwaita version.

## Run

```sh
cargo run --locked
cargo run --locked -- examples/sample.json
cargo run --locked -- examples/events.jsonl
cargo run --locked -- examples/sample.yaml
cargo run --locked -- examples/resources.yml
```

Multiple command-line files open in separate windows. Opening from a window's
chooser replaces its current document only after a successful parse.

| Shortcut | Action |
| --- | --- |
| Ctrl+O | Open document |
| Ctrl+W | Close window |
| Ctrl+Q | Quit |

## Structured nodes and jq

Expand a container by clicking its row; children appear with a 250 ms slide
animation and are built only on first expansion. Right-click its row (or a scalar
row) for a custom node menu. Object operations include keys, values, length and
entries; arrays offer sorting, uniqueness, reversal, flattening and aggregation;
strings offer case conversion and numeric conversion. Every type offers `type`.
Each filter runs against that node alone, using jq stdin rather than a shell.
The result appears in a separate **jq: Operation** dialog with a Copy button;
these actions do not edit, reorder or replace the document. For example, **Length**
counts array items, object members or string characters; **Keys** returns object
key names; **Values** returns an array of member values; **To Entries** returns
`{key, value}` pairs. **Sort**, **Unique**, **Reverse** and **Flatten** return
transformed arrays; **Add** sums numbers or combines compatible values; **First**,
**Last**, **Min** and **Max** select values. **Join** combines array values with
`, `; case conversion is ASCII-only; **Type** returns a type name such as `"object"`.
Structured keys and scalar values use the same JSON syntax colors and light/dark
schemes as Raw, including highlighting of string escapes.
Some operations are invalid for particular values (for example `tonumber` on
non-numeric text); jq's error is shown without changing the document.

## Keyboard navigation (Structured)

Click a node row or Tab into the tree to focus it. Keys move focus without changing
JSON or running jq:

| Key | Action |
| --- | --- |
| Right | Expand if necessary and focus the first child; no action on a leaf |
| Left | Collapse an expanded container; otherwise focus its parent |
| Up / Down | Previous / next visible row, skipping collapsed descendants |
| Home / End | First / last visible row |
| Enter / Space | Toggle the focused container |
| Menu / Shift+F10 | Open that node's Copy/jq context menu |
| Tab / Shift+Tab | Leave/re-enter the tree using GTK focus traversal |

Focus has a native row indicator; keyboard navigation scrolls the focused row
into view. Mouse collapse returns focus from a hidden descendant to its parent.
Nested expansion states survive parent collapse. Scalars and empty containers
are focusable leaf rows. Modified arrows are left to GTK/application shortcuts.

The map is based on the [W3C tree-view pattern](https://www.w3.org/WAI/ARIA/apg/patterns/treeview/#keyboardinteraction),
with one intentional difference: classic Right first expands a closed node and
only enters on the next press; Ravel combines both steps as requested. This is
focus navigation, not multi-selection; type-ahead and expand-all are not implemented.

## Key hover (Raw JSON)

Try `cargo run --locked -- examples/hover.json`. Hovering a `name` key highlights
all visible `name` keys. Shift-hovering `/users/0/name` matches `/users/1/name`
but excludes `/metadata/name`. Only actual array indices are wildcarded; numeric
object member names remain literal. Names match exactly, including case.

Pressing or releasing either Shift key updates the highlight without requiring
mouse movement. Scrolling updates the visible matches under the pointer. Leaving
a key or opening another document clears the highlight. The interaction is
local to each JSON preview; it does not modify text or highlight structured rows.

## Code folding (Raw JSON)

Click **▼** beside an object or array's opening line to collapse its body; click
**▶** to expand it again. Opening and closing lines remain visible, with their
original indentation. Empty inline containers and scalars have nothing to fold.

Nested blocks fold independently. Expanding a parent preserves any previously
collapsed children. Folds reset when the preview content is replaced, such as
when opening a document.

Folding only hides text: the document is unchanged. The Copy JSON button and
selection copying (Ctrl+C) include folded content. Hover matching excludes keys
inside collapsed blocks.

## JSON Lines conventions

The `.jsonl` extension is detected case-insensitively. Each nonblank line must
contain exactly one JSON value; scalar records are supported. Blank lines are
skipped and an empty file is valid. Errors report the physical, one-based source
line. A leading UTF-8 BOM is accepted for all supported formats.

JSON Lines is displayed as a synthetic array. `/0/message`, for example, refers
to the first nonblank record's `message`, not to a physical line number. JSON
Pointers escape `/` as `~1` and `~` as `~0`; the root pointer is the empty string.

## YAML conventions

YAML is parsed into the same JSON-compatible value model. Mappings, sequences,
scalar roots, block strings, anchors, aliases and merge keys are supported.
Only `true`/`false` are inferred as booleans; `NO`, `yes`, `on` and plain dates
remain strings. Explicit quoted strings stay strings. Mapping order is retained;
merge keys are resolved rather than displayed as literal `<<` members.

A single YAML document is displayed directly. A stream with multiple documents
is a synthetic array: `/0/metadata/name` addresses the first document's name.
Empty documents and explicit null documents retain their slots. An empty or
comment-only file displays null and reports zero source documents.

**Raw YAML** shows the original source with YAML syntax highlighting, preserving
comments, formatting, anchor names, merge expressions and document separators
(a leading UTF-8 BOM is stripped). The header Copy button copies YAML source;
selection copying in Raw copies the selected source text.

Structured view, its per-node Copy JSON menus, JSON Pointers, statistics and jq
operate on normalized values. Structured colors, animated expansion and keyboard
navigation remain available. JSON key-hover matching and delimiter gutter folding
are disabled in Raw YAML; they would interpret YAML strings and flow syntax
incorrectly. Parsed values do not retain comment or tag metadata, but original
source does.

Non-string mapping keys (including numeric keys), duplicate keys, custom or
unsupported tags, recursive aliases and non-finite numbers are rejected with an
error, rather than silently changed. Quoted numeric keys are supported. YAML
integers must fit signed/unsigned 64-bit ranges; larger integers are rejected
rather than rounded (quote them to retain the digits as strings). YAML floating
point values use finite IEEE-754 double precision. Parser budgets bound nesting,
alias expansion, node count and document count. No `!include` or external resource
loading is enabled. A failed parse leaves the previous document unchanged.

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

The ignored GTK smoke test exercises real widgets. Install `xdotool` alongside
Xvfb to also run native X11 arrow/Tab keystroke checks (CI installs both).
With Xvfb installed:

```sh
GDK_BACKEND=x11 GSK_RENDERER=cairo GTK_A11Y=none GIO_USE_VFS=local \
  xvfb-run -a cargo test -p ravel-desktop --locked -- --ignored
```

The workspace separates `ravel-core` (document behavior) from `ravel-desktop`
(native interface). Boardo's three Rust skills and their references are copied
into `.agents/skills`, exposed through relative `.pi/skills` symlinks, and
explained in `AGENTS.md`.

## Current scope

Documents and their structural index are held in memory. Pretty-printing
currently runs on the UI thread; this first version
is for small to medium documents, not multi-gigabyte log streaming. JSON whitespace
is not retained; YAML source text is retained for Raw YAML. Editing, JSONPath, pagination, packaging and other
text formats are not implemented yet.

`data/io.github.ravel.Ravel.desktop` is a starter desktop entry for an installed
`ravel` binary. The application ID is provisional and should be changed to an
owned reverse-DNS namespace before distribution.
