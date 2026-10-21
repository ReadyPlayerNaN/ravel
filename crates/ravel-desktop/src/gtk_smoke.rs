//! GTK and WebKit must live on the process's actual main thread, not a Rust
//! test-harness worker. Opt in using the same `--ignored` flag as before.
#![expect(
    unused_imports,
    dead_code,
    reason = "Shared modules' unit-test imports/helpers compile here without their #[test] functions; the normal test target lints and runs those tests"
)]

include!("viewer_modules.rs");
mod fold_tests;
mod hover_tests;
mod structured_tests;
mod tree_navigation_tests;
mod ui_tests;
#[path = "window_smoke.rs"]
mod window;
mod yaml_tests;

fn main() {
    if std::env::args().any(|argument| argument == "--ignored") {
        ui_tests::run();
        println!("GTK smoke test passed");
    } else {
        println!("GTK smoke test skipped (requires a display; pass --ignored)");
    }
}
