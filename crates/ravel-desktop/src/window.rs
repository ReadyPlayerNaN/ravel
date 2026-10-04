use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use gtk::{gio, glib};
use libadwaita::{self as adw, prelude::*};

use ravel_core::{Document, Format};

struct Viewer {
    window: glib::WeakRef<adw::ApplicationWindow>,
    title: adw::WindowTitle,
    stack: gtk::Stack,
    text: sourceview5::View,
    hover: Rc<crate::key_hover::KeyHover>,
    structured: Rc<crate::structured::Structured>,
    _folding: Rc<crate::folding::Folding>,
    pointer: gtk::Label,
    status: gtk::Label,
    spinner: gtk::Spinner,
    document: RefCell<Option<Arc<Document>>>,
    generation: Cell<u64>,
}

pub fn create(app: &adw::Application, file: Option<gio::File>) -> adw::ApplicationWindow {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Ravel")
        .default_width(1100)
        .default_height(740)
        .build();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let title = adw::WindowTitle::new("Ravel", "JSON document viewer");
    header.set_title_widget(Some(&title));
    let open = gtk::Button::builder()
        .label("Open…")
        .action_name("win.open")
        .tooltip_text("Open a JSON document (Ctrl+O)")
        .build();
    header.pack_start(&open);
    let spinner = gtk::Spinner::new();
    header.pack_end(&spinner);
    toolbar.add_top_bar(&header);

    let stack = gtk::Stack::new();
    let empty = adw::StatusPage::builder()
        .icon_name("text-x-generic-symbolic")
        .title("Untangle your JSON")
        .description("Open a .json or .jsonl file to explore its structure.")
        .build();
    let empty_open = gtk::Button::builder()
        .label("Open Document…")
        .action_name("win.open")
        .halign(gtk::Align::Center)
        .css_classes(["suggested-action", "pill"])
        .build();
    empty.set_child(Some(&empty_open));
    stack.add_named(&empty, Some("empty"));

    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let detail = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let detail_header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    detail_header.set_margin_top(8);
    detail_header.set_margin_bottom(8);
    detail_header.set_margin_start(12);
    detail_header.set_margin_end(12);
    let pointer = gtk::Label::builder()
        .label("$ (document root)")
        .xalign(0.0)
        .hexpand(true)
        .selectable(true)
        .ellipsize(gtk::pango::EllipsizeMode::Middle)
        .build();
    let copy = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text("Copy selected JSON")
        .css_classes(["flat"])
        .build();
    copy.set_widget_name("copy-json");
    let (structured_widget, structured) = crate::structured::create();
    let text = crate::preview::create();
    let detail_scroll = gtk::ScrolledWindow::builder()
        .child(&text)
        .vexpand(true)
        .hexpand(true)
        .build();
    let view_stack = adw::ViewStack::new();
    view_stack.set_widget_name("view-stack");
    view_stack.add_titled_with_icon(
        &structured_widget,
        Some("structured"),
        "Structured",
        "view-list-symbolic",
    );
    view_stack.add_titled_with_icon(
        &detail_scroll,
        Some("raw"),
        "Raw",
        "text-x-generic-symbolic",
    );
    view_stack.set_property("visible-child-name", "structured");
    let switcher = adw::ViewSwitcher::builder()
        .stack(&view_stack)
        .policy(adw::ViewSwitcherPolicy::Wide)
        .build();
    detail_header.append(&switcher);
    detail_header.append(&pointer);
    detail_header.append(&copy);
    detail.append(&detail_header);
    detail.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    detail.append(&view_stack);
    detail.set_vexpand(true);
    content.append(&detail);
    let status = gtk::Label::builder()
        .xalign(0.0)
        .margin_start(12)
        .margin_end(12)
        .margin_top(6)
        .margin_bottom(6)
        .css_classes(["dim-label"])
        .build();
    status.set_widget_name("document-statistics");
    status.set_wrap(true);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&status);
    stack.add_named(&content, Some("document"));
    stack.set_visible_child_name("empty");
    toolbar.set_content(Some(&stack));
    window.set_content(Some(&toolbar));
    let hover = crate::key_hover::KeyHover::install(&text, &window);
    let folding = crate::folding::Folding::install(&text);

    let viewer = Rc::new(Viewer {
        window: window.downgrade(),
        title,
        stack,
        text,
        hover,
        structured,
        _folding: folding,
        pointer,
        status,
        spinner,
        document: RefCell::new(None),
        generation: Cell::new(0),
    });
    let open_action = gio::SimpleAction::new("open", None);
    open_action.connect_activate(glib::clone!(
        #[weak]
        viewer,
        move |_, _| viewer.choose_file()
    ));
    window.add_action(&open_action);
    let node_copy = gio::SimpleAction::new("node-copy", Some(glib::VariantTy::STRING));
    node_copy.connect_activate(glib::clone!(
        #[weak]
        viewer,
        move |_, pointer| {
            viewer.copy_node(
                pointer.and_then(|value| value.str()).unwrap_or_default(),
                false,
            );
        }
    ));
    window.add_action(&node_copy);
    let node_pointer = gio::SimpleAction::new("node-pointer", Some(glib::VariantTy::STRING));
    node_pointer.connect_activate(glib::clone!(
        #[weak]
        viewer,
        move |_, pointer| {
            viewer.copy_node(
                pointer.and_then(|value| value.str()).unwrap_or_default(),
                true,
            );
        }
    ));
    window.add_action(&node_pointer);
    let node_jq = gio::SimpleAction::new(
        "node-jq",
        Some(&<(String, String, String)>::static_variant_type()),
    );
    node_jq.connect_activate(glib::clone!(
        #[weak]
        viewer,
        move |_, target| {
            if let Some((pointer, program, label)) =
                target.and_then(|value| value.get::<(String, String, String)>())
            {
                viewer.run_jq(&pointer, &program, &label);
            }
        }
    ));
    window.add_action(&node_jq);
    copy.connect_clicked(glib::clone!(
        #[weak]
        viewer,
        move |_| {
            let buffer = viewer.text.buffer();
            viewer.text.clipboard().set_text(&buffer.text(
                &buffer.start_iter(),
                &buffer.end_iter(),
                true,
            ));
        }
    ));
    // Keep state alive with the window, without a strong reference back to it.
    window.connect_destroy({
        let viewer = Rc::clone(&viewer);
        move |_| {
            let _ = &viewer;
        }
    });
    window.present();
    if let Some(file) = file {
        viewer.load(file);
    }
    window
}

impl Viewer {
    fn choose_file(self: &Rc<Self>) {
        let Some(window) = self.window.upgrade() else {
            return;
        };
        let filter = gtk::FileFilter::new();
        filter.set_name(Some("JSON documents (.json, .jsonl)"));
        filter.add_suffix("json");
        filter.add_suffix("jsonl");
        let all = gtk::FileFilter::new();
        all.set_name(Some("All files"));
        all.add_pattern("*");
        let filters = gio::ListStore::new::<gtk::FileFilter>();
        filters.append(&filter);
        filters.append(&all);
        let dialog = gtk::FileDialog::builder()
            .title("Open Document")
            .filters(&filters)
            .default_filter(&filter)
            .build();
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = viewer)]
            self,
            async move {
                match dialog.open_future(Some(&window)).await {
                    Ok(file) => viewer.load(file),
                    Err(error)
                        if error.matches(gtk::DialogError::Dismissed)
                            || error.matches(gtk::DialogError::Cancelled) => {}
                    Err(error) => viewer.show_error(&error.to_string()),
                }
            }
        ));
    }

    fn load(self: &Rc<Self>, file: gio::File) {
        let generation = self.generation.get() + 1;
        self.generation.set(generation);
        self.spinner.start();
        let name = file
            .basename()
            .map(|path| path.to_string_lossy().into_owned())
            .unwrap_or_else(|| file.uri().into());
        let format = Format::from_filename(&name);
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = viewer)]
            self,
            async move {
                let result = async {
                    let (bytes, _) = file
                        .load_contents_future()
                        .await
                        .map_err(|error| error.to_string())?;
                    gio::spawn_blocking(move || Document::parse(&bytes, format))
                        .await
                        .map_err(|_| "Document parsing worker failed".to_owned())?
                        .map_err(|error| error.to_string())
                }
                .await;
                // An older asynchronous open must not replace a newer document.
                if viewer.generation.get() != generation {
                    return;
                }
                viewer.spinner.stop();
                match result {
                    Ok(document) => {
                        *viewer.document.borrow_mut() = Some(Arc::new(document));
                        viewer.title.set_title(&name);
                        viewer.title.set_subtitle("Ravel · Read only");
                        if let Some(window) = viewer.window.upgrade() {
                            window.set_title(Some(&format!("{name} — Ravel")));
                        }

                        viewer.stack.set_visible_child_name("document");
                        viewer.show_document();
                    }
                    Err(message) => viewer.show_error(&message),
                }
            }
        ));
    }

    fn show_document(&self) {
        let document = self.document.borrow();
        let Some(document) = document.as_ref() else {
            return;
        };
        let Some(value) = document.value_at("") else {
            return;
        };
        match serde_json::to_string_pretty(value) {
            Ok(text) => self.hover.set_content(value, "", &text),
            Err(error) => {
                self.show_error(&error.to_string());
                return;
            }
        }
        self.structured.set_content(document, "");
        self.pointer.set_text("$ (document root)");
        let stats = document.statistics();
        let format = if document.format() == Format::JsonLines {
            format!("JSON Lines · {} records", document.root().children.len())
        } else {
            "JSON".to_owned()
        };
        self.status.set_text(&format!(
            "{format} · {} values · {} objects · {} arrays · {} keys · {} strings · {} numbers · {} booleans · {} nulls · Depth {} · Read only",
            stats.values, stats.objects, stats.arrays, stats.keys, stats.strings,
            stats.numbers, stats.booleans, stats.nulls, stats.max_depth,
        ));
    }

    /// Copies a node's JSON or its JSON Pointer to the clipboard.
    fn copy_node(&self, pointer: &str, as_pointer: bool) {
        let document = self.document.borrow();
        let text = if as_pointer {
            Some(pointer.to_owned())
        } else {
            document
                .as_ref()
                .and_then(|document| document.value_at(pointer))
                .and_then(|value| serde_json::to_string_pretty(value).ok())
        };
        if let Some(text) = text
            && let Some(window) = self.window.upgrade()
        {
            window.clipboard().set_text(&text);
        }
    }

    /// Runs a jq operation on a node and presents the result in a dialog.
    fn run_jq(self: &Rc<Self>, pointer: &str, program: &str, label: &str) {
        let document = self.document.borrow();
        let Some(value) = document
            .as_ref()
            .and_then(|document| document.value_at(pointer))
        else {
            return;
        };
        let Ok(input) = serde_json::to_string(value) else {
            return;
        };
        let title = format!("jq: {label}");
        let program = program.to_owned();
        glib::spawn_future_local(glib::clone!(
            #[weak(rename_to = viewer)]
            self,
            async move {
                match crate::jq::run(&program, &input).await {
                    Ok(output) => {
                        if let Some(window) = viewer.window.upgrade() {
                            crate::jq::show_result(&window, &title, &output);
                        }
                    }
                    Err(message) => viewer.show_error(&message),
                }
            }
        ));
    }

    fn show_error(&self, message: &str) {
        if let Some(window) = self.window.upgrade() {
            let dialog = adw::AlertDialog::new(Some("Could not open document"), Some(message));
            dialog.add_response("close", "Close");
            dialog.set_default_response(Some("close"));
            dialog.set_close_response("close");
            dialog.present(Some(&window));
        }
    }
}
