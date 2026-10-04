# Ravel

Ravel is a read-only GTK4 + Libadwaita document viewer, initially for JSON and
JSON Lines. Keep the application simple; do not add editing or network services
without an explicit requirement.

## Architecture

- `crates/ravel-core`: parsing, document values, JSON Pointers, structural search
  and filtering. No GTK, GIO, or application-runtime dependencies.
- `crates/ravel-desktop`: native UI, file opening, application actions, and errors.
  `preview.rs` configures the read-only GtkSourceView JSON preview and binds its
  color scheme to Libadwaita's light/dark theme. `key_index.rs` indexes rendered
  keys and their array-wildcard structural paths; `key_hover.rs` handles pointer,
  Shift, and viewport highlighting. Keep highlighting in this layer.
  Use the GLib main context for UI tasks and GIO's worker pool for parsing.
- `examples`: small documents for manual and automated smoke tests.

Prefer concrete types and small modules. Preserve the inward dependency rule
from the hexagonal skill, without inventing repository traits or backend layers
for this desktop application. GTK objects stay on the main thread. Connect UI
signals with weak captures and avoid strong state/window reference cycles.

## Reused Rust skills

Copied from `~/Work/boardo/.agents/skills`, including their reference material:

- `.agents/skills/rust-best-practices/SKILL.md`: load when writing/reviewing Rust.
- `.agents/skills/rust-async-patterns/SKILL.md`: load for concurrent/async work;
  adapt runtime-specific guidance to GLib/GIO rather than adding Tokio by default.
- `.agents/skills/rust-hexagonal/SKILL.md`: load when changing layer boundaries;
  its server-specific scaffolding does not apply to this small desktop app.

`.pi/skills` contains repository-relative symlinks to these copies. This project
must not depend on Boardo being present on another developer's machine. Original
skill metadata and attribution are retained. Do not copy Boardo domain rules.

## Git workflow

- Use strict Conventional Commits for every commit.
- Commit each functional piece of work before starting the next piece; do not
  carry uncommitted functional changes forward between pieces.
- Slice plans into small, independently committable functional additions that
  build on the existing state. Each addition should leave the project in a
  coherent, working state.

## Verification

```sh
cargo fmt --all --check
cargo test --workspace --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
GDK_BACKEND=x11 GSK_RENDERER=cairo GTK_A11Y=none GIO_USE_VFS=local \
  xvfb-run -a cargo test -p ravel-desktop --locked -- --ignored
```

Prefer core tests for document behavior. The explicitly ignored GTK test needs a
real display or Xvfb. Keep `Cargo.lock` tracked, document public core APIs, avoid
production `unwrap`/`expect`, and return useful parse errors with source location.
