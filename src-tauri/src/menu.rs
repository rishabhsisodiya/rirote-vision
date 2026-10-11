//! The menu bar. Every command the window offers is here too; a chosen item
//! reaches the interface as a `menu` event carrying the item's id.

use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Runtime};

pub const EVENT: &str = "menu";

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let item = |id: &str, text: &str, keys: Option<&str>| {
        let builder = MenuItemBuilder::with_id(id, text);
        match keys {
            Some(keys) => builder.accelerator(keys).build(app),
            None => builder.build(app),
        }
    };

    let app_menu = SubmenuBuilder::new(app, "RIROTE VISION")
        .about(None)
        .separator()
        .item(&item("setup.check", "Check Setup…", None)?)
        .separator()
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator()
        .quit()
        .build()?;

    let file = SubmenuBuilder::new(app, "File")
        .item(&item("project.new", "New Project…", Some("CmdOrCtrl+N"))?)
        .item(&item("project.open", "Open Project…", Some("CmdOrCtrl+O"))?)
        .separator()
        .item(&item("project.export", "Export…", Some("CmdOrCtrl+E"))?)
        .item(&item("project.reveal", "Show Project in Finder", Some("CmdOrCtrl+Shift+R"))?)
        .item(&item("project.close", "Close Project", Some("CmdOrCtrl+Shift+W"))?)
        .separator()
        .close_window()
        .build()?;

    // Undo and redo are the app's own items: they act on the timeline, and
    // the window passes them on to a text field when one has the focus.
    let edit = SubmenuBuilder::new(app, "Edit")
        .item(&item("edit.undo", "Undo", Some("CmdOrCtrl+Z"))?)
        .item(&item("edit.redo", "Redo", Some("CmdOrCtrl+Shift+Z"))?)
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    let clip = SubmenuBuilder::new(app, "Clip")
        .item(&item("clip.split", "Split at Playhead", Some("CmdOrCtrl+B"))?)
        .item(&item("clip.duplicate", "Duplicate", Some("CmdOrCtrl+D"))?)
        .item(&item("clip.delete", "Delete", None)?)
        .build()?;

    let view = SubmenuBuilder::new(app, "View")
        .item(&item("view.sidebar", "Show or Hide Project Sidebar", Some("CmdOrCtrl+0"))?)
        .item(&item("view.assistant", "Show or Hide Assistant", Some("CmdOrCtrl+Alt+0"))?)
        .separator()
        .fullscreen()
        .build()?;

    let window = SubmenuBuilder::new(app, "Window").minimize().maximize().build()?;

    let help = SubmenuBuilder::new(app, "Help")
        .item(&item("help.shortcuts", "Keyboard Shortcuts", Some("CmdOrCtrl+/"))?)
        .build()?;

    MenuBuilder::new(app).items(&[&app_menu, &file, &edit, &clip, &view, &window, &help]).build()
}

pub fn install<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    app.set_menu(build(app)?)?;
    app.on_menu_event(|app, event| {
        let _ = app.emit(EVENT, event.id().as_ref());
    });
    Ok(())
}
