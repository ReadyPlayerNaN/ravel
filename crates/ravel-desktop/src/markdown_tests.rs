use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

#[path = "markdown_code_tests.rs"]
mod code_tests;
#[path = "markdown_json_tests.rs"]
mod json_tests;

use super::build;
use crate::ui_tests::{find, wait_for};
use gtk::{gio, glib};
use libadwaita::{self as adw, prelude::*};
use sourceview5::prelude::*;
use webkit6::prelude::*;

#[track_caller]
fn eval(view: &webkit6::WebView, script: &str) -> webkit6::javascriptcore::Value {
    let result = std::rc::Rc::new(std::cell::RefCell::new(None));
    let received = std::rc::Rc::clone(&result);
    view.evaluate_javascript(
        script,
        None,
        None,
        None::<&gio::Cancellable>,
        move |value| {
            *received.borrow_mut() = Some(value);
        },
    );
    wait_for(|| result.borrow().is_some());
    result.borrow_mut().take().unwrap().unwrap()
}

fn ready(view: &webkit6::WebView) {
    wait_for(|| {
        !view.is_loading()
            && eval(view, "document.documentElement.dataset.ready === 'true'").to_boolean()
    });
}

fn responsive_contents(window: &adw::ApplicationWindow, view: &webkit6::WebView) {
    fn resize(window: &adw::ApplicationWindow, width: i32) {
        // Force the allocation even when Xvfb's screen is smaller than the viewport.
        window.set_size_request(width, 740);
        window.set_default_size(width, 740);
    }
    resize(window, 1200);
    wait_for(|| {
        eval(
            view,
            "innerWidth >= 1000 && document.querySelector('.contents').open",
        )
        .to_boolean()
    });
    assert!(eval(view, "document.querySelectorAll('.contents-nav').length === 1 && getComputedStyle(document.querySelector('.contents summary')).display === 'none'").to_boolean());
    assert!(eval(view, "document.querySelector('.contents-nav').getBoundingClientRect().right < document.querySelector('article').getBoundingClientRect().left").to_boolean());
    assert!(eval(view, "document.documentElement.scrollWidth <= innerWidth").to_boolean());
    eval(view, "window.scrollTo(0, 400)");
    wait_for(|| eval(view, "scrollY >= 400").to_boolean());
    assert!(
        eval(
            view,
            "Math.abs(document.querySelector('.contents-nav').getBoundingClientRect().top - 24) < 1"
        )
        .to_boolean()
    );

    resize(window, 800);
    wait_for(|| {
        eval(
            view,
            "innerWidth < 1000 && !document.querySelector('.contents').open",
        )
        .to_boolean()
    });
    eval(view, "window.scrollTo(0, 0)");
    assert!(eval(view, "getComputedStyle(document.querySelector('.contents summary')).cursor === 'default' && getComputedStyle(document.querySelector('.contents summary')).display !== 'none'").to_boolean());
    assert!(eval(view, "document.querySelector('.contents-nav').getBoundingClientRect().bottom <= document.querySelector('article').getBoundingClientRect().top").to_boolean());
    assert!(eval(view, "document.documentElement.scrollWidth <= innerWidth").to_boolean());
    eval(view, "document.querySelector('.contents summary').click()");
    assert!(eval(view, "document.querySelector('.contents').open").to_boolean());
    // Resizing within the narrow layout must not undo the user's expansion.
    resize(window, 850);
    wait_for(|| eval(view, "innerWidth > 800 && innerWidth < 1000").to_boolean());
    assert!(eval(view, "document.querySelector('.contents').open").to_boolean());

    resize(window, 1200);
    wait_for(|| {
        eval(
            view,
            "innerWidth >= 1000 && document.querySelector('.contents').open",
        )
        .to_boolean()
    });
    eval(view, "document.querySelector('.contents a').focus()");
    resize(window, 800);
    wait_for(|| {
        eval(
            view,
            "innerWidth < 1000 && !document.querySelector('.contents').open",
        )
        .to_boolean()
    });
    assert!(
        eval(
            view,
            "document.activeElement === document.querySelector('.contents summary')"
        )
        .to_boolean()
    );
    resize(window, 1100);
    wait_for(|| {
        eval(
            view,
            "innerWidth >= 1000 && document.querySelector('.contents').open",
        )
        .to_boolean()
    });
}

pub(crate) fn run(app: &adw::Application) {
    code_tests::run(app);
    json_tests::run(app);
    let examples = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let (window, viewer) = build(app);
    window.present();
    viewer.load(gio::File::for_path(examples.join("sample.json")));
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("sample.json"))
    });
    viewer.load(gio::File::for_path(examples.join("sample.md")));
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("sample.md"))
    });
    let view = &viewer.markdown.view().unwrap();
    ready(view);

    let widget = window.clone().upcast::<gtk::Widget>();
    let stack = find::<adw::ViewStack>(&widget, Some("view-stack")).unwrap();
    assert_eq!(stack.visible_child_name().as_deref(), Some("formatted"));
    assert!(!viewer.structured_page.is_visible());
    assert!(viewer.markdown_page.is_visible());
    let tree = find::<gtk::Box>(&widget, Some("structured-view")).unwrap();
    assert!(tree.first_child().is_none());
    assert!(!view.is_editable());
    assert!(view.is_focusable());
    assert!(
        eval(
            view,
            "document.querySelector('h1').textContent === 'Ravel Markdown'"
        )
        .to_boolean()
    );
    assert!(
        eval(
            view,
            "document.querySelector('strong').textContent === 'read-only'"
        )
        .to_boolean()
    );
    assert!(eval(view, "document.querySelector('.literal-html').textContent.includes('<script>not executed</script>')").to_boolean());
    assert!(
        eval(
            view,
            "document.querySelectorAll('.math .katex').length === 3"
        )
        .to_boolean()
    );
    assert!(eval(view, "document.querySelectorAll('.math math').length === 3").to_boolean());
    assert!(
        eval(
            view,
            "document.querySelectorAll('.math mfrac').length === 2"
        )
        .to_boolean()
    );
    assert!(
        eval(
            view,
            "document.querySelectorAll('.math-error').length === 0"
        )
        .to_boolean()
    );
    wait_for(|| eval(view, "document.fonts.check('16px KaTeX_Main')").to_boolean());
    responsive_contents(&window, view);

    // The contents index and footnotes use same-document navigation, including
    // focus movement and back-links for repeated references.
    eval(
        view,
        "document.querySelector('a[href=\"#lists-and-quotes\"]').click()",
    );
    wait_for(|| {
        eval(view, "location.hash === '#lists-and-quotes' && document.activeElement.id === 'lists-and-quotes'").to_boolean()
    });
    eval(
        view,
        "document.querySelector('a[role=doc-noteref]').click()",
    );
    wait_for(|| eval(view, "document.activeElement.id === 'ravel-fn-1'").to_boolean());
    eval(
        view,
        "document.querySelector('a[role=doc-backlink]').click()",
    );
    wait_for(|| eval(view, "document.activeElement.id === 'ravel-ref-1-1'").to_boolean());
    assert!(eval(view, "document.querySelector('a[href=\"https://example.org\"]').href === 'https://example.org/'").to_boolean());

    assert!(eval(view, "document.querySelector('article a[href=\"https://example.org/docs\"]').textContent === 'https://example.org/docs' && document.querySelector('article a[href=\"https://www.example.org\"]').textContent === 'www.example.org'").to_boolean());

    assert!(eval(view, "document.querySelector('article a[href=\"mailto:hello@example.org\"]').textContent === 'hello@example.org' && document.querySelector('article a[href=\"mailto:hello@example.org?subject=Hello%20Ravel\"]').textContent === 'mailto:hello@example.org?subject=Hello%20Ravel'").to_boolean());

    let styles = adw::StyleManager::default();
    let original_scheme = styles.color_scheme();
    styles.set_color_scheme(adw::ColorScheme::ForceLight);
    wait_for(|| eval(view, "document.documentElement.dataset.theme === 'light'").to_boolean());
    let light = eval(
        view,
        "getComputedStyle(document.documentElement).backgroundColor",
    )
    .to_string();
    styles.set_color_scheme(adw::ColorScheme::ForceDark);
    wait_for(|| eval(view, "document.documentElement.dataset.theme === 'dark'").to_boolean());
    assert_ne!(
        light,
        eval(
            view,
            "getComputedStyle(document.documentElement).backgroundColor"
        )
        .to_string()
    );
    styles.set_color_scheme(original_scheme);

    let source = std::fs::read_to_string(examples.join("sample.md")).unwrap();
    let was_checked = eval(view, "document.querySelector('input.task').checked").to_boolean();
    eval(view, "document.querySelector('input.task').click()");
    assert_ne!(
        was_checked,
        eval(view, "document.querySelector('input.task').checked").to_boolean()
    );
    if std::env::var("GDK_BACKEND").as_deref() == Ok("x11")
        && std::process::Command::new("xdotool")
            .arg("--version")
            .output()
            .is_ok()
    {
        assert!(view.grab_focus());
        eval(
            view,
            "document.querySelector('input.task').scrollIntoView(); document.querySelector('input.task').focus()",
        );
        assert!(
            std::process::Command::new("xdotool")
                .args(["key", "space"])
                .status()
                .unwrap()
                .success()
        );
        wait_for(|| {
            eval(view, "document.querySelector('input.task').checked").to_boolean() == was_checked
        });
    }
    assert_eq!(
        std::fs::read_to_string(examples.join("sample.md")).unwrap(),
        source
    );
    viewer.copy.emit_clicked();
    assert_eq!(
        glib::MainContext::default()
            .block_on(window.clipboard().read_text_future())
            .unwrap()
            .unwrap(),
        source
    );
    eval(
        view,
        "var range = document.createRange(); range.selectNodeContents(document.querySelector('h1')); getSelection().removeAllRanges(); getSelection().addRange(range)",
    );
    view.execute_editing_command("Copy");
    wait_for(|| {
        glib::MainContext::default()
            .block_on(window.clipboard().read_text_future())
            .unwrap()
            .is_some_and(|text| text == "Ravel Markdown")
    });

    // Real local image decode and audio/video metadata, with no autoplay. The
    // smoke-test environment needs GStreamer's base/good codec packages.
    eval(view, "document.querySelector('img').loading = 'eager'");
    wait_for(|| eval(view, "document.querySelector('img').complete").to_boolean());
    assert!(
        eval(view, "document.querySelector('img').naturalWidth === 480").to_boolean(),
        "{}",
        eval(
            view,
            "JSON.stringify({uri:location.href, img:document.querySelector('img').outerHTML, src:document.querySelector('img').src, error:document.querySelector('.media-error').hidden})"
        )
    );
    wait_for(|| {
        eval(view, "Array.from(document.querySelectorAll('audio,video')).every(m => m.readyState >= 1 || m.error)").to_boolean()
    });
    assert!(eval(view, "Array.from(document.querySelectorAll('audio,video')).every(m => m.readyState >= 1 && m.duration > 1 && m.controls && m.paused)").to_boolean(), "{}", eval(view, "JSON.stringify(Array.from(document.querySelectorAll('audio,video')).map(m => ({src:m.src, error:m.error && m.error.message, code:m.error && m.error.code, state:m.readyState, duration:m.duration})))"));
    assert!(eval(view, "document.querySelector('video').videoWidth === 320").to_boolean());
    eval(
        view,
        "window.playbackError = ''; var video = document.querySelector('video'); video.scrollIntoView(); video.muted = true; video.play().catch(e => window.playbackError = String(e)); undefined;",
    );
    wait_for(|| {
        eval(
            view,
            "!!window.playbackError || document.querySelector('video').currentTime > 0.05",
        )
        .to_boolean()
    });
    assert_eq!(eval(view, "window.playbackError").to_string(), "");
    eval(
        view,
        "document.querySelectorAll('audio,video').forEach(m => { m.pause(); m.currentTime = 1; })",
    );
    wait_for(|| {
        eval(view, "Array.from(document.querySelectorAll('audio,video')).every(m => m.paused && !m.seeking && m.currentTime >= 1)").to_boolean()
    });

    for name in ["node-copy", "node-pointer", "node-jq"] {
        assert!(!window.lookup_action(name).unwrap().is_enabled());
    }
    assert!(!viewer.pointer.is_visible());
    assert!(viewer.status.text().contains("Task changes are not saved"));
    stack.set_visible_child_name("raw");
    wait_for(|| viewer.text.is_mapped());
    let raw = viewer
        .text
        .buffer()
        .downcast::<sourceview5::Buffer>()
        .unwrap();
    assert_eq!(raw.language().unwrap().id(), "markdown");
    assert!(!viewer.text.is_editable());
    assert_eq!(raw.text(&raw.start_iter(), &raw.end_iter(), true), source);
    assert_eq!(viewer.raw_page.title().as_deref(), Some("Raw Markdown"));
    let folds = find::<sourceview5::GutterRendererText>(&widget, Some("code-folds")).unwrap();
    assert!(!folds.is_visible());

    let directory = std::env::temp_dir().join(format!(
        "ravel-markdown-tests-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let invalid = directory.join("invalid.md");
    std::fs::write(&invalid, [0xff]).unwrap();
    viewer.load(gio::File::for_path(&invalid));
    wait_for(|| window.visible_dialog().is_some());
    let alert = window
        .visible_dialog()
        .unwrap()
        .downcast::<adw::AlertDialog>()
        .unwrap();
    assert!(alert.body().contains("not UTF-8"));
    assert!(window.title().unwrap().contains("sample.md"));
    assert_eq!(stack.visible_child_name().as_deref(), Some("raw"));
    assert_eq!(
        viewer
            .text
            .buffer()
            .text(&raw.start_iter(), &raw.end_iter(), true),
        source
    );
    alert.close();

    // HTTP media uses WebKit's loader; no source scripts, cookies, or referrers.
    let server = MediaServer::start();
    let remote = directory.join("media and math.md");
    std::fs::write(&remote, format!("# Media\n\n![Remote]({}/image.svg)\n\n![Audio]({}/tone.ogg)\n\n![Video]({}/demo.webm)\n\n![Missing]({}/missing.png)\n\n$\\notARealCommand$\n\n$\\href{{javascript:alert(1)}}{{evil}}$\n\n<script>window.sourceExecuted = true</script>\n\n[unsafe](javascript:alert%281%29)", server.url, server.url, server.url, server.url)).unwrap();
    viewer.load(gio::File::for_path(&remote));
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("media and math.md"))
    });
    let view = &viewer.markdown.view().unwrap();
    ready(view);
    eval(
        view,
        "document.querySelectorAll('img').forEach(i => i.loading = 'eager')",
    );
    wait_for(|| eval(view, "document.querySelector('img').complete").to_boolean());
    assert!(
        eval(view, "document.querySelector('img').naturalWidth === 480").to_boolean(),
        "{}; requests: {:?}",
        eval(
            view,
            "JSON.stringify({img:document.querySelector('img').src,error:document.querySelector('.media-error').hidden})"
        ),
        server.requests.lock().unwrap()
    );
    wait_for(|| {
        eval(
            view,
            "Array.from(document.querySelectorAll('audio,video')).every(m => m.readyState >= 1 || m.error)",
        )
        .to_boolean()
    });
    assert!(
        eval(
            view,
            "Array.from(document.querySelectorAll('audio,video')).every(m => m.readyState >= 1)"
        )
        .to_boolean(),
        "{}; requests: {:?}",
        eval(
            view,
            "JSON.stringify(Array.from(document.querySelectorAll('audio,video')).map(m => ({src:m.src, error:m.error && m.error.message, code:m.error && m.error.code, state:m.readyState})))"
        ),
        server.requests.lock().unwrap()
    );
    wait_for(|| {
        eval(
            view,
            "!!document.querySelector('.media-error:not([hidden])')",
        )
        .to_boolean()
    });
    assert!(eval(view, "!!document.querySelector('.math-error')").to_boolean());
    assert!(eval(view, "typeof window.sourceExecuted === 'undefined' && !document.querySelector('a[href^=\"javascript:\"]')").to_boolean());
    eval(
        view,
        "window.cspBlocked = false; document.addEventListener('securitypolicyviolation', () => window.cspBlocked = true); var injected = document.createElement('script'); injected.textContent = 'window.sourceExecuted = true'; document.body.append(injected)",
    );
    wait_for(|| eval(view, "window.cspBlocked").to_boolean());
    assert!(eval(view, "typeof window.sourceExecuted === 'undefined'").to_boolean());
    assert!(
        server
            .requests
            .lock()
            .unwrap()
            .iter()
            .all(|request| !request.to_ascii_lowercase().contains("referer:")
                && !request.to_ascii_lowercase().contains("cookie:"))
    );

    viewer.load(gio::File::for_path(examples.join("sample.json")));
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("sample.json"))
    });
    assert_eq!(stack.visible_child_name().as_deref(), Some("structured"));
    assert!(!viewer.markdown_page.is_visible());
    assert!(viewer.structured_page.is_visible());
    assert!(viewer.markdown.view().is_none());
    assert!(view.parent().is_none());
    assert!(
        view.is_muted(),
        "Detached views must stop producing audio immediately"
    );
    assert!(viewer.pointer.is_visible());
    assert!(folds.is_visible());
    assert_eq!(raw.language().unwrap().id(), "json");
    assert!(window.lookup_action("node-jq").unwrap().is_enabled());

    let empty = directory.join("empty.MARKDOWN");
    std::fs::write(&empty, b"\xef\xbb\xbf").unwrap();
    viewer.load(gio::File::for_path(examples.join("sample.md")));
    viewer.load(gio::File::for_path(&empty));
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("empty.MARKDOWN"))
    });
    let view = &viewer.markdown.view().unwrap();
    ready(view);
    assert!(eval(view, "document.querySelector('article').textContent === ''").to_boolean());
    assert_eq!(stack.visible_child_name().as_deref(), Some("formatted"));
    assert_eq!(viewer.text.buffer().char_count(), 0);
    viewer.load(gio::File::for_path(examples.join("sample.yaml")));
    wait_for(|| {
        window
            .title()
            .is_some_and(|title| title.contains("sample.yaml"))
    });
    assert_eq!(stack.visible_child_name().as_deref(), Some("structured"));
    assert_eq!(viewer.raw_page.title().as_deref(), Some("Raw YAML"));
    assert!(!folds.is_visible());
    window.close();
    std::fs::remove_dir_all(directory).unwrap();
}

struct MediaServer {
    url: String,
    requests: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl MediaServer {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let requests = Arc::new(Mutex::new(Vec::new()));
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_requests = Arc::clone(&requests);
        let thread = std::thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                let Ok((mut stream, _)) = listener.accept() else {
                    std::thread::sleep(Duration::from_millis(5));
                    continue;
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(1)))
                    .unwrap();
                let mut bytes = [0; 8192];
                let Ok(length) = stream.read(&mut bytes) else {
                    continue;
                };
                let request = String::from_utf8_lossy(&bytes[..length]).into_owned();
                let (mime, body): (&str, &[u8]) = match request.split_whitespace().nth(1) {
                    Some("/image.svg") => (
                        "image/svg+xml",
                        include_bytes!("../../../examples/media/ravel.svg"),
                    ),
                    Some("/tone.ogg") => (
                        "audio/ogg",
                        include_bytes!("../../../examples/media/tone.ogg"),
                    ),
                    Some("/demo.webm") => (
                        "video/webm",
                        include_bytes!("../../../examples/media/demo.webm"),
                    ),
                    _ => ("text/plain", b""),
                };
                let range = request
                    .lines()
                    .find_map(|line| line.strip_prefix("Range: bytes="))
                    .and_then(|range| range.split_once('-'))
                    .and_then(|(start, end)| {
                        Some((
                            start.parse::<usize>().ok()?,
                            end.parse::<usize>().unwrap_or(body.len().saturating_sub(1)),
                        ))
                    })
                    .filter(|&(start, end)| start <= end && end < body.len());
                thread_requests.lock().unwrap().push(request);
                let (status, range_header, body) = if let Some((start, end)) = range {
                    (
                        "206 Partial Content",
                        format!("Content-Range: bytes {start}-{end}/{}\r\n", body.len()),
                        &body[start..=end],
                    )
                } else {
                    (
                        if body.is_empty() {
                            "404 Not Found"
                        } else {
                            "200 OK"
                        },
                        String::new(),
                        body,
                    )
                };
                let header = format!(
                    "HTTP/1.1 {status}\r\nContent-Type: {mime}\r\nAccept-Ranges: bytes\r\n{range_header}Content-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream
                    .write_all(header.as_bytes())
                    .and_then(|_| stream.write_all(body));
            }
        });
        Self {
            url,
            requests,
            stop,
            thread: Some(thread),
        }
    }
}

impl Drop for MediaServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
    }
}
