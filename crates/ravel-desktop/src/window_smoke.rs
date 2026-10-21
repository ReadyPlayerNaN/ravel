// Keep white-box viewer tests beside the private state without exposing it in
// production. This module is compiled only by the harness-free GTK test target.
include!("window.rs");
#[path = "markdown_tests.rs"]
pub(crate) mod markdown_tests;
