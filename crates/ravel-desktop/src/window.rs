use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    sync::Arc,
};

use gtk::{gio, glib};
use libadwaita::{self as adw, prelude::*};

use ravel_core::{Document, Format, Kind, Node};

const KINDS: [Kind; 6] = [
    Kind::Object,
    Kind::Array,
    Kind::String,
    Kind::Number,
    Kind::Boolean,
    Kind::Null,
];

struct Viewer {
    window: glib::WeakRef<adw::ApplicationWindow>,
    title: adw::WindowTitle,
    stack: gtk::Stack,
    tree_stack: gtk::Stack,
    search: gtk::SearchEntry,
    kind: gtk::DropDown,
    selection: gtk::SingleSelection,
    text: sourceview5::View,
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
        .description("Open a .json or .jsonl file to browse, search and filter its structure.")
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
    let controls = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    controls.set_margin_top(12);
    controls.set_margin_bottom(12);
    controls.set_margin_start(12);
    controls.set_margin_end(12);
    let search = gtk::SearchEntry::builder()
        .placeholder_text("Search keys, paths and values…")
        .hexpand(true)
        .tooltip_text("Case-insensitive substring search (Ctrl+F)")
        .build();
    let kind = gtk::DropDown::from_strings(&[
        "All types",
        "Objects",
        "Arrays",
        "Strings",
        "Numbers",
        "Booleans",
        "Null",
    ]);
    kind.set_tooltip_text(Some(
        "Filter matching values by JSON type; ancestors remain visible",
    ));
    controls.append(&search);
    controls.append(&kind);
    content.append(&controls);
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));

    let selection = gtk::SingleSelection::new(None::<gio::ListModel>);
    let list = gtk::ListView::new(Some(selection.clone()), Some(tree_factory()));
    list.set_css_classes(&["navigation-sidebar"]);
    list.set_widget_name("structure-tree");
    let tree_scroll = gtk::ScrolledWindow::builder()
        .child(&list)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();
    let tree_stack = gtk::Stack::new();
    tree_stack.add_named(&tree_scroll, Some("tree"));
    let no_matches = adw::StatusPage::builder()
        .icon_name("edit-find-symbolic")
        .title("No matching values")
        .description("Try another search or choose All types.")
        .build();
    tree_stack.add_named(&no_matches, Some("no-matches"));
    tree_stack.set_size_request(300, -1);

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
    detail_header.append(&pointer);
    detail_header.append(&copy);
    detail.append(&detail_header);
    detail.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    let text = crate::preview::create();
    let detail_scroll = gtk::ScrolledWindow::builder()
        .child(&text)
        .vexpand(true)
        .hexpand(true)
        .build();
    detail.append(&detail_scroll);
    let paned = gtk::Paned::builder()
        .orientation(gtk::Orientation::Horizontal)
        .position(420)
        .start_child(&tree_stack)
        .end_child(&detail)
        .vexpand(true)
        .shrink_start_child(false)
        .shrink_end_child(false)
        .build();
    content.append(&paned);
    let status = gtk::Label::builder()
        .xalign(0.0)
        .margin_start(12)
        .margin_end(12)
        .margin_top(6)
        .margin_bottom(6)
        .css_classes(["dim-label"])
        .build();
    content.append(&gtk::Separator::new(gtk::Orientation::Horizontal));
    content.append(&status);
    stack.add_named(&content, Some("document"));
    stack.set_visible_child_name("empty");
    toolbar.set_content(Some(&stack));
    window.set_content(Some(&toolbar));

    let viewer = Rc::new(Viewer {
        window: window.downgrade(),
        title,
        stack,
        tree_stack,
        search,
        kind,
        selection,
        text,
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
    let search_action = gio::SimpleAction::new("search", None);
    search_action.connect_activate(glib::clone!(
        #[weak]
        viewer,
        move |_, _| {
            viewer.search.grab_focus();
        }
    ));
    window.add_action(&search_action);
    viewer.search.connect_search_changed(glib::clone!(
        #[weak]
        viewer,
        move |_| viewer.rebuild_tree()
    ));
    viewer.kind.connect_selected_notify(glib::clone!(
        #[weak]
        viewer,
        move |_| viewer.rebuild_tree()
    ));
    viewer.selection.connect_selected_item_notify(glib::clone!(
        #[weak]
        viewer,
        move |_| viewer.show_selection()
    ));
    copy.connect_clicked(glib::clone!(
        #[weak]
        viewer,
        move |_| {
            let buffer = viewer.text.buffer();
            viewer.text.clipboard().set_text(&buffer.text(
                &buffer.start_iter(),
                &buffer.end_iter(),
                false,
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
                        viewer.search.set_text("");
                        viewer.kind.set_selected(0);
                        viewer.stack.set_visible_child_name("document");
                        viewer.rebuild_tree();
                    }
                    Err(message) => viewer.show_error(&message),
                }
            }
        ));
    }

    fn rebuild_tree(&self) {
        let document = self.document.borrow();
        let Some(document) = document.as_ref() else {
            return;
        };
        let query = self.search.text();
        let kind = self
            .kind
            .selected()
            .checked_sub(1)
            .and_then(|index| KINDS.get(index as usize))
            .copied();
        let root = document.filter(&query, kind);
        let has_matches = root.is_some();
        let roots = node_store(root);
        let model = gtk::TreeListModel::new(
            roots,
            false,
            !query.trim().is_empty() || kind.is_some(),
            |object| {
                let object = object.downcast_ref::<glib::BoxedAnyObject>()?;
                let node = object.borrow::<Arc<Node>>();
                if node.children.is_empty() {
                    None
                } else {
                    Some(node_store(node.children.iter().cloned()).upcast())
                }
            },
        );
        self.selection.set_model(Some(&model));
        if let Some(row) = model.row(0) {
            row.set_expanded(true);
        }
        self.tree_stack
            .set_visible_child_name(if has_matches { "tree" } else { "no-matches" });
        let root = document.root();
        let summary = if document.format() == Format::JsonLines {
            format!("JSON Lines · {} records", root.children.len())
        } else {
            "JSON".into()
        };
        self.status.set_text(&format!(
            "{summary} · Read only{}",
            if has_matches {
                ""
            } else {
                " · No matching values"
            }
        ));
    }

    fn show_selection(&self) {
        let Some(row) = self
            .selection
            .selected_item()
            .and_downcast::<gtk::TreeListRow>()
        else {
            self.text.buffer().set_text("");
            self.pointer.set_text("No selection");
            return;
        };
        let Some(object) = row.item().and_downcast::<glib::BoxedAnyObject>() else {
            return;
        };
        let node = object.borrow::<Arc<Node>>();
        let document = self.document.borrow();
        let Some(value) = document
            .as_ref()
            .and_then(|document| document.value_at(&node.pointer))
        else {
            return;
        };
        match serde_json::to_string_pretty(value) {
            Ok(text) => self.text.buffer().set_text(&text),
            Err(error) => {
                self.show_error(&error.to_string());
                return;
            }
        }
        self.pointer.set_text(if node.pointer.is_empty() {
            "$ (document root)"
        } else {
            &node.pointer
        });
        self.pointer.set_tooltip_text(Some(&node.pointer));
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

fn node_store(nodes: impl IntoIterator<Item = Arc<Node>>) -> gio::ListStore {
    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    for node in nodes {
        store.append(&glib::BoxedAnyObject::new(node));
    }
    store
}

fn tree_factory() -> gtk::SignalListItemFactory {
    let factory = gtk::SignalListItemFactory::new();
    factory.connect_setup(|_, object| {
        let Some(item) = object.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        row.set_margin_top(6);
        row.set_margin_bottom(6);
        let key = gtk::Label::builder()
            .xalign(0.0)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .width_chars(12)
            .build();
        let value = gtk::Label::builder()
            .xalign(0.0)
            .hexpand(true)
            .ellipsize(gtk::pango::EllipsizeMode::End)
            .css_classes(["dim-label"])
            .build();
        row.append(&key);
        row.append(&value);
        let expander = gtk::TreeExpander::new();
        expander.set_child(Some(&row));
        item.set_child(Some(&expander));
    });
    factory.connect_bind(|_, object| {
        let Some(item) = object.downcast_ref::<gtk::ListItem>() else {
            return;
        };
        let Some(expander) = item.child().and_downcast::<gtk::TreeExpander>() else {
            return;
        };
        let Some(row) = item.item().and_downcast::<gtk::TreeListRow>() else {
            return;
        };
        let Some(object) = row.item().and_downcast::<glib::BoxedAnyObject>() else {
            return;
        };
        let node = object.borrow::<Arc<Node>>();
        expander.set_list_row(Some(&row));
        expander.set_tooltip_text(Some(&format!("{} · {:?}", node.pointer, node.kind)));
        let Some(labels) = expander.child() else {
            return;
        };
        if let Some(key) = labels.first_child().and_downcast::<gtk::Label>() {
            key.set_text(&node.label);
        }
        if let Some(value) = labels.last_child().and_downcast::<gtk::Label>() {
            value.set_text(&node.preview);
        }
    });
    factory.connect_unbind(|_, object| {
        if let Some(expander) = object
            .downcast_ref::<gtk::ListItem>()
            .and_then(|item| item.child())
            .and_downcast::<gtk::TreeExpander>()
        {
            expander.set_list_row(None::<&gtk::TreeListRow>);
        }
    });
    factory
}
