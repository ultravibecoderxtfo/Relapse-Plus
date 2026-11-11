//! Client Manager: accounts from Relapse profiles, multi-launch and cookies.

use crate::Workspace;
use gtk::glib;
use gtk::prelude::*;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

struct Account {
    profile: String,
    user_id: Option<u64>,
    username: Option<String>,
    display_name: Option<String>,
    avatar: Option<Vec<u8>>,
    saved_cookie: bool,
}

fn profiles_dir() -> PathBuf {
    PathBuf::from(concat!(
        env!("HOME"),
        "/.local/share/relapse/profiles"
    ))
}

fn launcher(ws: &Workspace) -> Option<PathBuf> {
    std::env::var_os("RELAPSE_LAUNCHER")
        .map(PathBuf::from)
        .or_else(|| {
            let path = ws.prefs.borrow().launcher.clone();
            if path.trim().is_empty() {
                None
            } else {
                Some(PathBuf::from(path))
            }
        })
}

fn spawn_profile(ws: &Rc<Workspace>, profile: &str, game: Option<&str>) -> Result<(), String> {
    let Some(launcher) = launcher(ws) else {
        return Err("Set the Relapse launcher in Settings → Launcher first.".to_string());
    };
    if !launcher.exists() {
        return Err(format!("Launcher not found: {}", launcher.display()));
    }
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        let portal = PathBuf::from(runtime).join("doc/by-app/org.vinegarhq.Sober");
        let _ = std::fs::create_dir_all(portal);
    }

    let dir = profiles_dir().join(profile);
    // Repair profiles created before the layout marker existed.
    let _ = ensure_profile_layout(&dir);
    // Repair cookie profiles the launcher would still show its onboarding for.
    seed_sober_state(&dir);
    let log_path = dir.join("ui-launch.log");
    let log = std::fs::File::create(&log_path)
        .map_err(|error| format!("Could not open the launch log: {error}"))?;
    let log_err = log
        .try_clone()
        .map_err(|error| format!("Could not open the launch log: {error}"))?;

    let mut command = std::process::Command::new(&launcher);
    command.args(["--profile", profile]);
    // Without this the launcher opens its own window and waits, which looked
    // like the Launch button doing nothing and added seconds to every start.
    command.arg("--no-launch-ui");
    if let Some(id) = game {
        command.args(["--", "--gameId", id]);
    }
    command
        // The launcher refuses to run with an inherited LD_LIBRARY_PATH, and the
        // app can inherit one from the shell or desktop session that started it.
        .env_remove("LD_LIBRARY_PATH")
        .stdout(std::process::Stdio::from(log))
        .stderr(std::process::Stdio::from(log_err));
    let child = command
        .spawn()
        .map_err(|error| format!("Could not start the launcher: {error}"))?;
    watch_launch(ws, profile.to_string(), Rc::new(RefCell::new(child)), log_path);
    Ok(())
}

/// Reports a launcher that dies immediately instead of failing silently.
fn watch_launch(
    ws: &Rc<Workspace>,
    profile: String,
    child: Rc<RefCell<std::process::Child>>,
    log_path: PathBuf,
) {
    let weak = Rc::downgrade(ws);
    let mut checks = 0;
    glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        checks += 1;
        let mut slot = child.borrow_mut();
        match slot.try_wait() {
            Ok(Some(status)) => {
                if !status.success() {
                    let raw = std::fs::read_to_string(&log_path).unwrap_or_default();
                    let lines: Vec<&str> = raw
                        .lines()
                        .map(str::trim)
                        .filter(|line| !line.is_empty())
                        .collect();
                    let detail = lines
                        .iter()
                        .rev()
                        .take(2)
                        .rev()
                        .copied()
                        .collect::<Vec<_>>()
                        .join(" · ");
                    let message = if detail.is_empty() {
                        format!("{profile} failed to start ({status}).")
                    } else {
                        format!("{profile} failed: {detail}")
                    };
                    if let Some(ws) = weak.upgrade() {
                        ws.toast(message);
                    }
                }
                glib::ControlFlow::Break
            }
            Ok(None) => {
                if checks >= 40 {
                    glib::ControlFlow::Break
                } else {
                    glib::ControlFlow::Continue
                }
            }
            Err(_) => glib::ControlFlow::Break,
        }
    });
}

fn scan_accounts() -> Vec<Account> {
    let mut accounts = Vec::new();
    let Ok(entries) = std::fs::read_dir(profiles_dir()) else {
        return accounts;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let profile = entry.file_name().to_string_lossy().to_string();
        let saved_cookie = path.join("cookie.txt").is_file();
        let data = std::fs::read_to_string(path.join("account.json"))
            .ok()
            .and_then(|raw| serde_json::from_str::<serde_json::Value>(&raw).ok());
        let account = data.and_then(|data| {
            let user_id = data.get("user_id").and_then(|value| value.as_u64())?;
            let username = data
                .get("username")
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .to_string();
            let display_name = data
                .get("display_name")
                .and_then(|value| value.as_str())
                .unwrap_or(&username)
                .to_string();
            let avatar = data
                .get("avatar_png_base64")
                .and_then(|value| value.as_str())
                .and_then(decode_base64);
            Some(Account {
                profile: profile.clone(),
                user_id: Some(user_id),
                username: Some(username),
                display_name: Some(display_name),
                avatar,
                saved_cookie,
            })
        });
        match account {
            Some(account) => accounts.push(account),
            // A profile that only has a pasted cookie still gets listed so it
            // can be launched; Refresh fills in the account after it signs in.
            None if saved_cookie => accounts.push(Account {
                profile,
                user_id: None,
                username: None,
                display_name: None,
                avatar: None,
                saved_cookie: true,
            }),
            None => {}
        }
    }
    let sort_key = |account: &Account| {
        account
            .display_name
            .clone()
            .or_else(|| account.username.clone())
            .unwrap_or_else(|| account.profile.clone())
            .to_lowercase()
    };
    accounts.sort_by(|left, right| sort_key(left).cmp(&sort_key(right)));
    accounts
}

impl Workspace {
    pub(crate) fn open_clients(self: &Rc<Self>) {
        // Rebuild from scratch on every open. Reusing the old window left it
        // hidden or stacked behind the main window, so the button looked dead.
        if let Some(window) = self.clients_window.borrow_mut().take() {
            window.destroy();
        }

        let window = gtk::Window::builder()
            .title("Client Manager — Relapse +")
            .transient_for(&self.window)
            .default_width(620)
            .default_height(700)
            .build();
        window.add_css_class("settings-window");

        let header = gtk::Overlay::new();
        header.add_css_class("settings-titlebar");
        let titles = gtk::Box::new(gtk::Orientation::Vertical, 1);
        titles.set_valign(gtk::Align::Center);
        let title = gtk::Label::new(Some("Client Manager"));
        title.add_css_class("settings-header-title");
        title.set_xalign(0.0);
        let subtitle = gtk::Label::new(Some("Accounts, profiles and multi-launch"));
        subtitle.add_css_class("settings-header-subtitle");
        subtitle.set_xalign(0.0);
        titles.append(&title);
        titles.append(&subtitle);
        let handle = gtk::WindowHandle::new();
        handle.set_child(Some(&titles));
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

        // ------------------------------------------------------------ new profile
        body.append(&label("NEW PROFILE"));
        let new_card = card();
        let new_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        new_row.add_css_class("settings-card-row");
        let new_text = gtk::Box::new(gtk::Orientation::Vertical, 1);
        new_text.set_hexpand(true);
        new_text.append(&key("Profile name"));
        new_text.append(&hint(
            "Name it, paste the .ROBLOSECURITY cookie — the account is checked automatically.",
        ));
        new_row.append(&new_text);
        let name_entry = gtk::Entry::new();
        name_entry.set_placeholder_text(Some("alt1"));
        name_entry.set_width_chars(12);
        new_row.append(&name_entry);
        let create = gtk::Button::with_label("Paste cookie");
        create.add_css_class("settings-inline-button");
        new_row.append(&create);
        new_card.append(&new_row);
        body.append(&new_card);

        // --------------------------------------------------------------- accounts
        let accounts_head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let accounts_label = label("ACCOUNTS");
        accounts_label.set_hexpand(true);
        accounts_head.append(&accounts_label);
        let refresh = gtk::Button::with_label("Refresh");
        refresh.add_css_class("settings-inline-button");
        accounts_head.append(&refresh);
        body.append(&accounts_head);

        let accounts_card = card();
        let list = gtk::Box::new(gtk::Orientation::Vertical, 0);
        list.set_margin_top(4);
        list.set_margin_bottom(4);
        accounts_card.append(&list);
        body.append(&accounts_card);

        let selection: Rc<RefCell<Vec<(gtk::CheckButton, String)>>> =
            Rc::new(RefCell::new(Vec::new()));
        fill_accounts(self, &list, &selection);

        let weak = Rc::downgrade(self);
        let list_for_refresh = list.clone();
        let selection_for_refresh = selection.clone();
        refresh.connect_clicked(move |_| {
            if let Some(ws) = weak.upgrade() {
                selection_for_refresh.borrow_mut().clear();
                fill_accounts(&ws, &list_for_refresh, &selection_for_refresh);
            }
        });

        let weak = Rc::downgrade(self);
        let entry_for_create = name_entry.clone();
        let list_for_create = list.clone();
        let selection_for_create = selection.clone();
        create.connect_clicked(move |_| {
            let Some(ws) = weak.upgrade() else { return };
            // Open the paste window even without a name; the name can be typed
            // in the dialog. Only saving requires one.
            let raw = entry_for_create.text().trim().to_string();
            cookie_dialog(&ws, &raw, &list_for_create, &selection_for_create);
        });

        // ---------------------------------------------------------- multi-launch
        body.append(&label("MULTI-LAUNCH"));
        let multi_card = card();
        let multi_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        multi_row.add_css_class("settings-card-row");
        let multi_text = gtk::Box::new(gtk::Orientation::Vertical, 1);
        multi_text.set_hexpand(true);
        multi_text.append(&key("Game ID (optional)"));
        multi_text.append(&hint("Passed through to Sober after --"));
        multi_row.append(&multi_text);
        let game_entry = gtk::Entry::new();
        game_entry.set_placeholder_text(Some("place id"));
        game_entry.set_width_chars(14);
        multi_row.append(&game_entry);
        let launch_selected = gtk::Button::with_label("Launch selected");
        launch_selected.add_css_class("settings-inline-button");
        multi_row.append(&launch_selected);
        multi_card.append(&multi_row);
        body.append(&multi_card);

        let weak = Rc::downgrade(self);
        let selection_for_launch = selection.clone();
        let game_for_launch = game_entry.clone();
        launch_selected.connect_clicked(move |_| {
            let Some(ws) = weak.upgrade() else { return };
            let chosen: Vec<String> = selection_for_launch
                .borrow()
                .iter()
                .filter(|(check, _)| check.is_active())
                .map(|(_, profile)| profile.clone())
                .collect();
            if chosen.is_empty() {
                ws.toast("Select at least one account.");
                return;
            }
            let game = game_for_launch.text().trim().to_string();
            let game = if game.is_empty() { None } else { Some(game) };
            let mut started = 0;
            for profile in &chosen {
                match spawn_profile(&ws, profile, game.as_deref()) {
                    Ok(()) => started += 1,
                    Err(error) => ws.toast(error),
                }
            }
            if started > 0 {
                ws.toast(format!("Launching {started} client(s)…"));
            }
        });

        let note = gtk::Label::new(Some(
            "Pasted cookies are stored per profile in ~/.local/share/relapse/profiles/<profile>/cookie.txt (mode 600) and applied to that profile's next launch.",
        ));
        note.add_css_class("settings-footer");
        note.set_xalign(0.0);
        note.set_wrap(true);
        body.append(&note);

        let scroll = gtk::ScrolledWindow::new();
        scroll.add_css_class("settings-scroll");
        scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroll.set_child(Some(&body));
        window.set_child(Some(&scroll));

        self.clients_window.replace(Some(window.clone()));
        let weak = Rc::downgrade(self);
        window.connect_destroy(move |_| {
            if let Some(ws) = weak.upgrade() {
                ws.clients_window.replace(None);
            }
        });
        window.present();

        if std::env::var_os("RELAPSE_TEST_COOKIE").is_some() {
            let ws = self.clone();
            let list_for_test = list.clone();
            let selection_for_test = selection.clone();
            glib::timeout_add_local_once(std::time::Duration::from_millis(250), move || {
                cookie_dialog(&ws, "test-profile", &list_for_test, &selection_for_test);
                let ws_check = ws.clone();
                glib::timeout_add_local_once(std::time::Duration::from_millis(400), move || {
                    let state = ws_check
                        .cookie_window
                        .borrow()
                        .as_ref()
                        .map(|dialog| {
                            (
                                dialog.is_visible(),
                                dialog.is_mapped(),
                                dialog.title().map(|title| title.to_string()),
                            )
                        });
                    eprintln!("TEST cookie window: {state:?}");
                    if let Some(dialog) = ws_check.cookie_window.borrow_mut().take() {
                        dialog.destroy();
                    }
                });
            });
        }
    }
}

fn fill_accounts(
    ws: &Rc<Workspace>,
    list: &gtk::Box,
    selection: &Rc<RefCell<Vec<(gtk::CheckButton, String)>>>,
) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }
    selection.borrow_mut().clear();
    let accounts = scan_accounts();
    if accounts.is_empty() {
        let empty = gtk::Label::new(Some(
            "No accounts yet. Type a profile name above, paste a cookie, then press Launch.",
        ));
        empty.add_css_class("settings-hint");
        empty.set_xalign(0.0);
        empty.set_wrap(true);
        empty.set_margin_start(12);
        empty.set_margin_end(12);
        empty.set_margin_top(10);
        empty.set_margin_bottom(10);
        list.append(&empty);
        return;
    }

    let mut first = true;
    for account in accounts {
        if !first {
            list.append(&divider());
        }
        first = false;
        list.append(&account_row(ws, list, selection, account));
    }
}

fn account_row(
    ws: &Rc<Workspace>,
    list: &gtk::Box,
    selection: &Rc<RefCell<Vec<(gtk::CheckButton, String)>>>,
    account: Account,
) -> gtk::Box {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 10);
    row.add_css_class("settings-card-row");
    row.add_css_class("client-row");

    let check = gtk::CheckButton::new();
    check.set_valign(gtk::Align::Center);
    check.set_tooltip_text(Some("Include in multi-launch"));
    row.append(&check);
    selection
        .borrow_mut()
        .push((check.clone(), account.profile.clone()));

    let avatar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
    avatar.add_css_class("client-avatar");
    avatar.set_valign(gtk::Align::Center);
    if let Some(data) = &account.avatar {
        let bytes = glib::Bytes::from_owned(data.clone());
        if let Ok(texture) = gtk::gdk::Texture::from_bytes(&bytes) {
            let image = gtk::Image::from_paintable(Some(&texture));
            image.set_pixel_size(34);
            avatar.append(&image);
        }
    }
    row.append(&avatar);

    let text = gtk::Box::new(gtk::Orientation::Vertical, 1);
    text.set_hexpand(true);
    text.set_valign(gtk::Align::Center);
    let name_text = account
        .display_name
        .clone()
        .or_else(|| account.username.clone())
        .unwrap_or_else(|| account.profile.clone());
    let name = gtk::Label::new(Some(&name_text));
    name.add_css_class("settings-value-title");
    name.set_xalign(0.0);
    name.set_ellipsize(gtk::pango::EllipsizeMode::End);
    text.append(&name);
    let meta_text = match (&account.username, account.user_id) {
        (Some(username), Some(user_id)) => format!(
            "@{} · {} · profile {}",
            username, user_id, account.profile
        ),
        _ => format!("no account yet · profile {}", account.profile),
    };
    let meta = gtk::Label::new(Some(&meta_text));
    meta.add_css_class("settings-hint");
    meta.set_xalign(0.0);
    meta.set_ellipsize(gtk::pango::EllipsizeMode::End);
    text.append(&meta);
    let cookie_state = gtk::Label::new(Some(if account.saved_cookie {
        "cookie saved locally"
    } else {
        "no cookie saved — paste one above"
    }));
    cookie_state.add_css_class("client-cookie");
    cookie_state.set_xalign(0.0);
    text.append(&cookie_state);
    row.append(&text);

    if let Some(user_id) = account.user_id {
        let profile_link = gtk::Button::with_label("Profile");
        profile_link.add_css_class("settings-inline-button");
        profile_link.set_tooltip_text(Some("Open the Roblox profile page"));
        let url = format!("https://www.roblox.com/users/{}/profile", user_id);
        profile_link.connect_clicked(move |_| {
            let _ = gtk::gio::AppInfo::launch_default_for_uri(
                &url,
                None::<&gtk::gio::AppLaunchContext>,
            );
        });
        row.append(&profile_link);
    }

    let launch = gtk::Button::with_label("Launch");
    launch.add_css_class("settings-inline-button");
    let weak = Rc::downgrade(ws);
    let profile_name = account.profile.clone();
    launch.connect_clicked(move |_| {
        if let Some(ws) = weak.upgrade() {
            match spawn_profile(&ws, &profile_name, None) {
                Ok(()) => ws.toast(format!("Launching {profile_name}…")),
                Err(error) => ws.toast(error),
            }
        }
    });
    row.append(&launch);

    let delete = gtk::Button::with_label("Delete");
    delete.add_css_class("settings-inline-button");
    delete.add_css_class("theme-delete");
    let weak = Rc::downgrade(ws);
    let list_for_delete = list.clone();
    let selection_for_delete = selection.clone();
    let profile_name = account.profile.clone();
    delete.connect_clicked(move |_| {
        let Some(ws) = weak.upgrade() else { return };
        let dir = profiles_dir().join(&profile_name);
        let _ = std::fs::remove_file(dir.join("account.json"));
        let _ = std::fs::remove_file(dir.join("cookie.txt"));
        ws.toast(format!("Removed {profile_name}'s account."));
        selection_for_delete.borrow_mut().clear();
        fill_accounts(&ws, &list_for_delete, &selection_for_delete);
    });
    row.append(&delete);

    row
}

fn profile_slug(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn set_private(path: &std::path::Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
}

/// Pulls the .ROBLOSECURITY value out of whatever the user pasted.
fn cookie_token(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    let upper = trimmed.to_ascii_uppercase();
    let body = match upper.find("ROBLOSECURITY=") {
        Some(index) => &trimmed[index + "ROBLOSECURITY=".len()..],
        None => trimmed,
    };
    let value: String = body
        .chars()
        .take_while(|ch| *ch != ';' && *ch != '\n' && *ch != '\r')
        .collect();
    let value = value.trim().trim_matches('"').trim().to_string();
    if value.len() < 40 {
        return Err("That does not look like a full .ROBLOSECURITY cookie.".to_string());
    }
    if value.chars().any(|ch| ch.is_whitespace()) {
        return Err("The cookie has spaces in it — paste it again in one piece.".to_string());
    }
    if value.contains('"') || value.contains('\\') {
        return Err("That cookie has characters a .ROBLOSECURITY value never has.".to_string());
    }
    Ok(value)
}

/// The launcher validates a profile directory before it will run it and
/// answers "unrecognized Profile layout" when the marker file is missing, which
/// is exactly what a cookie-only profile used to look like.
fn ensure_profile_layout(dir: &std::path::Path) -> Result<(), String> {
    let state = dir.join("state");
    if state.join(".format").is_file() {
        return Ok(());
    }
    std::fs::create_dir_all(&state).map_err(|error| error.to_string())?;
    for sub in [
        "state/app/data/sober",
        "state/app/config",
        "state/app/.local",
        "state/app/cache",
        "state/assets",
        "state/assets-work",
        "runtime/tmp",
        "runtime/run",
    ] {
        std::fs::create_dir_all(dir.join(sub)).map_err(|error| error.to_string())?;
    }
    let marker = layout_template(dir).unwrap_or_else(|| "1\n".to_string());
    let path = state.join(".format");
    std::fs::write(&path, marker).map_err(|error| error.to_string())?;
    set_private(&path);
    Ok(())
}

/// Reuses the marker from a profile the launcher wrote itself, so a future
/// layout version is picked up instead of hard-coding one.
fn layout_template(current: &std::path::Path) -> Option<String> {
    for entry in std::fs::read_dir(profiles_dir()).ok()?.flatten() {
        let path = entry.path();
        if path == current {
            continue;
        }
        if let Ok(raw) = std::fs::read_to_string(path.join("state/.format")) {
            if !raw.trim().is_empty() {
                return Some(raw);
            }
        }
    }
    None
}

/// A brand-new profile's Sober state drops the client into Sober's onboarding
/// UI, whose service dies with an X11 MIT-SHM BadAccess on some desktops, so a
/// pasted-cookie profile never opened a window. Reusing the state of a profile
/// that has already launched skips that first-run flow.
fn seed_sober_state(dir: &std::path::Path) {
    let target = dir.join("state/app/data/sober/state");
    let current = std::fs::read_to_string(&target).unwrap_or_default();
    if !is_first_run_state(&current) {
        return;
    }
    let donor = std::fs::read_dir(profiles_dir())
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .find(|path| {
            path != dir
                && std::fs::read_to_string(path.join("state/app/data/sober/state"))
                    .map(|raw| !is_first_run_state(&raw))
                    .unwrap_or(false)
        });
    let Some(donor) = donor else { return };
    if let Some(parent) = target.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::copy(donor.join("state/app/data/sober/state"), &target);
}

/// Sober writes impression 0 into a state file it has never run with.
fn is_first_run_state(raw: &str) -> bool {
    !raw.contains("tag__onboarding_home_impression")
        || raw.contains("\"tag__onboarding_home_impression\": 0")
}

/// Saves the pasted cookie for a profile: Relapse's own copy plus the Sober
/// cookie jar that profile launches with, so the next launch starts signed in.
fn write_cookie(profile: &str, token: &str) -> Result<(), String> {
    let dir = profiles_dir().join(profile);
    ensure_profile_layout(&dir)?;
    seed_sober_state(&dir);
    let copy = dir.join("cookie.txt");
    std::fs::write(&copy, format!("ROBLOSECURITY={token}\n"))
        .map_err(|error| format!("Could not save the cookie: {error}"))?;
    set_private(&copy);

    let jar = dir.join("state/app/data/sober/cookies");
    if let Some(parent) = jar.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("Could not prepare the profile: {error}"))?;
    }
    let mut pairs: Vec<String> = std::fs::read_to_string(&jar)
        .unwrap_or_default()
        .split(';')
        .map(|pair| pair.trim().to_string())
        .filter(|pair| {
            !pair.is_empty()
                && !pair
                    .split('=')
                    .next()
                    .unwrap_or_default()
                    .trim()
                    .to_ascii_uppercase()
                    .contains("ROBLOSECURITY")
        })
        .collect();
    // Sober's own jars name this cookie with the leading dot.
    pairs.push(format!(".ROBLOSECURITY={token}"));
    std::fs::write(&jar, pairs.join("; "))
        .map_err(|error| format!("Could not write the profile login: {error}"))?;
    set_private(&jar);
    Ok(())
}

struct AccountInfo {
    user_id: u64,
    username: String,
    display_name: String,
    avatar_png_base64: Option<String>,
}

fn temp_path(tag: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("relapse-{tag}-{}-{stamp}", std::process::id()))
}

fn curl_config(url: &str, cookie: Option<&str>) -> String {
    let mut config = format!(
        "url = \"{url}\"\nsilent\nshow-error\nmax-time = 20\nlocation\nheader = \"User-Agent: RelapsePlus/2.0\"\n"
    );
    if let Some(token) = cookie {
        config.push_str(&format!(
            "header = \"Cookie: .ROBLOSECURITY={token}\"\nheader = \"Accept: application/json\"\n"
        ));
    }
    config
}

/// Runs curl with a throwaway config file so the cookie never shows up in the
/// process arguments.
fn run_curl(config: &str) -> Result<Vec<u8>, String> {
    let path = temp_path("curl");
    std::fs::write(&path, config).map_err(|error| format!("Could not prepare the request: {error}"))?;
    set_private(&path);
    let output = std::process::Command::new("curl").arg("--config").arg(&path).output();
    let _ = std::fs::remove_file(&path);
    let output = output.map_err(|error| format!("curl is required for the account check: {error}"))?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            "Roblox did not answer the account check.".to_string()
        } else {
            detail
        });
    }
    Ok(output.stdout)
}

fn check_account(token: &str) -> Result<AccountInfo, String> {
    let raw = run_curl(&curl_config(
        "https://users.roblox.com/v1/users/authenticated",
        Some(token),
    ))?;
    let data: serde_json::Value = serde_json::from_slice(&raw).map_err(|_| {
        "Roblox rejected that cookie — copy a fresh .ROBLOSECURITY value.".to_string()
    })?;
    let user_id = data
        .get("id")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| {
            "Roblox rejected that cookie — copy a fresh .ROBLOSECURITY value.".to_string()
        })?;
    let username = data
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("unknown")
        .to_string();
    let display_name = data
        .get("displayName")
        .and_then(|value| value.as_str())
        .unwrap_or(&username)
        .to_string();
    Ok(AccountInfo {
        user_id,
        username,
        display_name,
        avatar_png_base64: fetch_avatar(user_id),
    })
}

fn fetch_avatar(user_id: u64) -> Option<String> {
    let url = format!(
        "https://thumbnails.roblox.com/v1/users/avatar-headshot?userIds={user_id}&size=150x150&format=Png&isCircular=false"
    );
    let raw = run_curl(&curl_config(&url, None)).ok()?;
    let data: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    let image_url = data.get("data")?.get(0)?.get("imageUrl")?.as_str()?;
    let image = run_curl(&curl_config(image_url, None)).ok()?;
    if image.is_empty() {
        return None;
    }
    Some(base64_encode(&image))
}

fn store_account(profile: &str, token: &str, info: &AccountInfo) -> Result<(), String> {
    let dir = profiles_dir().join(profile);
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    let checked_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0);
    let mut payload = serde_json::Map::new();
    payload.insert("version".to_string(), serde_json::json!(1));
    payload.insert("user_id".to_string(), serde_json::json!(info.user_id));
    payload.insert("username".to_string(), serde_json::json!(info.username));
    payload.insert(
        "display_name".to_string(),
        serde_json::json!(info.display_name),
    );
    payload.insert(
        "cookie_sha256".to_string(),
        serde_json::json!(sha256_hex(token)),
    );
    payload.insert("checked_at".to_string(), serde_json::json!(checked_at));
    if let Some(avatar) = &info.avatar_png_base64 {
        payload.insert("avatar_png_base64".to_string(), serde_json::json!(avatar));
    }
    let path = dir.join("account.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&serde_json::Value::Object(payload)).unwrap_or_default(),
    )
    .map_err(|error| format!("Could not save the account: {error}"))?;
    set_private(&path);
    Ok(())
}

fn check_and_store(profile: &str, token: &str) -> Result<String, String> {
    let info = check_account(token)?;
    store_account(profile, token, &info)?;
    Ok(info.username)
}

fn sha256_hex(input: &str) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut data = input.as_bytes().to_vec();
    let bit_len = (data.len() as u64) * 8;
    data.push(0x80);
    while data.len() % 64 != 56 {
        data.push(0);
    }
    data.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in data.chunks(64) {
        let mut words = [0u32; 64];
        for index in 0..16 {
            words[index] = u32::from_be_bytes([
                chunk[index * 4],
                chunk[index * 4 + 1],
                chunk[index * 4 + 2],
                chunk[index * 4 + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }
        let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) = (
            state[0], state[1], state[2], state[3], state[4], state[5], state[6], state[7],
        );
        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }
    state.iter().map(|value| format!("{value:08x}")).collect()
}

fn base64_encode(input: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(input.len().div_ceil(3) * 4);
    for chunk in input.chunks(3) {
        let first = chunk[0] as u32;
        let second = *chunk.get(1).unwrap_or(&0) as u32;
        let third = *chunk.get(2).unwrap_or(&0) as u32;
        let value = (first << 16) | (second << 8) | third;
        out.push(TABLE[((value >> 18) & 63) as usize] as char);
        out.push(TABLE[((value >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[(value & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

fn cookie_dialog(
    ws: &Rc<Workspace>,
    profile: &str,
    list: &gtk::Box,
    selection: &Rc<RefCell<Vec<(gtk::CheckButton, String)>>>,
) {
    // Keep exactly one paste window. A GTK toplevel with no strong reference
    // is finalized as soon as this function returns, which looked like the
    // button doing nothing.
    if let Some(existing) = ws.cookie_window.borrow_mut().take() {
        existing.destroy();
    }
    let parent = ws
        .clients_window
        .borrow()
        .clone()
        .unwrap_or_else(|| ws.window.clone().upcast());
    let title = if profile.trim().is_empty() {
        "Paste cookie".to_string()
    } else {
        format!("Paste cookie — {}", profile_slug(profile))
    };
    let dialog = gtk::Window::builder()
        .title(&title)
        .transient_for(&parent)
        .modal(true)
        .default_width(480)
        .default_height(340)
        .build();
    dialog.add_css_class("settings-window");

    let body = gtk::Box::new(gtk::Orientation::Vertical, 10);
    body.add_css_class("settings-body");
    body.set_margin_top(16);
    body.set_margin_bottom(16);
    body.set_margin_start(18);
    body.set_margin_end(18);
    body.append(&label("PASTE COOKIE"));
    body.append(&hint(
        "Paste the .ROBLOSECURITY cookie below. It is saved for this profile and the account is checked automatically.",
    ));

    let name_row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    let name_label = key("Profile name");
    name_row.append(&name_label);
    let name_entry = gtk::Entry::new();
    name_entry.set_placeholder_text(Some("alt1"));
    name_entry.set_text(profile);
    name_entry.set_hexpand(true);
    name_row.append(&name_entry);
    body.append(&name_row);

    let view = gtk::TextView::new();
    view.set_wrap_mode(gtk::WrapMode::Char);
    view.set_monospace(true);
    view.set_top_margin(6);
    view.set_bottom_margin(6);
    view.set_left_margin(8);
    view.set_right_margin(8);
    let scroll = gtk::ScrolledWindow::new();
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.set_min_content_height(130);
    scroll.set_vexpand(true);
    scroll.set_child(Some(&view));
    body.append(&scroll);

    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label("Cancel");
    let save = gtk::Button::with_label("Save cookie");
    save.add_css_class("settings-inline-button");
    buttons.append(&cancel);
    buttons.append(&save);
    body.append(&buttons);
    dialog.set_child(Some(&body));

    let dialog_for_cancel = dialog.clone();
    cancel.connect_clicked(move |_| dialog_for_cancel.destroy());

    let weak = Rc::downgrade(ws);
    let buffer = view.buffer();
    let dialog_for_save = dialog.clone();
    let list_for_save = list.clone();
    let selection_for_save = selection.clone();
    let name_entry_for_save = name_entry.clone();
    save.connect_clicked(move |_| {
        let Some(ws) = weak.upgrade() else { return };
        let raw_name = name_entry_for_save.text().trim().to_string();
        if raw_name.is_empty() {
            ws.toast("Give the profile a name first.");
            return;
        }
        let profile_owned = profile_slug(&raw_name);
        let text = buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), false)
            .to_string();
        let token = match cookie_token(&text) {
            Ok(token) => token,
            Err(error) => {
                ws.toast(error);
                return;
            }
        };
        if let Err(error) = write_cookie(&profile_owned, &token) {
            ws.toast(error);
            return;
        }
        dialog_for_save.destroy();
        selection_for_save.borrow_mut().clear();
        fill_accounts(&ws, &list_for_save, &selection_for_save);
        ws.toast(format!("Cookie saved — checking {profile_owned}'s account…"));

        // The Roblox lookup blocks on the network, so run it off the GTK
        // thread and poll for the result.
        let (tx, rx) = std::sync::mpsc::channel::<Result<String, String>>();
        let profile_for_thread = profile_owned.clone();
        let token_for_thread = token.clone();
        std::thread::spawn(move || {
            let _ = tx.send(check_and_store(&profile_for_thread, &token_for_thread));
        });

        let ws_poll = ws.clone();
        let list_poll = list_for_save.clone();
        let selection_poll = selection_for_save.clone();
        let profile_for_poll = profile_owned.clone();
        let started = std::time::Instant::now();
        glib::timeout_add_local(std::time::Duration::from_millis(200), move || {
            match rx.try_recv() {
                Ok(Ok(username)) => {
                    fill_accounts(&ws_poll, &list_poll, &selection_poll);
                    ws_poll.toast(format!(
                        "@{username} — {profile_for_poll} is ready to launch."
                    ));
                    glib::ControlFlow::Break
                }
                Ok(Err(error)) => {
                    fill_accounts(&ws_poll, &list_poll, &selection_poll);
                    ws_poll.toast(format!(
                        "Cookie saved, but the account check failed: {error}"
                    ));
                    glib::ControlFlow::Break
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    if started.elapsed() > std::time::Duration::from_secs(45) {
                        ws_poll.toast("Cookie saved; the account check timed out.");
                        glib::ControlFlow::Break
                    } else {
                        glib::ControlFlow::Continue
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => glib::ControlFlow::Break,
            }
        });
    });

    ws.cookie_window.replace(Some(dialog.clone()));
    let weak = Rc::downgrade(ws);
    dialog.connect_destroy(move |_| {
        if let Some(ws) = weak.upgrade() {
            ws.cookie_window.replace(None);
        }
    });
    dialog.present();
    if profile.trim().is_empty() {
        name_entry.grab_focus();
    } else {
        view.grab_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_matches_known_vectors() {
        assert_eq!(
            sha256_hex(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_encode(b"f"), "Zg==");
        assert_eq!(base64_encode(b"fo"), "Zm8=");
        assert_eq!(base64_encode(b"foo"), "Zm9v");
        assert_eq!(base64_encode(b"hello"), "aGVsbG8=");
    }

    #[test]
    fn spots_first_run_sober_state() {
        assert!(is_first_run_state(""));
        assert!(is_first_run_state(
            "{\"v4\": {\"tag__onboarding_home_impression\": 0}}"
        ));
        assert!(!is_first_run_state(
            "{\"v4\": {\"tag__onboarding_home_impression\": 8}}"
        ));
    }

    #[test]
    fn cookie_token_accepts_prefixed_and_raw_values() {
        let token = "A".repeat(60);
        assert_eq!(
            cookie_token(&format!(".ROBLOSECURITY={token}; other=1")).unwrap(),
            token
        );
        assert_eq!(cookie_token(&token).unwrap(), token);
        assert!(cookie_token("too-short").is_err());
    }
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut buffer: u64 = 0;
    let mut bits = 0u32;
    for byte in input.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\n' | b'\r' | b' ' | b'\t' => continue,
            _ => return None,
        } as u64;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    Some(out)
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
    label.set_wrap(true);
    label
}
