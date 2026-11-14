use gtk::prelude::*;
use tauri::Manager;

pub fn configure(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let overlay = app
        .get_webview_window("overlay")
        .ok_or("Missing recording overlay")?;
    overlay.set_background_color(Some(tauri::utils::config::Color(0, 0, 0, 0)))?;
    let native = overlay.gtk_window()?;
    native.set_accept_focus(false);
    native.set_focus_on_map(false);
    let css = gtk::CssProvider::new();
    css.load_from_data(b"window { background: transparent; border: none; box-shadow: none; }")?;
    native
        .style_context()
        .add_provider(&css, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);

    // Apply an empty Wayland input region only after GDK has created the surface.
    // Unlike cursor-ignore before realization, this also works on the first show.
    native.connect_realize(make_click_through);
    native.connect_map(make_click_through);
    if native.is_realized() {
        make_click_through(&native);
    }
    Ok(())
}

fn make_click_through(window: &gtk::ApplicationWindow) {
    if let Some(surface) = window.window() {
        surface.input_shape_combine_region(&gtk::cairo::Region::create(), 0, 0);
    }
}
