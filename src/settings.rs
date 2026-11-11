//! Settings window: appearance/themes, editor, launcher.

use crate::theme::{self, Theme};
use crate::Workspace;
use gtk::prelude::*;
use std::rc::Rc;

impl Workspace {
    pub(crate) fn open_settings(self: &Rc<Self>) {
        // Rebuild from scratch on every open. Reusing the old window left it
        // hidden or stacked behind the main window, so the button looked dead.
        if let Some(window) = self.settings_window.borrow_mut().take() {
            window.destroy();
        }

        let window = gtk::Window::builder()
            .title(format!("{} — Settings", self.theme_store.borrow().app_name))
            .transient_for(&self.window)
            .default_width(560)
            .default_height(680)
            .build();
        window.add_css_class("settings-window");

        let header = gtk::Overlay::new();
        header.add_css_class("settings-titlebar");
        let title_block = gtk::Box::new(gtk::Orientation::Vertical, 1);
        title_block.set_valign(gtk::Align::Center);
        let title = gtk::Label::new(Some("Settings"));
        title.add_css_class("settings-header-title");
        title.set_xalign(0.0);
        let subtitle = gtk::Label::new(Some("Appearance, editor and launcher"));
        subtitle.add_css_class("settings-header-subtitle");
        subtitle.set_xalign(0.0);
        title_block.append(&title);
        title_block.append(&subtitle);
        let handle = gtk::WindowHandle::new();
        handle.set_child(Some(&title_block));
        handle.set_margin_start(16);
        handle.set_margin_end(8);
        handle.set_halign(gtk::Align::Start);
        header.set_child(Some(&handle));
        let done = gtk::Button::with_label("Done");
        done.add_css_class("settings-done");
        done.set_halign(gtk::Align::End);
        done.set_valign(gtk::Align::Center);
        done.set_margin_end(16);
        let window_for_done = window.clone();
        done.connect_clicked(move |_| window_for_done.destroy());
        header.add_overlay(&done);
        window.set_titlebar(Some(&header));

        let body = gtk::Box::new(gtk::Orientation::Vertical, 12);
        body.add_css_class("settings-body");
        body.set_margin_top(16);
        body.set_margin_bottom(20);
        body.set_margin_start(18);
        body.set_margin_end(18);

        // ------------------------------------------------------------ appearance
        body.append(&label("APPEARANCE"));
        let themes_card = card();
        let themes_list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        themes_list.set_margin_top(4);
        themes_list.set_margin_bottom(4);
        fill_themes(self, &themes_list);
        themes_card.append(&themes_list);
        body.append(&themes_card);

        // typography
        let type_card = card();
        let current = self.theme_store.borrow().active.clone();
        type_card.append(&entry_row(
            "UI font",
            "Any installed font family",
            &current.font_family,
            {
                let weak = Rc::downgrade(self);
                move |value| {
                    if let Some(ws) = weak.upgrade() {
                        update_active(&ws, |theme| theme.font_family = value.clone());
                    }
                }
            },
        ));
        type_card.append(&divider());
        type_card.append(&entry_row(
            "Code font",
            "Editor and console",
            &current.mono_family,
            {
                let weak = Rc::downgrade(self);
                move |value| {
                    if let Some(ws) = weak.upgrade() {
                        update_active(&ws, |theme| theme.mono_family = value.clone());
                    }
                }
            },
        ));
        type_card.append(&divider());

        let size_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        size_row.add_css_class("settings-card-row");
        let size_text = gtk::Box::new(gtk::Orientation::Vertical, 1);
        size_text.set_hexpand(true);
        size_text.append(&key("Font size"));
        size_text.append(&hint("9 – 20 px, applies to the whole UI"));
        size_row.append(&size_text);
        let size_spin = gtk::SpinButton::with_range(9.0, 20.0, 1.0);
        size_spin.set_value(current.font_size as f64);
        size_spin.set_valign(gtk::Align::Center);
        size_spin.set_width_chars(3);
        let weak = Rc::downgrade(self);
        size_spin.connect_value_changed(move |spin| {
            if let Some(ws) = weak.upgrade() {
                let value = spin.value() as i32;
                update_active(&ws, |theme| theme.font_size = value);
            }
        });
        size_row.append(&size_spin);
        type_card.append(&size_row);
        body.append(&type_card);

        // ---------------------------------------------------------------- editor
        body.append(&label("EDITOR"));
        let editor_card = card();
        let auto_row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
        auto_row.add_css_class("settings-card-row");
        let auto_text = gtk::Box::new(gtk::Orientation::Vertical, 1);
        auto_text.set_hexpand(true);
        auto_text.append(&key("Autocomplete"));
        auto_text.append(&hint("Suggest Luau names while typing"));
        auto_row.append(&auto_text);
        let auto_switch = gtk::Switch::new();
        auto_switch.add_css_class("settings-switch");
        auto_switch.set_active(self.completion_enabled.get());
        auto_switch.set_valign(gtk::Align::Center);
        let weak = Rc::downgrade(self);
        auto_switch.connect_state_set(move |_, active| {
            if let Some(ws) = weak.upgrade() {
                ws.completion_enabled.set(active);
                {
                    let mut prefs = ws.prefs.borrow_mut();
                    prefs.autocomplete = active;
                    prefs.save();
                }
                if !active {
                    for tab in ws.tabs.borrow().iter() {
                        tab.completion.hide_now();
                    }
                }
            }
            gtk::glib::Propagation::Proceed
        });
        auto_row.append(&auto_switch);
        editor_card.append(&auto_row);
        body.append(&editor_card);

        // -------------------------------------------------------------- launcher
        body.append(&label("LAUNCHER"));
        let launcher_card = card();
        let prefs = self.prefs.borrow().clone();

        let launcher_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        launcher_row.add_css_class("settings-card-row");
        let launcher_text = gtk::Box::new(gtk::Orientation::Vertical, 1);
        launcher_text.set_hexpand(true);
        launcher_text.append(&key("Relapse launcher"));
        launcher_text.append(&hint("Used by the Launch button"));
        launcher_row.append(&launcher_text);
        let browse = gtk::Button::with_label("Browse…");
        browse.add_css_class("settings-inline-button");
        launcher_row.append(&browse);
        launcher_card.append(&launcher_row);

        let launcher_entry = gtk::Entry::new();
        launcher_entry.set_text(&prefs.launcher);
        launcher_entry.set_placeholder_text(Some("/path/to/relapse-launcher"));
        launcher_entry.set_margin_start(14);
        launcher_entry.set_margin_end(14);
        launcher_entry.set_margin_bottom(10);
        let weak = Rc::downgrade(self);
        launcher_entry.connect_changed(move |entry| {
            if let Some(ws) = weak.upgrade() {
                let mut prefs = ws.prefs.borrow_mut();
                prefs.launcher = entry.text().to_string();
                prefs.save();
            }
        });
        launcher_card.append(&launcher_entry);

        let weak = Rc::downgrade(self);
        let entry_for_browse = launcher_entry.clone();
        browse.connect_clicked(move |button| {
            let Some(ws) = weak.upgrade() else { return };
            let dialog = gtk::FileDialog::builder()
                .title("Choose the Relapse launcher")
                .modal(true)
                .build();
            let entry = entry_for_browse.clone();
            dialog.open(
                Some(&ws.window),
                None::<&gtk::gio::Cancellable>,
                move |result| {
                    if let Ok(file) = result {
                        if let Some(path) = file.path() {
                            entry.set_text(&path.display().to_string());
                        }
                    }
                    let _ = button;
                },
            );
        });

        launcher_card.append(&divider());
        launcher_card.append(&entry_row(
            "Profile",
            "Profile passed to the launcher",
            &prefs.profile,
            {
                let weak = Rc::downgrade(self);
                move |value| {
                    if let Some(ws) = weak.upgrade() {
                        let mut prefs = ws.prefs.borrow_mut();
                        prefs.profile = value.clone();
                        prefs.save();
                    }
                }
            },
        ));

        let launch_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        launch_row.add_css_class("settings-card-row");
        let launch_hint = hint("Starts the chosen profile in the background.");
        launch_hint.set_hexpand(true);
        launch_row.append(&launch_hint);
        let launch_now = gtk::Button::with_label("Launch now");
        launch_now.add_css_class("settings-inline-button");
        let weak = Rc::downgrade(self);
        launch_now.connect_clicked(move |_| {
            if let Some(ws) = weak.upgrade() {
                ws.launch_profile();
            }
        });
        launch_row.append(&launch_now);
        launcher_card.append(&launch_row);
        body.append(&launcher_card);

        let footer = gtk::Label::new(Some(
            "Themes are saved in ~/.config/relapse-ui/themes.json and apply instantly.",
        ));
        footer.add_css_class("settings-footer");
        footer.set_xalign(0.0);
        footer.set_wrap(true);
        body.append(&footer);

        let scroll = gtk::ScrolledWindow::new();
        scroll.add_css_class("settings-scroll");
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_child(Some(&body));
        window.set_child(Some(&scroll));

        self.settings_window.replace(Some(window.clone()));
        let weak = Rc::downgrade(self);
        window.connect_destroy(move |_| {
            if let Some(ws) = weak.upgrade() {
                ws.settings_window.replace(None);
            }
        });
        window.present();
    }
}

fn update_active(ws: &Rc<Workspace>, edit: impl FnOnce(&mut Theme)) {
    {
        let mut store = ws.theme_store.borrow_mut();
        edit(&mut store.active);
        store.save();
    }
    let active = ws.theme_store.borrow().active.clone();
    ws.apply_theme(&active);
}

fn fill_themes(ws: &Rc<Workspace>, list: &gtk::Box) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    let active_name = ws.theme_store.borrow().active.name.clone();
    let saved = ws.theme_store.borrow().themes.clone();
    let mut entries = theme::presets();
    entries.extend(saved);
    let mut first = true;
    for entry in entries {
        if !first {
            list.append(&divider());
        }
        first = false;
        let is_active = entry.name == active_name;
        let deletable = ws
            .theme_store
            .borrow()
            .themes
            .iter()
            .any(|item| item.id == entry.id);
        list.append(&theme_row(ws, list, entry, is_active, deletable));
    }
}

fn theme_row(
    ws: &Rc<Workspace>,
    list: &gtk::Box,
    theme: Theme,
    active: bool,
    deletable: bool,
) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("settings-card-row");
    let swatches = gtk::Label::new(None);
    swatches.add_css_class("theme-swatches");
    swatches.set_markup(&swatches_markup(&theme));
    row.append(&swatches);
    let name = gtk::Label::new(Some(&theme.name));
    name.add_css_class("settings-value-title");
    name.set_xalign(0.0);
    name.set_hexpand(true);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    row.append(&name);

    if active {
        let mark = gtk::Label::new(Some("In use"));
        mark.add_css_class("theme-row-active");
        row.append(&mark);
    } else {
        let use_button = gtk::Button::with_label("Use");
        use_button.add_css_class("settings-inline-button");
        let weak = Rc::downgrade(ws);
        let list_for_use = list.clone();
        let chosen = theme.clone();
        use_button.connect_clicked(move |_| {
            if let Some(ws) = weak.upgrade() {
                ws.use_theme(chosen.clone());
                fill_themes(&ws, &list_for_use);
            }
        });
        row.append(&use_button);
    }

    if deletable {
        let delete = gtk::Button::with_label("Delete");
        delete.add_css_class("settings-inline-button");
        let weak = Rc::downgrade(ws);
        let list_for_delete = list.clone();
        let id = theme.id.clone();
        delete.connect_clicked(move |_| {
            if let Some(ws) = weak.upgrade() {
                ws.theme_store.borrow_mut().delete(&id);
                let active = ws.theme_store.borrow().active.clone();
                ws.apply_theme(&active);
                fill_themes(&ws, &list_for_delete);
            }
        });
        row.append(&delete);
    }
    row
}

fn swatches_markup(theme: &Theme) -> String {
    [
        &theme.background,
        &theme.surface_alt,
        &theme.accent,
        &theme.success,
        &theme.error,
    ]
    .iter()
    .map(|color| format!("<span foreground=\"{color}\">●</span>"))
    .collect::<Vec<_>>()
    .join("  ")
}

fn entry_row(
    label_text: &str,
    hint_text: &str,
    value: &str,
    on_change: impl Fn(&String) + 'static,
) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    row.add_css_class("settings-card-row");
    let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
    text.set_hexpand(true);
    text.append(&key(label_text));
    text.append(&hint(hint_text));
    row.append(&text);
    let entry = gtk::Entry::new();
    entry.set_text(value);
    entry.set_width_chars(18);
    entry.set_valign(gtk::Align::Center);
    entry.connect_changed(move |entry| on_change(&entry.text().to_string()));
    row.append(&entry);
    row
}

fn card() -> gtk::Box {
    let card = gtk::Box::new(gtk::Orientation::Vertical, 0);
    card.add_css_class("settings-card");
    card
}

fn divider() -> gtk::Box {
    let line = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    line.add_css_class("settings-divider");
    line.set_margin_start(12);
    line.set_margin_end(12);
    line
}

fn label(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("settings-label");
    label.set_xalign(0.0);
    label
}

fn key(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("settings-key");
    label.set_xalign(0.0);
    label
}

fn hint(text: &str) -> gtk::Label {
    let label = gtk::Label::new(Some(text));
    label.add_css_class("settings-hint");
    label.set_xalign(0.0);
    label
}
