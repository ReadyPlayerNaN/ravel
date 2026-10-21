//! Rich Markdown in a restricted WebKit view. Only generated markup and bundled
//! application scripts run; source HTML remains escaped by ravel-core.

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

use gtk::{gio, glib};
use include_dir::{Dir, include_dir};
use libadwaita::{self as adw, prelude::*};
use ravel_core::markdown::Markdown;
use url::Url;
use webkit6::{self as webkit, prelude::*};

static ASSETS: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/assets/markdown");
const CSP: &str = "default-src 'none'; script-src ravel-assets:; style-src ravel-assets: 'unsafe-inline'; font-src ravel-assets:; img-src https: http: file: data:; media-src https: http: file:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'";

pub struct MarkdownView {
    widget: gtk::Box,
    view: RefCell<Option<webkit::WebView>>,
    window: glib::WeakRef<adw::ApplicationWindow>,
    error: gtk::Label,
}

impl MarkdownView {
    pub fn new(window: &adw::ApplicationWindow) -> Self {
        let error = gtk::Label::builder()
            .wrap(true)
            .selectable(true)
            .visible(false)
            .margin_top(8)
            .margin_bottom(8)
            .margin_start(12)
            .margin_end(12)
            .build();
        error.set_widget_name("markdown-error");
        let widget = gtk::Box::new(gtk::Orientation::Vertical, 0);
        widget.append(&error);
        Self {
            widget,
            view: RefCell::new(None),
            window: window.downgrade(),
            error,
        }
    }

    fn create_view(&self, document: &Markdown, uri: &str) -> Option<webkit::WebView> {
        let window = self.window.upgrade()?;
        let error = &self.error;
        let context = webkit::WebContext::new();
        register_assets(&context);
        // GStreamer opens local media inside the web-process sandbox. Grant only
        // this document's explicit media files, before starting a fresh process.
        // Never disable the sandbox or grant access to whole document directories.
        if let Ok(base) = Url::parse(uri) {
            for source in document.media_sources() {
                if let Ok(url) = base.join(source)
                    && let Ok(path) = url.to_file_path()
                    && path.is_file()
                {
                    context.add_path_to_sandbox(path, true);
                }
            }
        }
        let session = webkit::NetworkSession::new_ephemeral();
        if let Some(cookies) = session.cookie_manager() {
            cookies.set_accept_policy(webkit::CookieAcceptPolicy::Never);
        }
        session.set_persistent_credential_storage_enabled(false);
        session.connect_download_started(|_, download| download.cancel());
        let settings = webkit::Settings::builder()
            .default_font_family("sans-serif")
            .default_font_size(16)
            .enable_javascript(true)
            .javascript_can_open_windows_automatically(false)
            .javascript_can_access_clipboard(false)
            .enable_html5_database(false)
            .enable_html5_local_storage(false)
            .enable_page_cache(false)
            .enable_dns_prefetching(false)
            .enable_media_stream(false)
            .enable_webrtc(false)
            .enable_webgl(false)
            .enable_fullscreen(true)
            .media_playback_requires_user_gesture(true)
            .media_playback_allows_inline(true)
            .allow_file_access_from_file_urls(true)
            .allow_universal_access_from_file_urls(false)
            .enable_tabs_to_links(true)
            .enable_write_console_messages_to_stdout(cfg!(test))
            .build();
        let view = webkit::WebView::builder()
            .web_context(&context)
            .network_session(&session)
            .settings(&settings)
            .focusable(true)
            .vexpand(true)
            .hexpand(true)
            .build();
        view.set_widget_name("markdown-formatted");
        view.set_is_muted(false);
        install_navigation(&view, &window, uri, error);
        view.connect_permission_request(|_, request| {
            request.deny();
            true
        });
        view.connect_load_failed(glib::clone!(
            #[weak]
            error,
            #[upgrade_or]
            true,
            move |view, _, _, failure| {
                if view.parent().is_some() && !failure.matches(webkit::NetworkError::Cancelled) {
                    error.set_text(&format!(
                        "Formatted view could not load: {failure}. Raw Markdown remains available."
                    ));
                    error.set_visible(true);
                }
                true
            }
        ));
        view.connect_web_process_terminated(glib::clone!(#[weak] error, move |view, _| {
            if view.parent().is_some() {
                error.set_text("The formatted viewer stopped. Reopen the document to retry; Raw Markdown remains available.");
                error.set_visible(true);
            }
        }));
        install_fullscreen(&view, &window);
        let styles = adw::StyleManager::default();
        styles.connect_dark_notify(glib::clone!(
            #[weak]
            view,
            move |styles| {
                if view.parent().is_some() {
                    set_theme(&view, styles.is_dark());
                }
            }
        ));
        view.connect_load_changed(|view, event| {
            if event == webkit::LoadEvent::Finished && view.parent().is_some() {
                set_theme(view, adw::StyleManager::default().is_dark());
            }
        });
        Some(view)
    }

    pub(super) fn view(&self) -> Option<webkit::WebView> {
        self.view.borrow().clone()
    }

    pub fn widget(&self) -> &gtk::Box {
        &self.widget
    }

    pub fn set_content(&self, document: &Markdown, file: &gio::File) {
        self.clear();
        let uri = file.uri().to_string();
        let Some(view) = self.create_view(document, &uri) else {
            return;
        };
        self.widget.append(&view);
        let theme = if adw::StyleManager::default().is_dark() {
            "dark"
        } else {
            "light"
        };
        let html = format!(
            "<!doctype html><html data-theme=\"{theme}\"><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"{CSP}\"><meta name=\"referrer\" content=\"no-referrer\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"><link rel=\"stylesheet\" href=\"ravel-assets:///katex/katex.min.css\"><link rel=\"stylesheet\" href=\"ravel-assets:///reader.css\"></head><body>{}<script src=\"ravel-assets:///katex/katex.min.js\"></script><script src=\"ravel-assets:///reader.js\"></script></body></html>",
            document.html()
        );
        view.load_html(&html, Some(&uri));
        *self.view.borrow_mut() = Some(view);
    }

    pub fn clear(&self) {
        if let Some(view) = self.view() {
            self.view.borrow_mut().take();
            self.widget.remove(&view);
            view.set_is_muted(true);
            view.stop_loading();
            // Each document owns its context/process. Terminating it releases
            // its media, resource access and tasks even if callbacks retain a view.
            view.terminate_web_process();
        }
        self.error.set_visible(false);
    }
}

fn register_assets(context: &webkit::WebContext) {
    if let Some(security) = context.security_manager() {
        security.register_uri_scheme_as_secure("ravel-assets");
        security.register_uri_scheme_as_cors_enabled("ravel-assets");
    }
    context.register_uri_scheme("ravel-assets", |request| {
        let path = request.path().unwrap_or_default();
        let Some(file) = ASSETS.get_file(path.trim_start_matches('/')) else {
            request.finish_error(&mut glib::Error::new(
                gio::IOErrorEnum::NotFound,
                "Unknown bundled Markdown asset",
            ));
            return;
        };
        let bytes = glib::Bytes::from_static(file.contents());
        let stream = gio::MemoryInputStream::from_bytes(&bytes);
        let response = webkit::URISchemeResponse::new(&stream, bytes.len() as i64);
        response.set_content_type(if path.ends_with(".css") {
            "text/css"
        } else if path.ends_with(".js") {
            "application/javascript"
        } else if path.ends_with(".woff2") {
            "font/woff2"
        } else {
            "text/plain"
        });
        let headers = webkit::soup::MessageHeaders::new(webkit::soup::MessageHeadersType::Response);
        headers.append("Access-Control-Allow-Origin", "*");
        response.set_http_headers(headers);
        request.finish_with_response(&response);
    });
}

fn set_theme(view: &webkit::WebView, dark: bool) {
    let background = if dark { 36.0 / 255.0 } else { 1.0 };
    view.set_background_color(&gtk::gdk::RGBA::new(
        background, background, background, 1.0,
    ));
    view.evaluate_javascript(
        if dark {
            "document.documentElement.dataset.theme = 'dark';"
        } else {
            "document.documentElement.dataset.theme = 'light';"
        },
        None,
        None,
        None::<&gio::Cancellable>,
        |_| {},
    );
}

#[derive(Debug, PartialEq, Eq)]
enum LinkTarget {
    Internal,
    External,
    Blocked,
}

fn link_target(base: &str, target: &str) -> LinkTarget {
    let Ok(mut destination) = Url::parse(target) else {
        return LinkTarget::Blocked;
    };
    if destination.fragment().is_some()
        && let Ok(mut document) = Url::parse(base)
    {
        destination.set_fragment(None);
        document.set_fragment(None);
        if destination == document {
            return LinkTarget::Internal;
        }
    }
    match destination.scheme() {
        "https" | "http" | "mailto" | "file" => LinkTarget::External,
        _ => LinkTarget::Blocked,
    }
}

fn install_navigation(
    view: &webkit::WebView,
    window: &adw::ApplicationWindow,
    base: &str,
    error: &gtk::Label,
) {
    let base = base.to_owned();
    view.connect_decide_policy(glib::clone!(
        #[weak]
        window,
        #[weak]
        error,
        #[upgrade_or]
        true,
        move |_, decision, kind| {
            if !matches!(
                kind,
                webkit::PolicyDecisionType::NavigationAction
                    | webkit::PolicyDecisionType::NewWindowAction
            ) {
                return false;
            }
            let Some(action) = decision
                .downcast_ref::<webkit::NavigationPolicyDecision>()
                .and_then(|decision| decision.navigation_action())
            else {
                decision.ignore();
                return true;
            };
            let Some(uri) = action.request().and_then(|request| request.uri()) else {
                decision.ignore();
                return true;
            };
            let initial = action.navigation_type() == webkit::NavigationType::Other
                && (uri.as_str() == base.as_str() || uri == "about:blank");
            if initial || link_target(&base, &uri) == LinkTarget::Internal {
                decision.use_();
            } else {
                decision.ignore();
                if action.is_user_gesture() && link_target(&base, &uri) == LinkTarget::External {
                    open_external(&window, &uri, &error);
                }
            }
            true
        }
    ));
}

fn open_external(window: &adw::ApplicationWindow, uri: &str, error: &gtk::Label) {
    let uri = uri.to_owned();
    glib::spawn_future_local(glib::clone!(
        #[weak]
        window,
        #[weak]
        error,
        async move {
            let result = if uri.starts_with("file:") {
                // Local hyperlinks must not silently launch executables or active HTML.
                let launcher = gtk::FileLauncher::new(Some(&gio::File::for_uri(&uri)));
                launcher.set_always_ask(true);
                launcher.launch_future(Some(&window)).await
            } else {
                gtk::UriLauncher::new(&uri)
                    .launch_future(Some(&window))
                    .await
            };
            if let Err(failure) = result
                && !failure.matches(gtk::DialogError::Dismissed)
                && !failure.matches(gtk::DialogError::Cancelled)
            {
                error.set_text(&format!("Could not open link: {failure}"));
                error.set_visible(true);
            }
        }
    ));
}

fn install_fullscreen(view: &webkit::WebView, window: &adw::ApplicationWindow) {
    let was_fullscreen = Rc::new(Cell::new(false));
    view.connect_enter_fullscreen(glib::clone!(
        #[weak]
        window,
        #[strong]
        was_fullscreen,
        #[upgrade_or]
        true,
        move |_| {
            was_fullscreen.set(window.is_fullscreen());
            window.fullscreen();
            true
        }
    ));
    view.connect_leave_fullscreen(glib::clone!(
        #[weak]
        window,
        #[upgrade_or]
        true,
        move |_| {
            if !was_fullscreen.get() {
                window.unfullscreen();
            }
            true
        }
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_same_document_fragments_navigate_inside_the_viewer() {
        let base = "file:///home/user/notes.md";
        assert_eq!(
            link_target(base, "file:///home/user/notes.md#heading"),
            LinkTarget::Internal
        );
        assert_eq!(
            link_target(base, "file:///home/user/other.md#heading"),
            LinkTarget::External
        );
        assert_eq!(
            link_target(base, "https://example.org/#heading"),
            LinkTarget::External
        );
        assert_eq!(
            link_target(base, "mailto:user@example.org"),
            LinkTarget::External
        );
        for target in [
            "javascript:alert(1)",
            "data:text/html,evil",
            "ravel-assets:///reader.js",
            "ftp://example.org/file",
        ] {
            assert_eq!(link_target(base, target), LinkTarget::Blocked);
        }
    }
}
