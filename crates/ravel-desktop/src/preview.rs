use libadwaita as adw;
use sourceview5::{self as sourceview, prelude::*};

pub fn create() -> sourceview::View {
    let buffer = sourceview::Buffer::new(None);
    buffer.set_language(
        sourceview::LanguageManager::default()
            .language("json")
            .as_ref(),
    );
    buffer.set_highlight_syntax(true);
    buffer.set_highlight_matching_brackets(false);

    // GObject bindings hold weak references and detach when the buffer is destroyed.
    adw::StyleManager::default()
        .bind_property("dark", &buffer, "style-scheme")
        .transform_to(|_, dark: bool| {
            let schemes = sourceview::StyleSchemeManager::default();
            schemes
                .scheme(if dark { "Adwaita-dark" } else { "Adwaita" })
                .or_else(|| schemes.scheme(if dark { "oblivion" } else { "classic" }))
        })
        .sync_create()
        .build();

    let view = sourceview::View::builder()
        .buffer(&buffer)
        .editable(false)
        .monospace(true)
        .cursor_visible(false)
        .left_margin(16)
        .right_margin(16)
        .top_margin(12)
        .bottom_margin(12)
        .wrap_mode(gtk::WrapMode::None)
        .build();
    view.connect_copy_clipboard(|view| {
        let buffer = view.buffer();
        if let Some((start, end)) = buffer.selection_bounds() {
            view.clipboard().set_text(&buffer.text(&start, &end, true));
        }
        view.stop_signal_emission_by_name("copy-clipboard");
    });
    view
}
