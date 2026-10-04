//! jq bridge: operation catalog, per-node context menus, and result dialogs.

use gtk::{gdk, gio, glib};
use libadwaita::{self as adw, prelude::*};
use ravel_core::Kind;

/// A named jq filter applicable to a single JSON input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Op {
    pub label: &'static str,
    pub program: &'static str,
}

/// Common jq operations per node type.
pub fn operations(kind: Kind) -> Vec<Op> {
    let common = [Op {
        label: "Type",
        program: "type",
    }];
    let specific: &[Op] = match kind {
        Kind::Object => &[
            Op {
                label: "Keys",
                program: "keys",
            },
            Op {
                label: "Values",
                program: "[.[]]",
            },
            Op {
                label: "Length",
                program: "length",
            },
            Op {
                label: "To Entries",
                program: "to_entries",
            },
        ],
        Kind::Array => &[
            Op {
                label: "Length",
                program: "length",
            },
            Op {
                label: "First",
                program: "first",
            },
            Op {
                label: "Last",
                program: "last",
            },
            Op {
                label: "Add",
                program: "add",
            },
            Op {
                label: "Unique",
                program: "unique",
            },
            Op {
                label: "Sort",
                program: "sort",
            },
            Op {
                label: "Reverse",
                program: "reverse",
            },
            Op {
                label: "Flatten",
                program: "flatten",
            },
            Op {
                label: "Min",
                program: "min",
            },
            Op {
                label: "Max",
                program: "max",
            },
            Op {
                label: "Join (\", \")",
                program: "join(\", \")",
            },
        ],
        Kind::String => &[
            Op {
                label: "Length",
                program: "length",
            },
            Op {
                label: "Uppercase",
                program: "ascii_upcase",
            },
            Op {
                label: "Lowercase",
                program: "ascii_downcase",
            },
            Op {
                label: "To Number",
                program: "tonumber",
            },
        ],
        Kind::Number => &[Op {
            label: "To String",
            program: "tostring",
        }],
        Kind::Boolean | Kind::Null => &[],
    };
    common.iter().chain(specific).copied().collect()
}

/// Builds the right-click menu for a node: copies plus jq operations.
pub fn context_menu(kind: Kind, pointer: &str) -> gio::Menu {
    let menu = gio::Menu::new();
    let node = gio::Menu::new();
    let copy_item = gio::MenuItem::new(Some("Copy JSON"), None);
    copy_item.set_action_and_target_value(Some("win.node-copy"), Some(&pointer.to_variant()));
    node.append_item(&copy_item);
    let pointer_item = gio::MenuItem::new(Some("Copy JSON Pointer"), None);
    pointer_item.set_action_and_target_value(Some("win.node-pointer"), Some(&pointer.to_variant()));
    node.append_item(&pointer_item);
    menu.append_section(Some("Node"), &node);

    let jq = gio::Menu::new();
    for op in operations(kind) {
        let item = gio::MenuItem::new(Some(op.label), None);
        let target = (pointer, op.program, op.label).to_variant();
        item.set_action_and_target_value(Some("win.node-jq"), Some(&target));
        jq.append_item(&item);
    }
    menu.append_section(Some("jq"), &jq);
    menu
}

/// Attaches a secondary-click context menu to a structured-view node row.
pub fn attach_menu(widget: &impl IsA<gtk::Widget>, kind: Kind, pointer: &str) {
    let pointer = pointer.to_owned();
    let keys = gtk::EventControllerKey::new();
    let menu_widget = widget.upcast_ref::<gtk::Widget>().downgrade();
    let key_pointer = pointer.clone();
    keys.connect_key_pressed(move |_, key, _, modifiers| {
        if (key == gdk::Key::Menu && modifiers.is_empty())
            || (key == gdk::Key::F10 && modifiers == gdk::ModifierType::SHIFT_MASK)
        {
            if let Some(widget) = menu_widget.upgrade() {
                popup(&widget, kind, &key_pointer, 0, widget.height());
            }
            glib::Propagation::Stop
        } else {
            glib::Propagation::Proceed
        }
    });
    widget.add_controller(keys);
    let gesture = gtk::GestureClick::new();
    gesture.set_button(gdk::BUTTON_SECONDARY);
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
    let menu_widget = widget.upcast_ref::<gtk::Widget>().downgrade();
    gesture.connect_pressed(move |gesture, _, x, y| {
        gesture.set_state(gtk::EventSequenceState::Claimed);
        let Some(menu_widget) = menu_widget.upgrade() else {
            return;
        };
        popup(&menu_widget, kind, &pointer, x as i32, y as i32);
    });
    widget.add_controller(gesture);
}

fn popup(widget: &gtk::Widget, kind: Kind, pointer: &str, x: i32, y: i32) {
    crate::tree_navigation::focus(widget);
    let popover = gtk::PopoverMenu::from_model(Some(&context_menu(kind, pointer)));
    popover.set_has_arrow(false);
    popover.set_pointing_to(Some(&gdk::Rectangle::new(x, y, 1, 1)));
    popover.set_parent(widget);
    popover.popup();
    popover.connect_closed(move |popover| {
        // Action lookup needs the ancestry until menu dispatch completes.
        let popover = popover.clone();
        glib::idle_add_local_once(move || popover.unparent());
    });
}

/// Runs `jq program` with `input` on stdin, returning stdout or jq's error.
pub async fn run(program: &str, input: &str) -> Result<String, String> {
    let argv: [&std::ffi::OsStr; 3] = ["jq".as_ref(), "--".as_ref(), program.as_ref()];
    let subprocess = gio::Subprocess::newv(
        &argv,
        gio::SubprocessFlags::STDIN_PIPE
            | gio::SubprocessFlags::STDOUT_PIPE
            | gio::SubprocessFlags::STDERR_PIPE,
    )
    .map_err(|error| format!("Could not start jq: {error}. Is jq installed?"))?;
    let (stdout, stderr) = subprocess
        .communicate_future(Some(&glib::Bytes::from(input.as_bytes())))
        .await
        .map_err(|error| format!("Running jq failed: {error}"))?;
    let stderr = stderr
        .map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
        .unwrap_or_default();
    if subprocess.status() != 0 {
        return Err(if stderr.is_empty() {
            format!("jq exited with status {}", subprocess.status())
        } else {
            stderr
        });
    }
    stdout
        .map(|bytes| String::from_utf8_lossy(&bytes).trim_end().to_owned())
        .ok_or_else(|| "jq produced no output".to_owned())
}

/// Presents jq output in a read-only dialog with syntax highlighting.
pub fn show_result(parent: &impl IsA<gtk::Window>, title: &str, output: &str) {
    let view = crate::preview::create();
    view.buffer().set_text(output);
    let scrolled = gtk::ScrolledWindow::builder()
        .child(&view)
        .vexpand(true)
        .hexpand(true)
        .min_content_width(520)
        .min_content_height(380)
        .build();
    let toolbar = adw::ToolbarView::new();
    let header = adw::HeaderBar::new();
    let copy = gtk::Button::builder()
        .icon_name("edit-copy-symbolic")
        .tooltip_text("Copy result")
        .css_classes(["flat"])
        .build();
    copy.connect_clicked(glib::clone!(
        #[weak]
        view,
        #[to_owned]
        output,
        move |_| view.clipboard().set_text(&output)
    ));
    header.pack_end(&copy);
    toolbar.add_top_bar(&header);
    toolbar.set_content(Some(&scrolled));
    let dialog = adw::Dialog::builder().title(title).child(&toolbar).build();
    dialog.present(Some(parent.as_ref()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jq_runs_filters_and_reports_errors() {
        let context = glib::MainContext::new();
        context.block_on(async {
            assert_eq!(
                run("sort", "[3,1,2]").await.unwrap(),
                "[\n  1,\n  2,\n  3\n]"
            );
            assert_eq!(
                run("[.[]]", "{\"a\":1,\"b\":2}").await.unwrap(),
                "[\n  1,\n  2\n]"
            );
            assert!(run("tonumber", "\"oops\"").await.is_err());
        });
    }

    #[test]
    fn objects_offer_catalog_and_key_operations() {
        let labels: Vec<_> = operations(Kind::Object).iter().map(|op| op.label).collect();
        assert!(labels.contains(&"Keys"));
        assert!(labels.contains(&"Type"));
        assert_eq!(labels.iter().filter(|label| **label == "Sort").count(), 0);
    }

    #[test]
    fn arrays_offer_aggregation_operations() {
        let programs: Vec<_> = operations(Kind::Array)
            .iter()
            .map(|op| op.program)
            .collect();
        assert!(programs.contains(&"sort"));
        assert!(programs.contains(&"unique"));
        assert!(programs.contains(&"flatten"));
    }

    #[test]
    fn strings_and_numbers_offer_conversions() {
        assert!(
            operations(Kind::String)
                .iter()
                .any(|op| op.program == "tonumber")
        );
        assert!(
            operations(Kind::Number)
                .iter()
                .any(|op| op.program == "tostring")
        );
    }

    #[test]
    fn context_menu_targets_carry_pointer_and_program() {
        let menu = context_menu(Kind::Array, "/users/0");
        assert_eq!(menu.n_items(), 2); // Node and jq sections.
        let node = menu.item_link(0, "section").unwrap();
        assert_eq!(
            node.item_attribute_value(0, "action", None).unwrap().str(),
            Some("win.node-copy")
        );
        assert_eq!(
            node.item_attribute_value(0, "target", None).unwrap().str(),
            Some("/users/0")
        );
        let jq = menu.item_link(1, "section").unwrap();
        assert_eq!(
            jq.item_attribute_value(0, "target", None)
                .unwrap()
                .get::<(String, String, String)>(),
            Some(("/users/0".into(), "type".into(), "Type".into()))
        );
    }
}
