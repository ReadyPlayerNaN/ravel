# Ravel

A simple, read-only structured document viewer built with Rust, GTK4 and Libadwaita.
Supports JSON, JSON Lines, YAML and formatted Markdown.

## Initial interface

- Open `.json`, `.jsonl`, `.yaml`, `.yml`, `.md` and `.markdown` documents from the native chooser,
  the searchable **Open ▾** recent-documents menu, or command line (extensions are case-insensitive).
- Explore nested objects and arrays in a full-width document view, without a
  separate navigation sidebar. The former sidebar search/type controls are removed.
- Read document statistics in the footer: value and type counts, object keys,
  maximum depth, JSON Lines record count, and YAML document count.
- Switch between **Structured** (native GTK nodes with animated, lazy expansion)
  and **Raw** (JSON preview) or **Raw YAML** (original YAML source).
  Structured is the default for JSON and YAML.
- Read Markdown in **Formatted** by default, with headings, emphasis, lists,
  quotes, code, tables, clickable links, a heading index, images, audio/video,
  footnotes and math, or switch to **Raw Markdown** for the original source.
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
- WebKitGTK **6.0 API / 2.40+** development libraries for rich Markdown.
- GStreamer base/good plugins for audio/video (including Ogg/Vorbis and WebM/VP8
  used by the examples and tests). Other formats require their system codecs.
- `jq` on `PATH` for node operations and desktop integration tests; viewing works
  without it and operations report a clear error if it is missing.
- A graphical session (Wayland or X11) to run the application.

On a recent Fedora release:

```sh
sudo dnf install gcc pkgconf-pkg-config gtk4-devel libadwaita-devel gtksourceview5-devel webkitgtk6.0-devel gstreamer1-plugins-base gstreamer1-plugins-good jq
```

Debian/Ubuntu package names are `build-essential`, `pkg-config`, `libgtk-4-dev`
`libadwaita-1-dev`, `libgtksourceview-5-dev`, `libwebkitgtk-6.0-dev`,
`gstreamer1.0-plugins-base`, `gstreamer1.0-plugins-good` and `jq`; check the versions above, since
older releases such as Ubuntu 24.04 ship an insufficient Libadwaita version.

## Arch Linux package

- [`ravel-git`](packaging/aur/README.md) builds the latest `master` from source.
- [`ravel-bin`](packaging/aur-bin/README.md) installs prebuilt x86-64 binaries from
  tagged GitHub releases, without requiring Rust or Cargo.

Both install the `ravel` executable and desktop entry; choose one, since they
conflict. See the package documentation for publishing setup and local builds.

## Run

```sh
cargo run --locked
cargo run --locked -- examples/sample.json
cargo run --locked -- examples/events.jsonl
cargo run --locked -- examples/sample.yaml
cargo run --locked -- examples/resources.yml
cargo run --locked -- examples/sample.md
```

Multiple command-line files open in separate windows. Opening from a window's
chooser or recent-documents menu replaces its current document only after a successful parse.

The **Open ▾** menu shows recent local documents, newest first, with filenames and
parent directories. Search matches filenames and paths case-insensitively. Enter
opens the first match; Down enters the list, Up from its first row returns to
search, and Escape closes the menu. The folder button (or Ctrl+O) opens the native
file chooser. **×** forgets an entry without deleting or changing the document.
Missing files report the normal open error and can be removed from the list.

Recent-document history is application **state**, stored in
`$XDG_STATE_HOME/global.zak.ravel/recent-files.json`, falling back to
`~/.local/state/global.zak.ravel/recent-files.json` when `XDG_STATE_HOME` is unset,
empty, or relative. It is not written into XDG config or cache directories.
Only successfully opened local file URIs are remembered (at most 100); no document
contents or remote URLs are stored. State writes run off the UI thread and use
atomic replacement with private file permissions. Storage errors are logged and
do not prevent viewing. Delete this file while Ravel is closed to clear history.

| Shortcut | Action |
| --- | --- |
| Ctrl+O | Open document |
| Ctrl+W | Close window |
| Ctrl+Q | Quit |

## Structured nodes and jq

Documents open with the root expanded, showing its immediate fields or array
items. This also applies to JSON Lines records and YAML stream documents. Deeper
containers start collapsed; click a row to expand it with a 250 ms slide
animation. Children of those nested containers are built only on first expansion. Right-click its row (or a scalar
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

## Markdown conventions

`.md` and `.markdown` files (case-insensitive) open as formatted, selectable
read-only documents in a restricted WebKitGTK pane. CommonMark formatting,
strikethrough and tables (including column alignment) are supported. Code blocks
preserve whitespace but are not syntax-highlighted in Formatted. Styling follows
the application's light/dark theme.

### Links, headings and footnotes

Normal, reference-style and autolink hyperlinks are clickable and keyboard
focusable. Bare `http://`, `https://` and `www.` URLs, `mailto:user@example.org`
URIs and email addresses such as `user@example.org` in prose also become links
(`www.` uses HTTPS). Mailto query parameters such as `?subject=Hello%20there`
are preserved. Code, literal HTML and existing link labels are left alone.
HTTP(S) and email links open in the system browser/mail application;
local file links use an application chooser rather than silently launching files.
Relative destinations resolve against the Markdown file, not the working directory.
Active or unsupported schemes such as `javascript:` are not clickable.

Headings have automatic lowercase anchors (punctuation removed, spaces replaced
with hyphens; repeated names get `-1`, `-2`, etc.). Unicode headings are supported.
Use `[Section](#section)` for internal navigation, or `## Title {#custom}` for an
explicit ID. **Contents** links to every heading: a sticky left sidebar in wide
viewports (1000 CSS pixels or more), or a collapsed, keyboard-operable disclosure
above the text in smaller viewports. Resizing into the smaller layout collapses
it again. Navigation moves keyboard focus as well as scrolling. Footnotes use `[^name]` references and
`[^name]: ...` definitions, with formatted, multi-paragraph notes collected at the
end and a separate return link for each reference.

### Code blocks

Fenced code blocks with a language after the opening backticks have syntax colors:

````markdown
```javascript
const greeting = "Hello, Markdown!";
console.log(greeting);
```
````

Valid `json` fences open in the **same native structured tree** used for JSON
files, with the first level expanded. The block header's **Raw / Structured**
button switches between that tree and the original, syntax-highlighted JSON;
expanded branches are preserved while toggling. Each block has its own node copy,
JSON Pointer and jq actions, scoped to that block rather than the Markdown file.
The tree grows and shrinks with expanded branches, up to a height of 640 CSS
pixels. Larger trees scroll within the block; at its scroll boundary the page
continues scrolling. Tab enters/leaves the tree, and Shift+Tab returns to the
header button.
The header Copy action for the whole document still copies the original Markdown.

Invalid JSON, untagged code and blocks beyond native-view budgets remain raw.
Embedded JSON is parsed off the UI thread with a limit of 16 JSON fences attempted,
64 KiB per block, 256 KiB total, 2,048 values per tree and 128 immediate root
children. No content is discarded when a limit is exceeded. Native overlays are
clipped to the Markdown viewport and removed on document replacement; their
trusted bridge accepts only document-local slot geometry and focus IDs.

Each code block has a compact language header, defaulting to `text` for untagged
fences and indented code. The header stays above the code when scrolling long
lines and is separate from code selection/copying.

Highlighting uses bundled **Highlight.js 11.11.1**, works offline, and follows the
light/dark theme. Common languages include JavaScript (`js`), TypeScript (`ts`),
Rust, Python, JSON, YAML, Bash, SQL, CSS and HTML. Language names are
case-insensitive; the first word of the fence info string selects the language.
See the [bundled grammar list](crates/ravel-desktop/assets/markdown/highlight/README.md).
Inline code, untagged fences, and unknown languages remain plain. For bounded
rendering work, highlighting stops at 16 KiB per block, 128 KiB total code, or 128
blocks; skipped blocks still display in full. Code is never executed, and its
whitespace, copied text and original Markdown stay unchanged. Long code lines
scroll within their block rather than widening the page.

### Images, audio and video

Images render inline, including PNG, JPEG, GIF, WebP, SVG and other formats
supported by WebKit. Standard image syntax also embeds audio/video based on the
file extension; an explicit `"audio"` or `"video"` title supports extensionless URLs:

```markdown
![Picture](images/picture.png "Caption")
![Movie](media/demo.webm)
![Music](media/track.ogg)
![Stream](https://example.org/play?id=123 "video")
```

Relative paths, absolute `file:` URLs and HTTP(S) media URLs are supported. Images
scale to the available width; audio/video has play/pause, seek and volume controls,
and video supports fullscreen. Media never autoplays. Missing resources and
unsupported formats show an error rather than replacing the document. Codec
availability is determined by WebKit/GStreamer and the installed distribution
plugins; DRM sites, iframe players and arbitrary embedded HTML are not supported.

**Opening Markdown can contact remote media servers** (images load lazily and
players may fetch metadata). No document referrer, persistent cookies or stored
credentials are sent. Each document gets an ephemeral session and fresh sandbox;
only explicitly referenced local media files receive read-only sandbox access.

### Math and task lists

Inline `$...$` and display `$$...$$` math are typeset with bundled **KaTeX 0.19.0**,
including fractions, roots, matrices, aligned equations and TeX symbols supported
by KaTeX. MathML is included for accessibility. No CDN or Node runtime is needed;
typesetting works offline. Unsupported/invalid TeX remains visible with a diagnostic.
This is KaTeX's math language, not a full LaTeX document compiler: shell execution,
file includes, HTML commands and trusted URL commands are not enabled. Expansion,
size and text budgets bound math work (32 KiB per expression, 1 MiB total TeX).

`- [ ]` and `- [x]` render as accessible, keyboard-operable checkboxes, including
nested lists. **Toggles are temporary viewing state, not edits**: switching to Raw
and back preserves them, but reopening/replacing a document resets them. Source
text and files are never changed, and the header Copy button still copies the
original Markdown.

### Source and safety

**Raw Markdown** preserves source with syntax highlighting, apart from stripping a
leading UTF-8 BOM. Selection copying in Formatted copies rendered text. JSON trees,
pointers, jq actions, key hover and JSON folding do not apply to the Markdown
document itself. Embedded JSON trees have their own block-local node actions.

Embedded HTML remains literal text, including `<script>`, `<img>`, `<audio>` and
`<video>` tags. Only Ravel-generated markup and bundled application scripts run;
a restrictive Content Security Policy blocks source scripts, frames, forms and
remote scripts. Camera, microphone and other permission requests are denied.

Empty files and incomplete Markdown are valid; invalid UTF-8 leaves the previous
document intact. Parsing runs on the GIO worker pool, with limits of 16 MiB source,
32 MiB rendered output, 128 nested elements and one million parser events. Assets
and their license/update instructions are in `crates/ravel-desktop/assets/markdown`.

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

The opt-in GTK smoke executable runs on the process's actual main thread, as
required by WebKit. Normal `cargo test` skips it; `--ignored` enables it. It covers
real widgets, Markdown navigation, math, CSP, local/HTTP media, video playback,
copying and document replacement. Install GStreamer's base/good codecs, and
`xdotool` alongside Xvfb for native X11 arrow/Tab checks (CI installs these).
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
is not retained; YAML and Markdown source text is retained for their Raw views.
Editing, JSONPath, pagination, packaging and other text formats are not implemented yet.

`data/global.zak.ravel.desktop` is the desktop entry bundled for an installed
`ravel` binary. The application ID is `global.zak.ravel`.
