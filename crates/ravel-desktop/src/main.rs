mod fold_index;
mod folding;
mod key_hover;
mod key_index;
mod preview;
mod window;

#[cfg(test)]
mod fold_tests;
#[cfg(test)]
mod hover_tests;
#[cfg(test)]
mod ui_tests;

use gtk::{gio, glib};
use libadwaita::{self as adw, prelude::*};

fn main() -> glib::ExitCode {
    let app = adw::Application::builder()
        .application_id("io.github.ravel.Ravel")
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .build();

    app.connect_activate(|app| {
        if let Some(window) = app.active_window() {
            window.present();
        } else {
            window::create(app, None);
        }
    });
    app.connect_open(|app, files, _| {
        for file in files {
            window::create(app, Some(file.clone()));
        }
    });
    let quit = gio::SimpleAction::new("quit", None);
    quit.connect_activate(glib::clone!(
        #[weak]
        app,
        move |_, _| app.quit()
    ));
    app.add_action(&quit);
    app.set_accels_for_action("win.open", &["<Control>o"]);
    app.set_accels_for_action("win.search", &["<Control>f"]);
    app.set_accels_for_action("window.close", &["<Control>w"]);
    app.set_accels_for_action("app.quit", &["<Control>q"]);
    app.run()
}
