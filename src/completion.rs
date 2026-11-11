use gtk::prelude::*;
use gtk::{gdk, glib};
use sourceview5 as source;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const COMPLETIONS: &[&str] = &[
    "and", "break", "continue", "do", "else", "elseif", "end", "false", "for",
    "function", "if", "in", "local", "nil", "not", "or", "repeat", "return",
    "then", "true", "until", "while", "assert", "collectgarbage", "error",
    "getmetatable", "ipairs", "loadstring", "newproxy", "next", "pairs", "pcall",
    "print", "rawequal", "rawget", "rawlen", "rawset", "require", "select",
    "setmetatable", "tonumber", "tostring", "type", "typeof", "unpack", "xpcall",
    "task", "wait", "spawn", "delay", "warn", "tick", "time", "elapsedTime",
    "game", "workspace", "script", "plugin", "shared", "Instance", "Vector2",
    "Vector3", "Vector2int16", "Vector3int16", "CFrame", "Color3", "ColorSequence",
    "ColorSequenceKeypoint", "NumberSequence", "NumberSequenceKeypoint", "Rect",
    "UDim", "UDim2", "Ray", "RaycastParams", "Region3", "Region3int16", "TweenInfo",
    "BrickColor", "Random", "DateTime", "Enum", "Axes", "Faces",
    "PhysicalProperties", "DockWidgetPluginGuiInfo", "PathWaypoint", "WebSocket",
    "TweenService", "RunService", "UserInputService", "Players", "ReplicatedStorage",
    "ServerStorage", "StarterGui", "Lighting", "Camera", "Humanoid", "BasePart",
    "Model", "Folder", "ScreenGui", "TextLabel", "Frame", "ImageLabel",
    "TextButton", "Sound", "Wait",
    "GetObjects", "cloneref", "compareinstances", "fireclickdetector",
    "fireproximityprompt", "firetouchinterest", "getcallbackvalue",
    "gethui", "getinstances", "getnilinstances", "isnetworkowner",
    "cache.invalidate", "cache.iscached", "cache.replace",
    "checkcaller", "clonefunction", "getfunctionhash",
    "hookfunction", "hookmetamethod", "iscclosure", "isexecutorclosure",
    "isfunctionhooked", "islclosure", "loadstring", "newcclosure", "restorefunction",
    "debug.getconstant", "debug.getconstants", "debug.getinfo", "debug.getproto",
    "debug.getprotos", "debug.getstack", "debug.getupvalue", "debug.getupvalues",
    "debug.setconstant", "debug.setstack", "debug.setupvalue",
    "Drawing.new", "cleardrawcache", "getrenderproperty", "isrenderobj",
    "setrenderproperty", "base64decode", "base64encode", "crypt.generatebytes",
    "crypt.generatekey", "crypt.hash", "crypt.encrypt", "crypt.decrypt", "crypt.hmac",
    "crypt.random", "lz4compress", "lz4decompress",
    "getgc", "getgcheapstats", "getgenv", "getreg", "getrenv", "filtergc",
    "appendfile", "delfile", "delfolder", "getcustomasset", "isfile",
    "isfolder", "listfiles", "loadfile", "makefolder", "readfile", "writefile",
    "getnamecallmethod", "getrawmetatable", "isreadonly", "makereadonly",
    "makewritable", "setnamecallmethod", "setrawmetatable", "setreadonly",
    "getfpscap", "identifyexecutor", "queue_on_teleport", "request", "setclipboard",
    "setfpscap", "settimescale",
    "gethiddenproperty", "getproperties", "getthreadidentity",
    "isscriptable", "sethiddenproperty", "setscriptable", "setthreadidentity",
    "decompile", "dumpstring", "getcallingscript", "getloadedmodules", "getmenv",
    "getrunningscripts", "getscriptbytecode", "getscriptclosure", "getscriptfromthread",
    "getscripthash", "getscripts", "getsenv",
    "cansignalreplicate", "firesignal",
    "getconnection", "getconnections", "getsignalarguments",
    "getsignalargumentsinfo", "getsignalwhitelist", "replicatesignal",
    "WebSocket.connect", "WebSocket:Send", "WebSocket:Close",
    "getactors", "run_on_actor", "isparallel", "create_comm_channel",
    "get_comm_channel", "RelapseChannel.new",
    "getfflag", "setfflag", "fflag_list",
    "HttpGet", "http.request",
    "mouse1click", "mouse1press", "mouse1release",
    "mouse2click", "mouse2press", "mouse2release",
    "mousemoveabs", "mousemoverel", "mousescroll",
    "keypress", "keyrelease", "keyclick",
    "isrbxactive", "messagebox",
    "oth.hook", "oth.unhook", "oth.get_root_callback", "oth.is_hook_thread",
    "get_original_thread", "gettimescale",
    "raknet.add_receive_hook", "raknet.add_send_hook",
    "raknet.remove_receive_hook", "raknet.remove_send_hook",
    "raknet.send", "raknet.receive", "raknet.is_enabled", "raknet.diagnostics",
    "RelapseSignal.new",
    "gethiddenproperty", "getscripts", "setfpscap",
];

struct CompletionState {
    view: source::View,
    buffer: source::Buffer,
    scroll: gtk::ScrolledWindow,
    panel: gtk::Box,
    list: gtk::ListBox,
    rows: RefCell<Vec<gtk::ListBoxRow>>,
    suggestions: RefCell<Vec<&'static str>>,
    selected: Cell<usize>,
    visible: Cell<bool>,
    prefix: RefCell<String>,
    suppress: Cell<bool>,
    enabled: Rc<Cell<bool>>,
}

pub struct Completion {
    state: Rc<CompletionState>,
}

impl Completion {
    pub fn attach(
        view: &source::View,
        buffer: &source::Buffer,
        scroll: &gtk::ScrolledWindow,
        overlay: &gtk::Overlay,
        enabled: Rc<Cell<bool>>,
    ) -> Rc<Self> {
        let panel = gtk::Box::new(gtk::Orientation::Vertical, 0);
        panel.add_css_class("completion-panel");
        panel.set_halign(gtk::Align::Start);
        panel.set_valign(gtk::Align::Start);
        panel.set_can_focus(false);
        panel.set_focusable(false);
        panel.set_visible(false);

        let list = gtk::ListBox::new();
        list.set_selection_mode(gtk::SelectionMode::None);
        list.set_can_focus(false);
        list.set_focusable(false);
        list.add_css_class("completion-list");

        let scroller = gtk::ScrolledWindow::new();
        scroller.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
        scroller.set_max_content_height(126);
        scroller.set_propagate_natural_height(true);
        scroller.set_child(Some(&list));

        let wrapper = gtk::Box::new(gtk::Orientation::Vertical, 0);
        wrapper.add_css_class("completion-wrap");
        wrapper.set_width_request(232);
        let hint = gtk::Label::new(Some("Luau  ·  ↑↓ select  ·  Tab complete"));
        hint.add_css_class("completion-hint");
        hint.set_xalign(0.0);
        wrapper.append(&hint);
        wrapper.append(&scroller);
        panel.append(&wrapper);
        overlay.add_overlay(&panel);

        let state = Rc::new(CompletionState {
            view: view.clone(),
            buffer: buffer.clone(),
            scroll: scroll.clone(),
            panel,
            list,
            rows: RefCell::new(Vec::new()),
            suggestions: RefCell::new(Vec::new()),
            selected: Cell::new(0),
            visible: Cell::new(false),
            prefix: RefCell::new(String::new()),
            suppress: Cell::new(false),
            enabled,
        });

        let weak = Rc::downgrade(&state);
        buffer.connect_changed(move |_| {
            if let Some(state) = weak.upgrade() {
                Self::refresh(&state);
            }
        });

        let weak = Rc::downgrade(&state);
        let keys = gtk::EventControllerKey::new();
        keys.set_propagation_phase(gtk::PropagationPhase::Capture);
        keys.connect_key_pressed(move |_, keyval, _, _| {
            let Some(state) = weak.upgrade() else {
                return glib::Propagation::Proceed;
            };
            if Self::handle_key(&state, keyval) {
                glib::Propagation::Stop
            } else {
                glib::Propagation::Proceed
            }
        });
        view.add_controller(keys);

        let weak = Rc::downgrade(&state);
        let focus = gtk::EventControllerFocus::new();
        focus.connect_leave(move |_| {
            if let Some(state) = weak.upgrade() {
                Self::hide(&state);
            }
        });
        view.add_controller(focus);

        Rc::new(Self { state })
    }

    pub fn hide_now(&self) {
        Self::hide(&self.state);
    }

    fn hide(state: &Rc<CompletionState>) {
        if state.visible.replace(false) {
            state.panel.set_visible(false);
        }
        state.prefix.replace(String::new());
    }

    fn refresh(state: &Rc<CompletionState>) {
        if state.suppress.get() || !state.view.is_editable() {
            return;
        }
        if !state.enabled.get() {
            Self::hide(state);
            return;
        }
        let buffer = &state.buffer;
        if buffer.has_selection() {
            Self::hide(state);
            return;
        }
        let insert = buffer.iter_at_mark(&buffer.get_insert());
        let mut start = insert;
        while start.backward_char() {
            let ch = start.char();
            if !(ch.is_alphanumeric() || ch == '_') {
                start.forward_char();
                break;
            }
        }
        let prefix = buffer.text(&start, &insert, false).to_string();
        if prefix.is_empty() || prefix.chars().count() > 40 {
            Self::hide(state);
            return;
        }
        if state.visible.get() && state.prefix.borrow().eq_ignore_ascii_case(&prefix) {
            return;
        }

        let lower = prefix.to_lowercase();
        let mut matches: Vec<&'static str> = COMPLETIONS
            .iter()
            .copied()
            .filter(|word| {
                word.to_lowercase().starts_with(&lower) && *word != prefix
            })
            .collect();
        matches.sort_by_key(|word| (word.to_lowercase(), *word));
        matches.dedup();
        matches.truncate(60);
        if matches.is_empty() {
            Self::hide(state);
            return;
        }

        while let Some(child) = state.list.first_child() {
            state.list.remove(&child);
        }
        state.rows.borrow_mut().clear();
        state.suggestions.replace(matches.clone());
        state.selected.set(0);
        state.prefix.replace(prefix);

        for (index, word) in matches.iter().enumerate() {
            let row = gtk::ListBoxRow::new();
            row.set_activatable(true);
            row.set_focusable(false);
            row.add_css_class("completion-row");
            let label = gtk::Label::new(Some(word));
            label.set_xalign(0.0);
            label.add_css_class("completion-label");
            row.set_child(Some(&label));
            let weak = Rc::downgrade(state);
            let click = gtk::GestureClick::new();
            click.connect_pressed(move |_, _, _, _| {
                if let Some(state) = weak.upgrade() {
                    Self::accept_at(&state, index);
                }
            });
            row.add_controller(click);
            state.list.append(&row);
            state.rows.borrow_mut().push(row);
        }
        Self::update_selection(state);
        if !state.visible.get() {
            state.visible.set(true);
            state.panel.set_visible(true);
        }
        Self::place(state, &insert);
    }

    fn update_selection(state: &Rc<CompletionState>) {
        let rows = state.rows.borrow();
        for (index, row) in rows.iter().enumerate() {
            if index == state.selected.get() {
                row.add_css_class("selected");
            } else {
                row.remove_css_class("selected");
            }
        }
    }

    fn place(state: &Rc<CompletionState>, iter: &gtk::TextIter) {
        let location = state.view.iter_location(iter);
        let (x, y) = state.view.buffer_to_window_coords(
            gtk::TextWindowType::Widget,
            location.x(),
            location.y(),
        );
        let local_x = x - state.scroll.hadjustment().value() as i32;
        let local_y = y - state.scroll.vadjustment().value() as i32;
        let width = state.scroll.width().max(1);
        let height = state.scroll.height().max(1);
        let rows = state.suggestions.borrow().len().min(5) as i32;
        let panel_width = 244;
        let panel_height = rows * 21 + 26;
        let left = local_x.clamp(0, (width - panel_width).max(0));
        let below = local_y + location.height();
        // Right under the caret; only move above the line when the editor
        // has no room left below it.
        let top = if below + panel_height <= height - 4 {
            below
        } else {
            (local_y - panel_height).max(0)
        };
        state.panel.set_margin_start(left);
        state.panel.set_margin_top(top);
    }

    fn handle_key(state: &Rc<CompletionState>, keyval: gdk::Key) -> bool {
        if !state.visible.get() {
            return false;
        }
        match keyval {
            gdk::Key::Up => {
                let count = state.suggestions.borrow().len();
                if count > 0 {
                    let next = (state.selected.get() + count - 1) % count;
                    state.selected.set(next);
                    Self::update_selection(state);
                }
                true
            }
            gdk::Key::Down => {
                let count = state.suggestions.borrow().len();
                if count > 0 {
                    state.selected.set((state.selected.get() + 1) % count);
                    Self::update_selection(state);
                }
                true
            }
            gdk::Key::Tab => {
                Self::accept_at(state, state.selected.get());
                true
            }
            gdk::Key::Escape => {
                Self::hide(state);
                true
            }
            _ => false,
        }
    }

    fn accept_at(state: &Rc<CompletionState>, index: usize) {
        let Some(word) = state.suggestions.borrow().get(index).copied() else {
            return;
        };
        let buffer = &state.buffer;
        let mut end = buffer.iter_at_mark(&buffer.get_insert());
        let mut start = end;
        while start.backward_char() {
            let ch = start.char();
            if !(ch.is_alphanumeric() || ch == '_') {
                start.forward_char();
                break;
            }
        }
        state.suppress.set(true);
        state.panel.set_visible(false);
        state.visible.set(false);
        buffer.begin_user_action();
        buffer.delete(&mut start, &mut end);
        buffer.insert(&mut end, word);
        buffer.place_cursor(&end);
        buffer.end_user_action();
        state.suppress.set(false);
        state.prefix.replace(String::new());
        state.view.grab_focus();
    }
}
