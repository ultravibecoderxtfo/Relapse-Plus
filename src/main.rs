mod clients;
mod completion;
mod protocol;
mod settings;
mod theme;

use completion::Completion;
use gtk::prelude::*;
use gtk::{gdk, glib, Application, ApplicationWindow};
use protocol::{profile_name, profile_sockets, Event, ProtocolClient};
use sourceview5 as source;
use sourceview5::prelude::*;
use std::cell::{Cell, RefCell};
use std::ffi::OsString;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

const APP_ID: &str = "dev.relapse.Relapse";
const STYLE: &str = include_str!("../style.css");
const EXTRA_STYLE: &str = r#"
.completion-popover, .completion-popover > contents {
  background-color: #252526; border: 1px solid #3a3a3a; border-radius: 6px;
}
.completion-panel {
  background-color: #252526; border: 1px solid #3a3a3a; border-radius: 6px;
  padding: 4px; box-shadow: 0 8px 22px rgba(0, 0, 0, 0.45);
}
.completion-wrap { min-width: 232px; }
.completion-hint {
  color: #777; font-size: 8.5px; letter-spacing: .4px; padding: 4px 8px 3px;
}
.completion-list { background: transparent; }
.completion-row {
  min-height: 21px; padding: 0 8px; border-radius: 3px; background: transparent;
}
.completion-row.selected { background-color: #094771; }
.completion-label { color: #c8c8c8; font-family: monospace; font-size: 11.5px; }
.completion-row.selected .completion-label { color: #ffffff; }
.tab-menu { padding: 5px; min-width: 205px; }
.tab-menu-item {
  min-height: 28px; padding: 0 10px; background: transparent; border: none;
  border-radius: 4px; color: #c9c9c9; font-size: 11px;
}
.tab-menu-item:hover { background-color: #333; color: #fff; }
.tab.pinned .tab-name { color: #cfcfcf; }
"#;

const LUA_LANG: &str = include_str!("../assets/lua.lang");
const THEME: &str = include_str!("../assets/relapse-dark.xml");
const LIGHT_THEME: &str = include_str!("../assets/relapse-light.xml");

struct Tab {
    id: u64,
    title: RefCell<String>,
    path: RefCell<Option<PathBuf>>,
    buffer: source::Buffer,
    view: source::View,
    page: gtk::Overlay,
    widget: gtk::Box,
    name_label: gtk::Label,
    dirty: gtk::Label,
    pinned: Cell<bool>,
    read_only: Cell<bool>,
    completion: Rc<Completion>,
}

struct Workspace {
    window: ApplicationWindow,
    stack: gtk::Stack,
    tabs_box: gtk::Box,
    tabs: RefCell<Vec<Rc<Tab>>>,
    current: RefCell<Option<Rc<Tab>>>,
    next_id: Cell<u64>,
    lang: Option<source::Language>,
    scheme_dark: Option<source::StyleScheme>,
    scheme_light: Option<source::StyleScheme>,
    theme_store: Rc<RefCell<theme::ThemeStore>>,
    theme_provider: gtk::CssProvider,
    prefs: Rc<RefCell<theme::Prefs>>,
    settings_window: RefCell<Option<gtk::Window>>,
    scripts: gtk::Box,
    console: gtk::TextView,
    console_revealer: gtk::Revealer,
    toast: gtk::Label,
    toast_revealer: gtk::Revealer,
    pill: gtk::MenuButton,
    pill_label: gtk::Label,
    profiles_body: gtk::Box,
    profiles_popover: gtk::Popover,
    instances: RefCell<Vec<Rc<RelapseInstance>>>,
    launch_button: gtk::Button,
    execute_button: gtk::Button,
    file_execute_button: gtk::Button,
    open_button: gtk::Button,
    save_button: gtk::Button,
    new_button: gtk::Button,
    output_button: gtk::Button,
    clear_button: gtk::Button,
    scripts_refresh_button: gtk::Button,
    settings_button: gtk::Button,
    clients_button: gtk::Button,
    clients_window: RefCell<Option<gtk::Window>>,
    cookie_window: RefCell<Option<gtk::Window>>,
    filter_roblox: gtk::ToggleButton,
    filter_ui: gtk::ToggleButton,
    console_last: RefCell<String>,
    completion_enabled: Rc<Cell<bool>>,
    detached: RefCell<Vec<gtk::Window>>,
    _lang_manager: source::LanguageManager,
}

/// One attached Relapse profile socket. Several can be attached at once so an
/// execute is broadcast to every running Roblox instance.
struct RelapseInstance {
    path: PathBuf,
    client: ProtocolClient,
    status: RefCell<String>,
    ready: Cell<bool>,
    session: Cell<i64>,
}

impl Workspace {
    fn new(app: &Application) -> Rc<Self> {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Relapse +")
            .default_width(1040)
            .default_height(620)
            .build();
        window.set_size_request(820, 480);
        window.add_css_class("relapse-window");

        let launch_button = gtk::Button::with_label("Launch");
        launch_button.add_css_class("title-launch");
        launch_button.set_tooltip_text(Some("Launch Relapse"));
        let new_button = gtk::Button::with_label("+");
        new_button.add_css_class("tab-new");
        new_button.set_tooltip_text(Some("New tab (Ctrl+N)"));

        let tabs_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let tabbar = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        tabbar.add_css_class("tabbar");
        tabbar.append(&tabs_box);
        tabbar.append(&new_button);

        let stack = gtk::Stack::new();
        stack.set_hexpand(true);
        stack.set_vexpand(true);
        stack.set_transition_type(gtk::StackTransitionType::None);

        let scripts = gtk::Box::new(gtk::Orientation::Vertical, 2);
        scripts.add_css_class("scripts-list");
        let scripts_scroll = gtk::ScrolledWindow::new();
        scripts_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scripts_scroll.set_child(Some(&scripts));
        let scripts_refresh_button = gtk::Button::with_label("Refresh");
        scripts_refresh_button.add_css_class("settings-inline-button");
        let scripts_head = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        scripts_head.add_css_class("settings-card-row");
        let scripts_title = gtk::Label::new(Some("SCRIPTS"));
        scripts_title.add_css_class("panel-title");
        scripts_title.set_xalign(0.0);
        scripts_title.set_hexpand(true);
        scripts_head.append(&scripts_title);
        scripts_head.append(&scripts_refresh_button);
        let scripts_panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        scripts_panel.add_css_class("scripts-panel");
        scripts_panel.set_size_request(230, -1);
        scripts_panel.set_hexpand(false);
        scripts_panel.set_halign(gtk::Align::Fill);
        scripts_panel.append(&scripts_head);
        scripts_panel.append(&scripts_scroll);
        scripts_scroll.set_vexpand(true);

        let console = gtk::TextView::new();
        console.set_editable(false);
        console.set_cursor_visible(false);
        console.set_monospace(true);
        console.set_wrap_mode(gtk::WrapMode::WordChar);
        console.add_css_class("console-text");
        let console_scroll = gtk::ScrolledWindow::new();
        console_scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        console_scroll.set_min_content_height(120);
        console_scroll.set_child(Some(&console));
        let console_head = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        console_head.add_css_class("console-head");
        let console_title = gtk::Label::new(Some("OUTPUT"));
        console_title.add_css_class("console-title");
        console_head.append(&console_title);
        let filter_roblox = gtk::ToggleButton::with_label("Roblox");
        filter_roblox.add_css_class("console-filter");
        filter_roblox.set_active(true);
        filter_roblox.set_tooltip_text(Some("Script output redirected from Roblox"));
        let filter_ui = gtk::ToggleButton::with_label("UI");
        filter_ui.add_css_class("console-filter");
        filter_ui.set_active(true);
        filter_ui.set_tooltip_text(Some("Executor messages"));
        console_head.append(&filter_roblox);
        console_head.append(&filter_ui);
        let spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        spacer.set_hexpand(true);
        console_head.append(&spacer);
        let clear_button = gtk::Button::with_label("Clear");
        clear_button.add_css_class("console-action");
        console_head.append(&clear_button);
        let output_panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        output_panel.add_css_class("console-panel");
        output_panel.append(&console_head);
        output_panel.append(&console_scroll);
        let console_revealer = gtk::Revealer::new();
        console_revealer.set_transition_type(gtk::RevealerTransitionType::SlideUp);
        console_revealer.set_transition_duration(140);
        console_revealer.set_reveal_child(false);
        console_revealer.set_child(Some(&output_panel));

        let execute_button = gtk::Button::with_label("Execute");
        execute_button.add_css_class("exec-button");
        execute_button.set_tooltip_text(Some("Execute current tab (Ctrl+Enter)"));
        let file_execute_button = gtk::Button::with_label("Execute File");
        file_execute_button.add_css_class("tool-button");
        let open_button = gtk::Button::with_label("Open");
        open_button.add_css_class("tool-button");
        let save_button = gtk::Button::with_label("Save");
        save_button.add_css_class("tool-button");
        let output_button = gtk::Button::with_label("Output");
        output_button.add_css_class("tool-button");
        let settings_button = gtk::Button::with_label("Settings");
        settings_button.add_css_class("tool-button");
        settings_button.set_tooltip_text(Some("Settings (Ctrl+,)"));
        let clients_button = gtk::Button::with_label("Clients");
        clients_button.add_css_class("tool-button");
        clients_button.set_tooltip_text(Some("Client Manager — accounts and profiles"));
        let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 5);
        toolbar.add_css_class("toolbar");
        toolbar.append(&file_execute_button);
        toolbar.append(&open_button);
        toolbar.append(&save_button);
        toolbar.append(&output_button);
        toolbar.append(&settings_button);
        toolbar.append(&clients_button);
        let tool_spacer = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        tool_spacer.set_hexpand(true);
        toolbar.append(&tool_spacer);

        let pill_label = gtk::Label::new(Some("Disconnected"));
        pill_label.set_xalign(0.0);
        let pill_content = gtk::Box::new(gtk::Orientation::Horizontal, 7);
        pill_content.append(&pill_label);
        let chevron = gtk::Label::new(Some("⌄"));
        chevron.add_css_class("pill-chevron");
        pill_content.append(&chevron);
        let pill = gtk::MenuButton::new();
        pill.add_css_class("instance-pill");
        pill.set_child(Some(&pill_content));
        pill.set_always_show_arrow(false);
        pill.set_tooltip_text(Some("Relapse profile"));
        let profiles_body = gtk::Box::new(gtk::Orientation::Vertical, 3);
        profiles_body.add_css_class("popover-body");
        profiles_body.set_size_request(286, -1);
        let profiles_popover = gtk::Popover::new();
        profiles_popover.add_css_class("instances-popover");
        profiles_popover.set_has_arrow(false);
        profiles_popover.set_child(Some(&profiles_body));
        pill.set_popover(Some(&profiles_popover));
        pill.set_valign(gtk::Align::Center);
        execute_button.set_valign(gtk::Align::Center);
        toolbar.append(&pill);
        toolbar.append(&execute_button);

        let editor_pane = gtk::Box::new(gtk::Orientation::Vertical, 0);
        editor_pane.set_hexpand(true);
        editor_pane.append(&stack);
        let body = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        body.set_vexpand(true);
        body.set_hexpand(true);
        body.append(&editor_pane);
        body.append(&scripts_panel);

        let toast = gtk::Label::new(None);
        toast.add_css_class("toast");
        toast.set_wrap(true);
        let toast_revealer = gtk::Revealer::new();
        toast_revealer.set_transition_type(gtk::RevealerTransitionType::SlideUp);
        toast_revealer.set_transition_duration(120);
        toast_revealer.set_halign(gtk::Align::Center);
        toast_revealer.set_valign(gtk::Align::End);
        toast_revealer.set_margin_bottom(58);
        toast_revealer.set_child(Some(&toast));
        toast_revealer.set_can_target(false);
        toast_revealer.set_can_focus(false);

        let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
        root.append(&tabbar);
        root.append(&body);
        root.append(&console_revealer);
        root.append(&toolbar);
        let overlay = gtk::Overlay::new();
        overlay.set_child(Some(&root));
        overlay.add_overlay(&toast_revealer);

        let titlebar = gtk::Overlay::new();
        titlebar.add_css_class("titlebar-inner");
        let center = gtk::CenterBox::new();
        let title = gtk::Label::new(Some("Relapse +"));
        title.add_css_class("window-title");
        center.set_center_widget(Some(&title));
        // The handle fills the whole bar so every empty spot drags the window,
        // but the traffic lights and Launch button are overlay children drawn
        // above it, so their presses can never be swallowed by the drag gesture.
        let handle = gtk::WindowHandle::new();
        handle.set_child(Some(&center));
        titlebar.set_child(Some(&handle));
        let lights = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        lights.set_valign(gtk::Align::Center);
        lights.set_halign(gtk::Align::Start);
        lights.set_margin_start(14);
        lights.set_margin_end(8);
        let close = traffic_button("red", "×", &window);
        let minimize = traffic_button("yellow", "−", &window);
        let maximize = traffic_button("green", "+", &window);
        let win = window.clone();
        close.connect_clicked(move |_| win.close());
        let win = window.clone();
        minimize.connect_clicked(move |_| win.minimize());
        let win = window.clone();
        maximize.connect_clicked(move |_| {
            if win.is_maximized() { win.unmaximize(); } else { win.maximize(); }
        });
        lights.append(&close);
        lights.append(&minimize);
        lights.append(&maximize);
        titlebar.add_overlay(&lights);
        let right = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        right.set_valign(gtk::Align::Center);
        right.set_halign(gtk::Align::End);
        right.set_margin_start(8);
        right.set_margin_end(14);
        right.append(&launch_button);
        titlebar.add_overlay(&right);
        window.set_titlebar(Some(&titlebar));
        window.set_child(Some(&overlay));

        let assets = assets_dir();
        let lang_manager = source::LanguageManager::new();
        lang_manager.append_search_path(assets.join("language-specs").to_string_lossy().as_ref());
        let style_manager = source::StyleSchemeManager::new();
        style_manager.append_search_path(assets.join("styles").to_string_lossy().as_ref());
        let lang = lang_manager.language("lua");
        let scheme_dark = style_manager.scheme("relapse-dark");
        let scheme_light = style_manager.scheme("relapse-light");
        if lang.is_none() {
            eprintln!("relapse-ui: Lua language spec not found");
        }
        if scheme_dark.is_none() || scheme_light.is_none() {
            eprintln!("relapse-ui: GtkSourceView style scheme missing");
        }

        let theme_store = Rc::new(RefCell::new(theme::ThemeStore::load()));
        let prefs = Rc::new(RefCell::new(theme::Prefs::load()));
        let completion_enabled = Rc::new(Cell::new(prefs.borrow().autocomplete));
        let theme_provider = gtk::CssProvider::new();
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &theme_provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER + 1,
            );
        }

        // Only connect to a socket that actually exists. default_socket_path()
        // falls back to /tmp/relapse.sock, and retrying that forever spammed
        // "No Relapse socket at /tmp/relapse.sock" when no game was running.
        let initial_socket = protocol::default_socket_path();
        let explicit_socket = std::env::var_os("RELAPSE_SOCKET").is_some();
        let mut instances: Vec<Rc<RelapseInstance>> = Vec::new();
        if initial_socket.exists() || explicit_socket {
            let client = ProtocolClient::new(initial_socket.clone());
            instances.push(Rc::new(RelapseInstance {
                path: initial_socket,
                client,
                status: RefCell::new("connecting".to_string()),
                ready: Cell::new(false),
                session: Cell::new(0),
            }));
        }

        let workspace = Rc::new(Self {
            window: window.clone(),
            stack,
            tabs_box,
            tabs: RefCell::new(Vec::new()),
            current: RefCell::new(None),
            next_id: Cell::new(0),
            lang,
            scheme_dark,
            scheme_light,
            theme_store,
            theme_provider,
            prefs,
            settings_window: RefCell::new(None),
            scripts,
            console,
            console_revealer,
            toast,
            toast_revealer,
            pill,
            pill_label,
            profiles_body,
            profiles_popover,
            instances: RefCell::new(instances),
            launch_button,
            execute_button,
            file_execute_button,
            open_button,
            save_button,
            new_button,
            output_button,
            clear_button,
            scripts_refresh_button,
            settings_button,
            clients_button,
            clients_window: RefCell::new(None),
            cookie_window: RefCell::new(None),
            filter_roblox,
            filter_ui,
            console_last: RefCell::new(String::new()),
            completion_enabled,
            detached: RefCell::new(Vec::new()),
            _lang_manager: lang_manager,
        });
        workspace.install();
        workspace.refresh_scripts();
        workspace.new_blank();
        workspace.refresh_profiles();
        workspace.update_pill();
        workspace.poll();
        let active_theme = workspace.theme_store.borrow().active.clone();
        workspace.apply_theme(&active_theme);
        window.present();
        if let Ok(which) = std::env::var("RELAPSE_TEST_OPEN") {
            let ws = workspace.clone();
            glib::timeout_add_local_once(Duration::from_millis(400), move || {
                if which == "clients" {
                    ws.open_clients();
                } else {
                    ws.open_settings();
                }
                let ws_close = ws.clone();
                glib::timeout_add_local_once(Duration::from_millis(900), move || {
                    if let Some(window) = ws_close.settings_window.borrow_mut().take() {
                        window.destroy();
                    }
                    if let Some(window) = ws_close.clients_window.borrow_mut().take() {
                        window.destroy();
                    }
                });
            });
        }
        if let Ok(value) = std::env::var("RELAPSE_TEST_CLOSE_AFTER_MS") {
            if let Ok(milliseconds) = value.parse::<u64>() {
                let win = window.clone();
                glib::timeout_add_local_once(
                    Duration::from_millis(milliseconds),
                    move || win.close(),
                );
            }
        }
        workspace
    }

    fn install(self: &Rc<Self>) {
        let ws = Rc::downgrade(self);
        self.launch_button.connect_clicked(move |_| {
            if let Some(ws) = ws.upgrade() { ws.launch_profile(); }
        });
        let ws = Rc::downgrade(self);
        self.new_button.connect_clicked(move |_| {
            if let Some(ws) = ws.upgrade() { ws.new_blank(); }
        });
        let ws = Rc::downgrade(self);
        self.execute_button.connect_clicked(move |_| {
            if let Some(ws) = ws.upgrade() { ws.execute_current(); }
        });
        let ws = Rc::downgrade(self);
        self.file_execute_button.connect_clicked(move |_| {
            if let Some(ws) = ws.upgrade() { ws.execute_file_dialog(); }
        });
        let ws = Rc::downgrade(self);
        self.open_button.connect_clicked(move |_| {
            if let Some(ws) = ws.upgrade() { ws.open_dialog(); }
        });
        let ws = Rc::downgrade(self);
        self.save_button.connect_clicked(move |_| {
            if let Some(ws) = ws.upgrade() { ws.save_current(); }
        });
        let ws = Rc::downgrade(self);
        self.output_button.connect_clicked(move |_| {
            if let Some(ws) = ws.upgrade() {
                let shown = ws.console_revealer.reveals_child();
                ws.console_revealer.set_reveal_child(!shown);
            }
        });
        let ws = Rc::downgrade(self);
        self.clear_button.connect_clicked(move |_| {
            if let Some(ws) = ws.upgrade() { ws.console.buffer().set_text(""); }
        });
        let ws = Rc::downgrade(self);
        self.settings_button.connect_clicked(move |_| {
            if let Some(ws) = ws.upgrade() { ws.open_settings(); }
        });
        let ws = Rc::downgrade(self);
        self.clients_button.connect_clicked(move |_| {
            if let Some(ws) = ws.upgrade() { ws.open_clients(); }
        });
        let ws = Rc::downgrade(self);
        self.scripts_refresh_button.connect_clicked(move |_| {
            if let Some(ws) = ws.upgrade() { ws.refresh_scripts(); }
        });
        let ws = Rc::downgrade(self);
        self.profiles_popover.connect_show(move |_| {
            if let Some(ws) = ws.upgrade() { ws.refresh_profiles(); }
        });

        let ws = Rc::downgrade(self);
        let keys = gtk::EventControllerKey::new();
        // Capture at the window so shortcuts still fire while the editor has focus.
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(move |_, keyval, _, state| {
            let Some(ws) = ws.upgrade() else { return glib::Propagation::Proceed; };
            let ctrl = state.contains(gdk::ModifierType::CONTROL_MASK);
            if !ctrl { return glib::Propagation::Proceed; }
            let shift = state.contains(gdk::ModifierType::SHIFT_MASK);
            match keyval {
                gdk::Key::Return | gdk::Key::KP_Enter if shift => {
                    ws.execute_file_dialog(); glib::Propagation::Stop
                }
                gdk::Key::Return | gdk::Key::KP_Enter => {
                    ws.execute_current(); glib::Propagation::Stop
                }
                gdk::Key::o | gdk::Key::O => { ws.open_dialog(); glib::Propagation::Stop }
                gdk::Key::s | gdk::Key::S => { ws.save_current(); glib::Propagation::Stop }
                gdk::Key::n | gdk::Key::N => { ws.new_blank(); glib::Propagation::Stop }
                gdk::Key::comma => { ws.open_settings(); glib::Propagation::Stop }
                gdk::Key::w | gdk::Key::W => {
                    if let Some(tab) = ws.current.borrow().as_ref() { ws.close_tab(&tab.clone()); }
                    glib::Propagation::Stop
                }
                _ => glib::Propagation::Proceed,
            }
        });
        self.window.add_controller(keys);

        let ws = Rc::downgrade(self);
        self.window.connect_close_request(move |_| {
            if let Some(ws) = ws.upgrade() {
                for instance in ws.instances.borrow().iter() {
                    instance.client.stop();
                }
            }
            glib::Propagation::Proceed
        });
    }

    fn poll(self: &Rc<Self>) {
        let ws = self.clone();
        glib::timeout_add_local(Duration::from_millis(50), move || {
            let instances = ws.instances.borrow().clone();
            for instance in instances {
                while let Some(event) = instance.client.try_recv() {
                    ws.handle_event(&instance, event);
                }
            }
            glib::ControlFlow::Continue
        });
    }

    fn active_tab(&self) -> Option<Rc<Tab>> {
        self.current.borrow().clone()
    }

    fn new_blank(self: &Rc<Self>) {
        let id = self.next_id.get() + 1;
        self.next_id.set(id);
        let title = format!("untitled-{id}.lua");
        let tab = self.make_tab(id, &title, "", None);
        self.select_tab(&tab);
        tab.view.grab_focus();
    }

    fn make_tab(
        self: &Rc<Self>,
        id: u64,
        title: &str,
        text: &str,
        path: Option<PathBuf>,
    ) -> Rc<Tab> {
        let buffer = source::Buffer::new(None);
        buffer.set_language(self.lang.as_ref());
        buffer.set_style_scheme(self.current_scheme().as_ref());
        buffer.set_highlight_syntax(true);
        buffer.set_text(text);
        buffer.set_modified(false);
        let view = source::View::with_buffer(&buffer);
        configure_view(&view);
        let scroll = gtk::ScrolledWindow::new();
        scroll.add_css_class("editor-scroll");
        scroll.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        scroll.set_hexpand(true);
        scroll.set_vexpand(true);
        scroll.set_child(Some(&view));
        let page = gtk::Overlay::new();
        page.set_hexpand(true);
        page.set_vexpand(true);
        page.set_child(Some(&scroll));
        self.stack.add_named(&page, Some(&format!("tab-{id}")));

        let widget = gtk::Box::new(gtk::Orientation::Horizontal, 6);
        widget.add_css_class("tab");
        let icon = gtk::Image::from_icon_name("text-x-generic-symbolic");
        icon.set_pixel_size(12);
        icon.add_css_class("tab-icon");
        let name_label = gtk::Label::new(Some(title));
        name_label.add_css_class("tab-name");
        name_label.set_ellipsize(gtk::pango::EllipsizeMode::End);
        name_label.set_max_width_chars(26);
        let dirty = gtk::Label::new(Some("•"));
        dirty.add_css_class("dirty-dot");
        dirty.set_visible(false);
        let close = gtk::Button::with_label("×");
        close.add_css_class("tab-close");
        close.set_has_frame(false);
        widget.append(&icon);
        widget.append(&name_label);
        widget.append(&dirty);
        widget.append(&close);
        widget.set_tooltip_text(Some(&path.as_ref().map(|p| p.display().to_string()).unwrap_or_else(|| title.to_string())));

        let completion = Completion::attach(
            &view,
            &buffer,
            &scroll,
            &page,
            self.completion_enabled.clone(),
        );
        let tab = Rc::new(Tab {
            id,
            title: RefCell::new(title.to_string()),
            path: RefCell::new(path),
            buffer: buffer.clone(),
            view: view.clone(),
            page: page.clone(),
            widget: widget.clone(),
            name_label: name_label.clone(),
            dirty: dirty.clone(),
            pinned: Cell::new(false),
            read_only: Cell::new(false),
            completion,
        });

        let weak = Rc::downgrade(self);
        let tab_for_close = tab.clone();
        close.connect_clicked(move |_| {
            if let Some(ws) = weak.upgrade() { ws.close_tab(&tab_for_close); }
        });
        let weak = Rc::downgrade(self);
        let tab_for_click = tab.clone();
        let click = gtk::GestureClick::new();
        click.set_button(1);
        click.connect_pressed(move |_, _, _, _| {
            if let Some(ws) = weak.upgrade() { ws.select_tab(&tab_for_click); }
        });
        widget.add_controller(click);
        let weak = Rc::downgrade(self);
        let tab_for_menu = tab.clone();
        let menu_click = gtk::GestureClick::new();
        menu_click.set_button(3);
        menu_click.connect_pressed(move |_, _, x, y| {
            if let Some(ws) = weak.upgrade() { ws.open_tab_menu(&tab_for_menu, x, y); }
        });
        widget.add_controller(menu_click);

        let weak = Rc::downgrade(self);
        let tab_for_change = tab.clone();
        buffer.connect_modified_changed(move |b| {
            let visible = b.is_modified();
            tab_for_change.dirty.set_visible(visible);
            if let Some(ws) = weak.upgrade() {
                ws.update_tab_title(&tab_for_change);
            }
        });

        self.tabs.borrow_mut().push(tab.clone());
        self.tabs_box.append(&widget);
        tab
    }

    fn select_tab(self: &Rc<Self>, tab: &Rc<Tab>) {
        if !self.tabs.borrow().iter().any(|t| t.id == tab.id) {
            return;
        }
        for other in self.tabs.borrow().iter() {
            if other.id == tab.id {
                other.widget.add_css_class("active");
            } else {
                other.widget.remove_css_class("active");
            }
        }
        self.stack.set_visible_child(&tab.page);
        self.current.replace(Some(tab.clone()));
        tab.view.grab_focus();
    }

    fn close_tab(self: &Rc<Self>, tab: &Rc<Tab>) {
        let mut tabs = self.tabs.borrow_mut();
        let Some(index) = tabs.iter().position(|t| t.id == tab.id) else { return; };
        let was_current = self.current.borrow().as_ref().map(|t| t.id) == Some(tab.id);
        tabs.remove(index);
        drop(tabs);
        self.tabs_box.remove(&tab.widget);
        self.stack.remove(&tab.page);
        if was_current {
            self.current.replace(None);
            let next = self.tabs.borrow().get(index.min(self.tabs.borrow().len().saturating_sub(1))).cloned();
            if let Some(next) = next {
                self.select_tab(&next);
            } else {
                self.new_blank();
            }
        }
    }

    fn update_tab_title(&self, tab: &Tab) {
        let title = tab.title.borrow();
        tab.name_label.set_text(&title);
    }

    fn open_path(self: &Rc<Self>, path: PathBuf) {
        if !matches!(path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(), Some("lua" | "luau" | "txt")) {
            self.toast("Use a .lua, .luau, or .txt script");
            return;
        }
        if let Some(existing) = self.tabs.borrow().iter().find(|tab| tab.path.borrow().as_ref() == Some(&path)).cloned() {
            self.select_tab(&existing);
            return;
        }
        match std::fs::read_to_string(&path) {
            Ok(text) => {
                let id = self.next_id.get() + 1;
                self.next_id.set(id);
                let title = path.file_name().and_then(|n| n.to_str()).unwrap_or("script.lua").to_string();
                let tab = self.make_tab(id, &title, &text, Some(path));
                self.select_tab(&tab);
                self.update_sidebar();
            }
            Err(error) => self.toast(&format!("Could not open: {error}")),
        }
    }

    fn save_current(self: &Rc<Self>) {
        let Some(tab) = self.active_tab() else { return; };
        if let Some(path) = tab.path.borrow().clone() {
            self.write_tab(&tab, path);
            return;
        }
        let dialog = gtk::FileDialog::builder()
            .title("Save script")
            .modal(true)
            .initial_name(tab.title.borrow().clone())
            .build();
        let ws = Rc::downgrade(self);
        dialog.save(Some(&self.window), None::<&gtk::gio::Cancellable>, move |result| {
            let Some(ws) = ws.upgrade() else { return; };
            let Ok(file) = result else { return; };
            let Some(mut path) = file.path() else { return; };
            if path.extension().is_none() {
                path.set_extension("lua");
            }
            ws.write_tab(&tab, path);
            ws.refresh_scripts();
        });
    }

    fn write_tab(&self, tab: &Tab, path: PathBuf) {
        let text = tab.buffer.text(&tab.buffer.start_iter(), &tab.buffer.end_iter(), true).to_string();
        if let Err(error) = std::fs::write(&path, text) {
            self.toast(&format!("Could not save: {error}"));
            return;
        }
        tab.path.replace(Some(path.clone()));
        tab.title.replace(path.file_name().and_then(|n| n.to_str()).unwrap_or("script.lua").to_string());
        tab.buffer.set_modified(false);
        self.update_tab_title(tab);
        tab.widget.set_tooltip_text(Some(&path.display().to_string()));
        self.toast("Saved");
    }

    fn open_dialog(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder().title("Open script").modal(true).build();
        let ws = Rc::downgrade(self);
        dialog.open(Some(&self.window), None::<&gtk::gio::Cancellable>, move |result| {
            if let (Some(ws), Ok(file)) = (ws.upgrade(), result) {
                if let Some(path) = file.path() { ws.open_path(path); }
            }
        });
    }

    fn execute_file_dialog(self: &Rc<Self>) {
        let dialog = gtk::FileDialog::builder().title("Execute script").modal(true).build();
        let ws = Rc::downgrade(self);
        dialog.open(Some(&self.window), None::<&gtk::gio::Cancellable>, move |result| {
            let (Some(ws), Ok(file)) = (ws.upgrade(), result) else { return; };
            let Some(path) = file.path() else { return; };
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("script.lua").to_string();
            match std::fs::read_to_string(&path) {
                Ok(source) => ws.execute_source(&name, &source),
                Err(error) => ws.toast(&format!("Could not read: {error}")),
            }
        });
    }

    fn execute_current(self: &Rc<Self>) {
        let Some(tab) = self.active_tab() else { return; };
        let text = tab.buffer.text(&tab.buffer.start_iter(), &tab.buffer.end_iter(), true).to_string();
        if text.trim().is_empty() {
            self.toast("Nothing to execute");
            return;
        }
        let name = tab.title.borrow().clone();
        self.execute_source(&name, &text);
    }

    fn execute_source(self: &Rc<Self>, name: &str, source: &str) {
        self.console_revealer.set_reveal_child(true);
        let targets: Vec<Rc<RelapseInstance>> = self
            .instances
            .borrow()
            .iter()
            .filter(|instance| instance.ready.get() && instance.session.get() > 0)
            .cloned()
            .collect();
        if targets.is_empty() {
            let waiting = self
                .instances
                .borrow()
                .iter()
                .any(|instance| instance.status.borrow().as_str() == "waiting");
            let message = if waiting {
                "Relapse is connected, but no game session is ready."
            } else {
                "No Relapse profile is attached."
            };
            self.console_append(&format!("error: {message}"), true);
            self.toast(message);
            return;
        }
        if targets.len() == 1 {
            self.console_append(&format!("→ {name}"), false);
        } else {
            self.console_append(
                &format!("→ {name} ({} instances)", targets.len()),
                false,
            );
        }
        for instance in targets {
            instance.client.execute(name, source);
        }
    }

    fn refresh_scripts(self: &Rc<Self>) {
        while let Some(child) = self.scripts.first_child() {
            self.scripts.remove(&child);
        }
        let mut paths: Vec<PathBuf> = Vec::new();
        if let Ok(entries) = std::fs::read_dir("scripts") {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_file()
                    && matches!(path.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(), Some("lua" | "luau" | "txt"))
                {
                    paths.push(path);
                }
            }
        }
        paths.sort_by_key(|p| p.file_name().map(|n| n.to_string_lossy().to_lowercase()));
        if paths.is_empty() {
            let empty = gtk::Label::new(Some("No scripts yet.\nDrop .lua, .luau or .txt files into scripts/."));
            empty.add_css_class("script-empty");
            empty.set_xalign(0.0);
            empty.set_wrap(true);
            empty.set_margin_start(8);
            empty.set_margin_end(8);
            empty.set_margin_top(5);
            self.scripts.append(&empty);
            return;
        }
        let ws = Rc::downgrade(self);
        for path in paths {
            let button = gtk::Button::new();
            button.add_css_class("script-row");
            button.set_tooltip_text(Some(&path.display().to_string()));
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 7);
            let icon = gtk::Image::from_icon_name("text-x-generic-symbolic");
            icon.set_pixel_size(12);
            icon.add_css_class("script-icon");
            row.append(&icon);
            let label = gtk::Label::new(Some(&path.file_name().and_then(|n| n.to_str()).unwrap_or("script")));
            label.add_css_class("script-name");
            label.set_xalign(0.0);
            label.set_ellipsize(gtk::pango::EllipsizeMode::End);
            row.append(&label);
            button.set_child(Some(&row));
            let ws = ws.clone();
            button.connect_clicked(move |_| {
                if let Some(ws) = ws.upgrade() { ws.open_path(path.clone()); }
            });
            self.scripts.append(&button);
        }
    }

    fn update_sidebar(&self) {
        // Script names are updated after save; refreshing the list is cheap.
    }

    fn refresh_profiles(self: &Rc<Self>) {
        while let Some(child) = self.profiles_body.first_child() {
            self.profiles_body.remove(&child);
        }
        let header = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let title = gtk::Label::new(Some("PROFILES"));
        title.add_css_class("popover-title");
        title.set_xalign(0.0);
        title.set_hexpand(true);
        header.append(&title);
        let refresh = gtk::Button::with_label("Refresh");
        refresh.add_css_class("pop-action");
        header.append(&refresh);
        self.profiles_body.append(&header);

        let (ready_count, waiting, connecting) = {
            let instances = self.instances.borrow();
            (
                instances.iter().filter(|item| item.ready.get()).count(),
                instances
                    .iter()
                    .any(|item| item.status.borrow().as_str() == "waiting"),
                instances
                    .iter()
                    .any(|item| item.status.borrow().as_str() == "connecting"),
            )
        };
        let status_text = if ready_count > 1 {
            format!("Attached to {ready_count} instances")
        } else if ready_count == 1 {
            "Attached · session ready".to_string()
        } else if waiting {
            "Connected · waiting for a game".to_string()
        } else if connecting {
            "Connecting to the profile socket…".to_string()
        } else {
            "No Relapse profile attached".to_string()
        };
        let status = gtk::Label::new(Some(&status_text));
        status.add_css_class("pop-status");
        status.set_xalign(0.0);
        self.profiles_body.append(&status);

        let sockets = profile_sockets();
        if sockets.is_empty() {
            let empty = gtk::Label::new(Some("No profile sockets found in /tmp. Start a Relapse profile first."));
            empty.add_css_class("settings-hint");
            empty.set_wrap(true);
            empty.set_xalign(0.0);
            self.profiles_body.append(&empty);
        }
        for path in sockets {
            let row = gtk::Box::new(gtk::Orientation::Horizontal, 8);
            row.set_margin_top(4);
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("instance-dot");
            let instance = self
                .instances
                .borrow()
                .iter()
                .find(|item| item.path == path)
                .cloned();
            if instance
                .as_ref()
                .map(|item| item.ready.get())
                .unwrap_or(false)
            {
                dot.add_css_class("on");
            }
            row.append(&dot);
            let name = gtk::Label::new(Some(&profile_name(&path)));
            name.add_css_class("pop-name");
            name.set_xalign(0.0);
            name.set_hexpand(true);
            row.append(&name);
            let label = if instance.is_some() { "Detach" } else { "Attach" };
            let action = gtk::Button::with_label(label);
            action.add_css_class("pop-action");
            if instance.is_none() { action.add_css_class("inject"); }
            let ws = Rc::downgrade(self);
            let path_for_toggle = path.clone();
            action.connect_clicked(move |_| {
                let Some(ws) = ws.upgrade() else { return };
                let attached = ws
                    .instances
                    .borrow()
                    .iter()
                    .any(|item| item.path == path_for_toggle);
                if attached {
                    ws.detach_profile(&path_for_toggle);
                } else {
                    ws.attach_profile(path_for_toggle.clone());
                }
            });
            row.append(&action);
            self.profiles_body.append(&row);
        }
        let reconnect = gtk::Button::with_label("Reconnect now");
        reconnect.add_css_class("pop-action");
        reconnect.set_halign(gtk::Align::Start);
        reconnect.set_margin_top(7);
        let ws = Rc::downgrade(self);
        reconnect.connect_clicked(move |_| {
            if let Some(ws) = ws.upgrade() {
                for instance in ws.instances.borrow().iter() {
                    instance.client.reconnect();
                }
                ws.toast("Reconnecting…");
            }
        });
        self.profiles_body.append(&reconnect);
        let ws = Rc::downgrade(self);
        refresh.connect_clicked(move |_| {
            if let Some(ws) = ws.upgrade() { ws.refresh_profiles(); }
        });

        let sep = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        sep.add_css_class("pop-divider");
        sep.set_margin_top(10);
        sep.set_margin_bottom(6);
        self.profiles_body.append(&sep);
    }

    fn attach_profile(self: &Rc<Self>, path: PathBuf) {
        {
            let mut instances = self.instances.borrow_mut();
            match instances.iter().find(|instance| instance.path == path) {
                Some(instance) => {
                    instance.status.replace("connecting".to_string());
                    instance.ready.set(false);
                    instance.session.set(0);
                    instance.client.reconnect();
                }
                None => instances.push(Rc::new(RelapseInstance {
                    path: path.clone(),
                    client: ProtocolClient::new(path.clone()),
                    status: RefCell::new("connecting".to_string()),
                    ready: Cell::new(false),
                    session: Cell::new(0),
                })),
            }
        }
        self.update_pill();
        self.toast(&format!("Attaching to {}…", profile_name(&path)));
    }

    fn detach_profile(self: &Rc<Self>, path: &PathBuf) {
        let removed = {
            let mut instances = self.instances.borrow_mut();
            match instances.iter().position(|instance| &instance.path == path) {
                Some(index) => Some(instances.remove(index)),
                None => None,
            }
        };
        if let Some(instance) = removed {
            instance.client.stop();
            self.update_pill();
            self.toast(&format!("Detached from {}", profile_name(path)));
        }
    }

    fn current_scheme(&self) -> Option<source::StyleScheme> {
        if self.theme_store.borrow().active.is_dark() {
            self.scheme_dark.clone()
        } else {
            self.scheme_light.clone()
        }
    }

    fn apply_theme(&self, theme: &theme::Theme) {
        let css = theme::build_css(theme);
        self.theme_provider.load_from_string(&css);
        let app_name = self.theme_store.borrow().app_name.clone();
        self.window.set_title(Some(&app_name));
        let scheme = if theme.is_dark() {
            self.scheme_dark.clone()
        } else {
            self.scheme_light.clone()
        };
        for tab in self.tabs.borrow().iter() {
            tab.buffer.set_style_scheme(scheme.as_ref());
        }
    }

    fn use_theme(self: &Rc<Self>, theme: theme::Theme) {
        {
            let mut store = self.theme_store.borrow_mut();
            store.use_theme(theme);
        }
        let active = self.theme_store.borrow().active.clone();
        self.apply_theme(&active);
        self.toast(format!("Theme · {}", active.name));
    }

    fn update_pill(&self) {
        let instances = self.instances.borrow();
        let ready: Vec<&Rc<RelapseInstance>> = instances
            .iter()
            .filter(|instance| instance.ready.get())
            .collect();
        let text = if ready.len() > 1 {
            format!("Attached · {} instances", ready.len())
        } else if let Some(instance) = ready.first() {
            format!("Attached · session {}", instance.session.get())
        } else if instances
            .iter()
            .any(|instance| instance.status.borrow().as_str() == "waiting")
        {
            "Waiting for game".to_string()
        } else if !instances.is_empty()
            && instances
                .iter()
                .all(|instance| instance.status.borrow().as_str() == "connecting")
        {
            "Connecting…".to_string()
        } else {
            "Disconnected".to_string()
        };
        self.pill_label.set_text(&text);
        if ready.is_empty() {
            self.pill.remove_css_class("attached-pill");
        } else {
            self.pill.add_css_class("attached-pill");
        }
        let names: Vec<String> = instances
            .iter()
            .map(|instance| profile_name(&instance.path))
            .collect();
        let profiles = if names.is_empty() {
            "No profile".to_string()
        } else {
            names.join(", ")
        };
        self.pill
            .set_tooltip_text(Some(&format!("{profiles} · {text}")));
    }

    fn handle_event(self: &Rc<Self>, instance: &Rc<RelapseInstance>, event: Event) {
        match event {
            Event::State { state, error, socket, ready, session } => {
                instance.status.replace(state.to_string());
                instance.ready.set(ready);
                instance.session.set(session);
                let profile = profile_name(&socket);
                if state == "ready" {
                    self.console_last.replace(String::new());
                    self.toast(&format!("Attached to {profile}"));
                } else if state == "waiting" {
                    self.console_last.replace(String::new());
                    self.toast(&format!("{profile}: waiting for a game"));
                }
                if let Some(error) = error {
                    self.console_append(&format!("error: {profile}: {error}"), false);
                }
                self.update_pill();
            }
            Event::Output { text } => {
                self.console_revealer.set_reveal_child(true);
                let multiple = self
                    .instances
                    .borrow()
                    .iter()
                    .filter(|item| item.ready.get())
                    .count()
                    > 1;
                if multiple {
                    self.console_append(
                        &format!("[{}] {text}", profile_name(&instance.path)),
                        true,
                    );
                } else {
                    self.console_append(&text, true);
                }
            }
            Event::Notice(text) => self.console_append(&text, false),
            Event::Error(text) => {
                self.console_revealer.set_reveal_child(true);
                let multiple = self
                    .instances
                    .borrow()
                    .iter()
                    .filter(|item| item.ready.get())
                    .count()
                    > 1;
                if multiple {
                    self.console_append(
                        &format!("error: {}: {text}", profile_name(&instance.path)),
                        false,
                    );
                } else {
                    self.console_append(&format!("error: {text}"), false);
                }
                self.toast(&text);
            }
        }
    }

    fn console_append(&self, text: &str, from_roblox: bool) {
        let visible = if from_roblox {
            self.filter_roblox.is_active()
        } else {
            self.filter_ui.is_active()
        };
        if !visible {
            return;
        }
        // Connection retries repeat the same status line once a second;
        // collapse consecutive duplicates from the UI side only, so script
        // output is never swallowed.
        if !from_roblox && self.console_last.borrow().as_str() == text {
            return;
        }
        self.console_last.replace(text.to_string());
        let buffer = self.console.buffer();
        let mut end = buffer.end_iter();
        buffer.insert(&mut end, &format!("{text}\n"));
        self.console.scroll_to_iter(&mut end, 0.0, false, 0.0, 1.0);
    }

    fn toast(&self, message: impl AsRef<str>) {
        self.toast.set_text(message.as_ref());
        self.toast_revealer.set_reveal_child(true);
        let revealer = self.toast_revealer.clone();
        glib::timeout_add_local_once(Duration::from_millis(2400), move || {
            revealer.set_reveal_child(false);
        });
    }

    fn launch_profile(&self) {
        let profile = std::env::var_os("RELAPSE_PROFILE")
            .unwrap_or_else(|| OsString::from(self.prefs.borrow().profile.clone()));
        let launcher = std::env::var_os("RELAPSE_LAUNCHER")
            .map(PathBuf::from)
            .or_else(|| {
                let path = self.prefs.borrow().launcher.clone();
                if path.trim().is_empty() {
                    None
                } else {
                    Some(PathBuf::from(path))
                }
            });
        self._launch_on_profile(launcher, &profile);
    }

    fn _launch_on_profile(&self, launcher: Option<PathBuf>, profile: &OsString) {
        let Some(launcher) = launcher else {
            self.toast("Set RELAPSE_LAUNCHER to the Relapse executable.");
            return;
        };
        if !launcher.exists() {
            self.toast("Relapse launcher not found. Set RELAPSE_LAUNCHER.");
            return;
        }
        // The launcher binds its Sober sandbox against the Flatpak document
        // portal directory. The portal does not always create it, and bwrap
        // then aborts with "Can't find source path". Create it best-effort.
        if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
            let portal = PathBuf::from(runtime).join("doc/by-app/org.vinegarhq.Sober");
            let _ = std::fs::create_dir_all(portal);
        }
        match std::process::Command::new(&launcher)
            .args(["--profile", profile.to_string_lossy().as_ref(), "--no-launch-ui"])
            .env_remove("LD_LIBRARY_PATH")
            .spawn()
        {
            Ok(_) => self.toast(format!("Starting {}…", profile.to_string_lossy())),
            Err(error) => self.toast(format!("Could not launch: {error}")),
        }
    }

    fn open_tab_menu(self: &Rc<Self>, tab: &Rc<Tab>, x: f64, y: f64) {
        let popover = gtk::Popover::new();
        popover.set_has_arrow(false);
        popover.add_css_class("instances-popover");
        let menu = gtk::Box::new(gtk::Orientation::Vertical, 1);
        menu.add_css_class("tab-menu");
        let rename = gtk::Button::with_label("Rename");
        let close = gtk::Button::with_label("Close");
        let pin = gtk::Button::with_label(if tab.pinned.get() { "Unpin tab" } else { "Pin tab" });
        let readonly = gtk::Button::with_label(if tab.read_only.get() { "Allow editing" } else { "Read-only tab" });
        let detach = gtk::Button::with_label("Open in new window");
        for button in [&rename, &close, &pin, &readonly, &detach] {
            button.add_css_class("tab-menu-item");
            menu.append(button);
        }
        popover.set_child(Some(&menu));
        popover.set_parent(&tab.widget);
        popover.set_pointing_to(Some(&gdk::Rectangle::new(x as i32, y as i32, 1, 1)));

        let weak = Rc::downgrade(self);
        let tab_for_rename = tab.clone();
        let popover_for_rename = popover.clone();
        rename.connect_clicked(move |_| {
            popover_for_rename.popdown();
            if let Some(ws) = weak.upgrade() { ws.rename_tab(&tab_for_rename); }
        });
        let weak = Rc::downgrade(self);
        let tab_for_close = tab.clone();
        let popover_for_close = popover.clone();
        close.connect_clicked(move |_| {
            popover_for_close.popdown();
            if let Some(ws) = weak.upgrade() { ws.close_tab(&tab_for_close); }
        });
        let weak = Rc::downgrade(self);
        let tab_for_pin = tab.clone();
        let popover_for_pin = popover.clone();
        pin.connect_clicked(move |_| {
            popover_for_pin.popdown();
            if let Some(ws) = weak.upgrade() { ws.toggle_pin(&tab_for_pin); }
        });
        let weak = Rc::downgrade(self);
        let tab_for_read = tab.clone();
        let popover_for_read = popover.clone();
        readonly.connect_clicked(move |_| {
            popover_for_read.popdown();
            if let Some(ws) = weak.upgrade() { ws.toggle_read_only(&tab_for_read); }
        });
        let weak = Rc::downgrade(self);
        let tab_for_detach = tab.clone();
        let popover_for_detach = popover.clone();
        detach.connect_clicked(move |_| {
            popover_for_detach.popdown();
            if let Some(ws) = weak.upgrade() { ws.open_tab_window(&tab_for_detach); }
        });
        popover.popup();
    }

    fn rename_tab(self: &Rc<Self>, tab: &Rc<Tab>) {
        let dialog = gtk::Window::builder()
            .title("Rename tab")
            .transient_for(&self.window)
            .modal(true)
            .default_width(360)
            .build();
        dialog.add_css_class("relapse-window");
        let root = gtk::Box::new(gtk::Orientation::Vertical, 10);
        root.set_margin_top(16);
        root.set_margin_bottom(16);
        root.set_margin_start(16);
        root.set_margin_end(16);
        let entry = gtk::Entry::new();
        entry.set_text(&tab.title.borrow());
        root.append(&entry);
        let actions = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        actions.set_halign(gtk::Align::End);
        let cancel = gtk::Button::with_label("Cancel");
        let accept = gtk::Button::with_label("Rename");
        accept.add_css_class("settings-launch");
        actions.append(&cancel);
        actions.append(&accept);
        root.append(&actions);
        dialog.set_child(Some(&root));
        let dialog_for_cancel = dialog.clone();
        cancel.connect_clicked(move |_| dialog_for_cancel.close());
        let dialog_for_accept = dialog.clone();
        let tab_for_accept = tab.clone();
        let entry_for_accept = entry.clone();
        accept.connect_clicked(move |_| {
            let name = entry_for_accept.text().trim().to_string();
            if !name.is_empty() {
                tab_for_accept.title.replace(name);
                tab_for_accept.name_label.set_text(&tab_for_accept.title.borrow());
            }
            dialog_for_accept.close();
        });
        let dialog_for_enter = dialog.clone();
        let tab_for_enter = tab.clone();
        entry.connect_activate(move |entry| {
            let name = entry.text().trim().to_string();
            if !name.is_empty() {
                tab_for_enter.title.replace(name);
                tab_for_enter.name_label.set_text(&tab_for_enter.title.borrow());
            }
            dialog_for_enter.close();
        });
        dialog.present();
        entry.grab_focus();
    }

    fn toggle_pin(self: &Rc<Self>, tab: &Rc<Tab>) {
        let pinned = !tab.pinned.get();
        tab.pinned.set(pinned);
        if !pinned {
            tab.widget.remove_css_class("pinned");
            self.toast("Unpinned tab");
            return;
        }
        tab.widget.add_css_class("pinned");
        {
            let mut tabs = self.tabs.borrow_mut();
            if let Some(index) = tabs.iter().position(|t| t.id == tab.id) {
                let pinned_tab = tabs.remove(index);
                tabs.insert(0, pinned_tab);
            }
            for item in tabs.iter() {
                self.tabs_box.remove(&item.widget);
            }
            for item in tabs.iter() {
                self.tabs_box.append(&item.widget);
            }
        }
        self.toast("Pinned tab to front");
    }

    fn toggle_read_only(self: &Rc<Self>, tab: &Rc<Tab>) {
        let read_only = !tab.read_only.get();
        tab.read_only.set(read_only);
        tab.view.set_editable(!read_only);
        if read_only {
            tab.widget.add_css_class("read-only");
        } else {
            tab.widget.remove_css_class("read-only");
        }
        if read_only {
            tab.completion.hide_now();
            self.toast("Read-only tab");
        } else {
            self.toast("Editing enabled");
        }
    }

    fn open_tab_window(self: &Rc<Self>, tab: &Rc<Tab>) {
        let window = gtk::Window::builder()
            .title(tab.title.borrow().clone())
            .default_width(840)
            .default_height(620)
            .build();
        window.add_css_class("relapse-window");
        let scroller = gtk::ScrolledWindow::new();
        scroller.set_policy(gtk::PolicyType::Automatic, gtk::PolicyType::Automatic);
        let view = source::View::with_buffer(&tab.buffer);
        configure_view(&view);
        view.set_editable(!tab.read_only.get());
        scroller.set_child(Some(&view));
        window.set_child(Some(&scroller));
        window.present();
        self.detached.borrow_mut().push(window);
    }
}

fn assets_dir() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("HOME"), "/.local/share")))
        .join("relapse-ui/gtksource");
    let _ = std::fs::create_dir_all(base.join("language-specs"));
    let _ = std::fs::create_dir_all(base.join("styles"));
    let _ = std::fs::write(base.join("language-specs/lua.lang"), LUA_LANG);
    let _ = std::fs::write(base.join("styles/relapse-dark.xml"), THEME);
    let _ = std::fs::write(base.join("styles/relapse-light.xml"), LIGHT_THEME);
    base
}

fn traffic_button(color: &str, glyph: &str, _window: &ApplicationWindow) -> gtk::Button {
    let button = gtk::Button::new();
    button.add_css_class("traffic");
    button.add_css_class(color);
    button.set_has_frame(false);
    button.set_focus_on_click(false);
    let label = gtk::Label::new(Some(glyph));
    label.add_css_class("traffic-glyph");
    button.set_child(Some(&label));
    button
}

fn configure_view(view: &source::View) {
    view.add_css_class("editor");
    view.set_show_line_numbers(true);
    view.set_highlight_current_line(true);
    view.set_auto_indent(true);
    view.set_tab_width(4);
    view.set_insert_spaces_instead_of_tabs(true);
    view.set_wrap_mode(gtk::WrapMode::None);
    view.set_pixels_above_lines(6);
    view.set_pixels_below_lines(6);
    view.set_left_margin(8);
    view.set_right_margin(16);
    view.set_top_margin(8);
    view.set_bottom_margin(12);
    view.set_vexpand(true);
    view.set_hexpand(true);
}

fn main() -> glib::ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_startup(|_| {
        let provider = gtk::CssProvider::new();
        provider.load_from_string(&format!("{STYLE}\n{EXTRA_STYLE}"));
        if let Some(display) = gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER,
            );
        }
    });
    app.connect_activate(|app| {
        Workspace::new(app);
    });
    app.run()
}
