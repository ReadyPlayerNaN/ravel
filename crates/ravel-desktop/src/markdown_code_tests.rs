use gtk::{gio, glib};
use libadwaita::{self as adw, prelude::*};
use ravel_core::markdown::Markdown;
use webkit6::prelude::*;

use super::{eval, ready};
use crate::ui_tests::wait_for;

pub(super) fn run(app: &adw::Application) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/code.md");
    let source = std::fs::read_to_string(&path).unwrap();
    let file = gio::File::for_path(&path);
    let (window, viewer) = super::super::build(app);
    window.present();
    viewer.load(file.clone());
    wait_for(|| !viewer.spinner.is_spinning());
    let view = viewer.markdown.view().unwrap();
    ready(&view);
    assert!(eval(&view, "document.querySelector('code[data-language=javascript] .hljs-keyword').textContent === 'const'").to_boolean());
    assert!(eval(&view, "document.querySelector('code[data-language=javascript] .hljs-string').textContent === '\"Hello, Markdown!\"'").to_boolean());
    for language in ["JS", "json", "rust", "html"] {
        assert!(
            eval(
                &view,
                &format!(
                    "document.querySelector('code[data-language=\"{language}\"] span') !== null"
                )
            )
            .to_boolean(),
            "{language}"
        );
    }
    assert!(eval(&view, "Array.from(document.querySelectorAll('p code, pre code:not([data-language]), code[data-language=not-a-language]')).every(c => c.children.length === 0)").to_boolean());
    assert!(
        eval(
            &view,
            "!window.codeExecuted && !document.querySelector('pre script, pre img')"
        )
        .to_boolean()
    );
    assert!(eval(&view, "document.querySelector('code[data-language=html]').textContent.startsWith('<script>window.codeExecuted = true;</script>')").to_boolean());
    assert!(eval(&view, "getComputedStyle(document.querySelector('pre')).display === 'block' && getComputedStyle(document.querySelector('pre')).whiteSpace === 'pre'").to_boolean());
    let expected_code = "// JavaScript\nconst greeting = \"Hello, Markdown!\";\nfunction greet(name) {\n    return `${greeting} ${name}`;\n}\nwindow.codeExecuted = true;\n";
    assert_eq!(
        eval(
            &view,
            "document.querySelector('code[data-language=javascript]').textContent"
        )
        .to_string(),
        expected_code
    );
    eval(
        &view,
        "var range = document.createRange(); range.selectNodeContents(document.querySelector('code[data-language=javascript]')); getSelection().removeAllRanges(); getSelection().addRange(range)",
    );
    view.execute_editing_command("Copy");
    wait_for(|| {
        glib::MainContext::default()
            .block_on(window.clipboard().read_text_future())
            .unwrap()
            // WebKit's rendered selection may omit the final preformatted newline.
            .is_some_and(|text| text.trim_end_matches('\n') == expected_code.trim_end_matches('\n'))
    });

    let styles = adw::StyleManager::default();
    let original = styles.color_scheme();
    styles.set_color_scheme(adw::ColorScheme::ForceLight);
    wait_for(|| eval(&view, "document.documentElement.dataset.theme === 'light'").to_boolean());
    let light = eval(
        &view,
        "getComputedStyle(document.querySelector('pre .hljs-keyword')).color",
    )
    .to_string();
    assert!(eval(&view, "getComputedStyle(document.querySelector('pre .hljs-keyword')).color !== getComputedStyle(document.querySelector('pre code')).color").to_boolean());
    styles.set_color_scheme(adw::ColorScheme::ForceDark);
    wait_for(|| eval(&view, "document.documentElement.dataset.theme === 'dark'").to_boolean());
    assert_ne!(
        light,
        eval(
            &view,
            "getComputedStyle(document.querySelector('pre .hljs-keyword')).color"
        )
        .to_string()
    );
    assert!(eval(&view, "getComputedStyle(document.querySelector('pre .hljs-keyword')).color !== getComputedStyle(document.querySelector('pre code')).color").to_boolean());
    styles.set_color_scheme(original);
    assert_eq!(std::fs::read_to_string(&path).unwrap(), source);
    let raw = viewer.text.buffer();
    assert_eq!(raw.text(&raw.start_iter(), &raw.end_iter(), true), source);

    // One oversized block must not prevent a later small block from highlighting.
    // After the aggregate budget is exhausted, remaining blocks stay literal.
    let large = format!("const huge = '{}';\n", "x".repeat(16384));
    let medium = format!("const bounded = '{}';\n", "x".repeat(8000));
    let budgets = format!(
        "```javascript\n{large}```\n\n{}",
        format!("```javascript\n{medium}```\n\n").repeat(20)
    );
    let document = Markdown::parse(budgets.as_bytes()).unwrap();
    viewer.markdown.set_content(&document, &file);
    let view = viewer.markdown.view().unwrap();
    ready(&view);
    assert_eq!(
        eval(&view, "document.querySelector('pre code').textContent").to_string(),
        large
    );
    assert!(eval(&view, "!document.querySelector('pre code').children.length && document.querySelectorAll('pre code')[1].querySelector('.hljs-keyword') !== null && !document.querySelectorAll('pre code')[20].children.length").to_boolean());
    // A long, unwrapped code line scrolls inside the block, not the whole page.
    assert!(eval(&view, "document.querySelector('pre').scrollWidth > document.querySelector('pre').clientWidth && document.documentElement.scrollWidth <= innerWidth").to_boolean());

    let many = "```js\nconst tiny = 1;\n```\n\n".repeat(130);
    viewer
        .markdown
        .set_content(&Markdown::parse(many.as_bytes()).unwrap(), &file);
    let view = viewer.markdown.view().unwrap();
    ready(&view);
    assert!(eval(&view, "document.querySelectorAll('pre .hljs-keyword').length === 128 && !document.querySelectorAll('pre code')[128].children.length").to_boolean());
    window.close();
}
