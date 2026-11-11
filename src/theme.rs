//! Theme model, persistence and generated CSS override.

use serde_json::Value;
use std::path::PathBuf;

pub const DEFAULT_APP_NAME: &str = "Relapse +";
pub const DEFAULT_FONT: &str = "Adwaita Sans";
pub const DEFAULT_MONO: &str = "Adwaita Mono";

#[derive(Clone, Debug)]
pub struct Theme {
    pub id: String,
    pub name: String,
    pub background: String,
    pub surface: String,
    pub surface_alt: String,
    pub text: String,
    pub muted: String,
    pub border: String,
    pub accent: String,
    pub accent_text: String,
    pub editor_bg: String,
    pub success: String,
    pub error: String,
    pub font_family: String,
    pub mono_family: String,
    pub font_size: i32,
    pub header_image: String,
    pub background_image: String,
    pub builtin: bool,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            id: "preset-graphite".to_string(),
            name: "Graphite".to_string(),
            background: "#1e1e1e".to_string(),
            surface: "#242424".to_string(),
            surface_alt: "#2a2a2a".to_string(),
            text: "#cfcfcf".to_string(),
            muted: "#8d8d8d".to_string(),
            border: "#303030".to_string(),
            accent: "#3d78bd".to_string(),
            accent_text: "#dbe8fb".to_string(),
            editor_bg: "#1e1e1e".to_string(),
            success: "#63d38c".to_string(),
            error: "#df8585".to_string(),
            font_family: DEFAULT_FONT.to_string(),
            mono_family: DEFAULT_MONO.to_string(),
            font_size: 12,
            header_image: String::new(),
            background_image: String::new(),
            builtin: true,
        }
    }
}

impl Theme {
    pub fn is_dark(&self) -> bool {
        luminance(&self.background) < 0.5
    }

    fn from_value(value: &Value, builtin: bool) -> Self {
        let base = Theme::default();
        let text = |key: &str, fallback: &str| {
            value
                .get(key)
                .and_then(|item| item.as_str())
                .map(|item| item.to_string())
                .filter(|item| !item.trim().is_empty())
                .unwrap_or_else(|| fallback.to_string())
        };
        let color = |key: &str, fallback: &str| normalize_color(&text(key, fallback), fallback);
        let size = value
            .get("font_size")
            .and_then(|item| item.as_i64())
            .unwrap_or(base.font_size as i64)
            .clamp(9, 20) as i32;
        Self {
            id: text(
                "id",
                &format!("theme-{}", timestamp_id()),
            ),
            name: text("name", "Custom theme"),
            background: color("background", &base.background),
            surface: color("surface", &base.surface),
            surface_alt: color("surface_alt", &base.surface_alt),
            text: color("text", &base.text),
            muted: color("muted", &base.muted),
            border: color("border", &base.border),
            accent: color("accent", &base.accent),
            accent_text: color("accent_text", &base.accent_text),
            editor_bg: color("editor_bg", &base.editor_bg),
            success: color("success", &base.success),
            error: color("error", &base.error),
            font_family: sanitize_font(&text("font_family", DEFAULT_FONT)),
            mono_family: sanitize_font(&text("mono_family", DEFAULT_MONO)),
            font_size: size,
            header_image: text("header_image", ""),
            background_image: text("background_image", ""),
            builtin,
        }
    }

    fn to_value(&self) -> Value {
        serde_json::json!({
            "id": self.id,
            "name": self.name,
            "background": self.background,
            "surface": self.surface,
            "surface_alt": self.surface_alt,
            "text": self.text,
            "muted": self.muted,
            "border": self.border,
            "accent": self.accent,
            "accent_text": self.accent_text,
            "editor_bg": self.editor_bg,
            "success": self.success,
            "error": self.error,
            "font_family": self.font_family,
            "mono_family": self.mono_family,
            "font_size": self.font_size,
            "header_image": self.header_image,
            "background_image": self.background_image,
        })
    }
}

pub fn presets() -> Vec<Theme> {
    vec![
        Theme::default(),
        Theme {
            id: "preset-midnight".to_string(),
            name: "Midnight".to_string(),
            background: "#0f141b".to_string(),
            surface: "#171e27".to_string(),
            surface_alt: "#202a36".to_string(),
            text: "#dce6f2".to_string(),
            muted: "#7d8b9c".to_string(),
            border: "#2a3644".to_string(),
            accent: "#5b9bd5".to_string(),
            accent_text: "#f4f9ff".to_string(),
            editor_bg: "#0d1117".to_string(),
            success: "#67d391".to_string(),
            error: "#e08585".to_string(),
            font_family: "Inter".to_string(),
            mono_family: DEFAULT_MONO.to_string(),
            ..Theme::default()
        },
        Theme {
            id: "preset-frost".to_string(),
            name: "Frost".to_string(),
            background: "#f3f4f6".to_string(),
            surface: "#ffffff".to_string(),
            surface_alt: "#e9ebef".to_string(),
            text: "#1f2328".to_string(),
            muted: "#6b7280".to_string(),
            border: "#d6dae1".to_string(),
            accent: "#3b6fa0".to_string(),
            accent_text: "#ffffff".to_string(),
            editor_bg: "#ffffff".to_string(),
            success: "#1f7a4d".to_string(),
            error: "#b23b3b".to_string(),
            font_family: "Inter".to_string(),
            mono_family: DEFAULT_MONO.to_string(),
            ..Theme::default()
        },
        Theme {
            id: "preset-forest".to_string(),
            name: "Forest".to_string(),
            background: "#111a15".to_string(),
            surface: "#172219".to_string(),
            surface_alt: "#203024".to_string(),
            text: "#d6e8db".to_string(),
            muted: "#829888".to_string(),
            border: "#2b3d30".to_string(),
            accent: "#4f9e6b".to_string(),
            accent_text: "#effff4".to_string(),
            editor_bg: "#0e1511".to_string(),
            success: "#6fd39a".to_string(),
            error: "#d47f7f".to_string(),
            ..Theme::default()
        },
        Theme {
            id: "preset-ember".to_string(),
            name: "Ember".to_string(),
            background: "#1b1513".to_string(),
            surface: "#241c19".to_string(),
            surface_alt: "#2e241f".to_string(),
            text: "#eadfd6".to_string(),
            muted: "#a08b7e".to_string(),
            border: "#3a2c26".to_string(),
            accent: "#c47a4a".to_string(),
            accent_text: "#fff7f0".to_string(),
            editor_bg: "#16100e".to_string(),
            success: "#8fbc8f".to_string(),
            error: "#e08484".to_string(),
            ..Theme::default()
        },
    ]
}

pub fn config_dir() -> PathBuf {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("HOME"), "/.config")))
        .join("relapse-ui")
}

pub struct ThemeStore {
    pub app_name: String,
    pub themes: Vec<Theme>,
    pub active: Theme,
}

impl ThemeStore {
    pub fn load() -> Self {
        let path = config_dir().join("themes.json");
        let mut store = Self {
            app_name: DEFAULT_APP_NAME.to_string(),
            themes: Vec::new(),
            active: Theme::default(),
        };
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return store;
        };
        let Ok(data) = serde_json::from_str::<Value>(&raw) else {
            return store;
        };
        if let Some(name) = data.get("app_name").and_then(|value| value.as_str()) {
            if !name.trim().is_empty() {
                store.app_name = name.trim().to_string();
            }
        }
        if store.app_name == "Relapse" {
            store.app_name = DEFAULT_APP_NAME.to_string();
            store.save();
        }
        if let Some(list) = data.get("themes").and_then(|value| value.as_array()) {
            store.themes = list
                .iter()
                .map(|item| Theme::from_value(item, false))
                .collect();
        }
        if let Some(active) = data.get("active") {
            store.active = Theme::from_value(active, false);
        }
        store
    }

    pub fn save(&self) {
        let dir = config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let payload = serde_json::json!({
            "version": 1,
            "app_name": self.app_name,
            "active": self.active.to_value(),
            "themes": self.themes.iter().map(|theme| theme.to_value()).collect::<Vec<_>>(),
        });
        let _ = std::fs::write(
            dir.join("themes.json"),
            serde_json::to_string_pretty(&payload).unwrap_or_default() + "\n",
        );
    }

    pub fn use_theme(&mut self, theme: Theme) {
        let mut next = theme;
        if next.builtin || next.id.starts_with("preset-") {
            next.id = timestamp_id();
        }
        next.builtin = false;
        self.active = next;
        self.save();
    }

    pub fn delete(&mut self, id: &str) {
        self.themes.retain(|theme| theme.id != id);
        if self.active.id == id {
            self.active = Theme::default();
            self.active.id = timestamp_id();
            self.active.builtin = false;
        }
        self.save();
    }

}

#[derive(Clone)]
pub struct Prefs {
    pub autocomplete: bool,
    pub launcher: String,
    pub profile: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            autocomplete: true,
            launcher: String::new(),
            profile: "Default".to_string(),
        }
    }
}

impl Prefs {
    pub fn load() -> Self {
        let path = config_dir().join("settings.json");
        let Ok(raw) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let Ok(data) = serde_json::from_str::<Value>(&raw) else {
            return Self::default();
        };
        let mut prefs = Self::default();
        if let Some(flag) = data.get("autocomplete").and_then(|value| value.as_bool()) {
            prefs.autocomplete = flag;
        }
        if let Some(path) = data.get("launcher").and_then(|value| value.as_str()) {
            prefs.launcher = path.to_string();
        }
        if let Some(profile) = data.get("profile").and_then(|value| value.as_str()) {
            if !profile.trim().is_empty() {
                prefs.profile = profile.to_string();
            }
        }
        prefs
    }

    pub fn save(&self) {
        let dir = config_dir();
        let _ = std::fs::create_dir_all(&dir);
        let payload = serde_json::json!({
            "autocomplete": self.autocomplete,
            "launcher": self.launcher,
            "profile": self.profile,
        });
        let _ = std::fs::write(
            dir.join("settings.json"),
            serde_json::to_string_pretty(&payload).unwrap_or_default() + "\n",
        );
    }
}

fn timestamp_id() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    format!("{:x}{:x}", now.as_secs(), now.subsec_nanos())
}

fn sanitize_font(value: &str) -> String {
    let cleaned: String = value
        .replace('"', "")
        .replace(';', "")
        .chars()
        .take(80)
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        DEFAULT_FONT.to_string()
    } else {
        trimmed.to_string()
    }
}

fn normalize_color(value: &str, fallback: &str) -> String {
    let text = value.trim();
    let digits = text.strip_prefix('#').unwrap_or("");
    let valid = (digits.len() == 6 || digits.len() == 3)
        && digits.chars().all(|item| item.is_ascii_hexdigit());
    if !valid {
        return fallback.to_string();
    }
    if digits.len() == 3 {
        let expanded: String = digits.chars().flat_map(|item| [item, item]).collect();
        format!("#{expanded}")
    } else {
        format!("#{digits}")
    }
}

fn channels(hex: &str) -> (f64, f64, f64) {
    let digits = hex.trim_start_matches('#');
    let parse = |range: std::ops::Range<usize>| {
        u8::from_str_radix(&digits[range], 16).unwrap_or(0) as f64 / 255.0
    };
    (parse(0..2), parse(2..4), parse(4..6))
}

fn luminance(hex: &str) -> f64 {
    let (red, green, blue) = channels(&normalize_color(hex, "#000000"));
    let adjust = |value: f64| {
        if value <= 0.03928 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * adjust(red) + 0.7152 * adjust(green) + 0.0722 * adjust(blue)
}

fn mix(top: &str, bottom: &str, amount: f64) -> String {
    let (top_r, top_g, top_b) = channels(&normalize_color(top, "#000000"));
    let (low_r, low_g, low_b) = channels(&normalize_color(bottom, "#000000"));
    let blend = |high: f64, low: f64| -> u8 {
        ((high * amount + low * (1.0 - amount)) * 255.0).round() as u8
    };
    format!(
        "#{:02x}{:02x}{:02x}",
        blend(top_r, low_r),
        blend(top_g, low_g),
        blend(top_b, low_b)
    )
}

fn image_css(path: &str) -> String {
    if path.trim().is_empty() {
        return "none".to_string();
    }
    let path = PathBuf::from(path.trim());
    if !path.is_file() {
        return "none".to_string();
    }
    format!("url(\"{}\")", path.display())
}

pub fn build_css(theme: &Theme) -> String {
    let background = normalize_color(&theme.background, "#1e1e1e");
    let surface = normalize_color(&theme.surface, "#242424");
    let alt = normalize_color(&theme.surface_alt, "#2a2a2a");
    let text = normalize_color(&theme.text, "#cfcfcf");
    let muted = normalize_color(&theme.muted, "#8d8d8d");
    let border = normalize_color(&theme.border, "#303030");
    let accent = normalize_color(&theme.accent, "#3d78bd");
    let accent_text = normalize_color(&theme.accent_text, "#dbe8fb");
    let editor_bg = normalize_color(&theme.editor_bg, "#1e1e1e");
    let success = normalize_color(&theme.success, "#63d38c");
    let error = normalize_color(&theme.error, "#df8585");
    let size = theme.font_size.clamp(9, 20);
    let header_image = image_css(&theme.header_image);
    let editor_image = image_css(&theme.background_image);
    let editor_layer = if editor_image == "none" {
        editor_bg.clone()
    } else {
        format!("alpha({editor_bg}, 0.55)")
    };
    let header_rule = if header_image == "none" {
        "background-image: none; box-shadow: none;".to_string()
    } else {
        format!("background-image: {header_image}; background-size: cover; background-position: center; box-shadow: inset 0 0 0 2000px alpha({background}, 0.62);")
    };

    let css = r#"
window.relapse-window,
window.settings-window {
  background-color: %BG%;
  color: %TEXT%;
  font-family: "%FONT%", sans-serif;
  font-size: %SIZE%px;
}
.titlebar-inner {
  background-color: %SURFACE%;
  border-bottom-color: %BORDER%;
  %HEADER%
}
.window-title { color: %TEXT%; }
.title-launch {
  background-color: %ACCENT_BG%;
  border-color: %ACCENT%;
  color: %ACCENT_TEXT%;
}
.title-launch:hover { background-color: %ACCENT_HOVER%; }

.tabbar { background-color: %BG%; border-bottom-color: %BORDER%; }
.tab { border-right-color: %BORDER%; }
.tab:hover { background-color: %SURFACE%; }
.tab.active { background-color: %BG%; box-shadow: inset 0 1px 0 %ACCENT%; }
.tab-icon, .tab-name { color: %MUTED%; }
.tab.active .tab-name { color: %TEXT%; }
.tab.active .tab-icon { color: %ACCENT%; }
.tab-close, .tab-new { color: %MUTED%; }
.tab-close:hover, .tab-new:hover { background-color: %ALT%; color: %TEXT%; }

.editor-scroll,
.editor-scroll viewport {
  background-color: %EDITOR_BG%;
  background-image: %EDITOR_IMAGE%;
  background-size: cover;
  background-position: center;
}
.editor,
.editor text {
  background-color: %EDITOR_LAYER%;
  color: %TEXT%;
  caret-color: %TEXT%;
  font-family: "%MONO%", monospace;
  font-size: %SIZE%px;
}

.scripts-panel { background-color: %SURFACE%; border-left-color: %BORDER%; }
.panel-title, .script-empty, .script-icon { color: %MUTED%; }
.script-row, .script-name { color: %TEXT%; }
.script-row:hover { background-color: %ALT%; }

.toolbar { background-color: %SURFACE%; border-top-color: %BORDER%; }
.tool-button { color: %MUTED%; }
.tool-button:hover { background-color: %ALT%; color: %TEXT%; }
.exec-button {
  background-color: %ACCENT_BG%;
  border-color: %ACCENT%;
  color: %ACCENT_TEXT%;
}
.exec-button:hover { background-color: %ACCENT_HOVER%; }

.instance-pill > button {
  background-color: %ALT%;
  border-color: %BORDER%;
  color: %TEXT%;
}
.instance-pill > button:hover { background-color: %ACCENT_HOVER%; border-color: %ACCENT%; }
.instance-pill.attached-pill > button {
  background-color: %SUCCESS_BG%;
  border-color: %SUCCESS%;
  color: %TEXT%;
}
.pill-chevron { color: %MUTED%; }
.attached-pill .pill-chevron { color: %SUCCESS%; }

popover.instances-popover,
popover.instances-popover > contents,
popover.completion-popover,
popover.completion-popover > contents,
.completion-panel {
  background-color: %SURFACE%;
  border-color: %BORDER%;
}
.popover-title, .pop-footer, .pop-meta, .pop-status, .completion-hint { color: %MUTED%; }
.pop-status.ok { color: %SUCCESS%; }
.pop-status.bad { color: %ERROR%; }
.pop-name { color: %TEXT%; }
.pop-divider { background-color: %BORDER%; }
.instance-dot { background-color: %MUTED%; }
.instance-dot.on { background-color: %SUCCESS%; }
.pop-action { background-color: %ALT%; border-color: %BORDER%; color: %TEXT%; }
.pop-action:hover { background-color: %SURFACE%; }
.pop-action.inject { border-color: %ACCENT%; color: %ACCENT%; }
.completion-label { color: %TEXT%; }
.completion-row.selected { background-color: %ACCENT_BG%; }
.completion-row.selected .completion-label { color: %ACCENT_TEXT%; }

.console-panel { background-color: %EDITOR_BG%; border-top-color: %BORDER%; }
.console-head { background-color: %SURFACE%; border-bottom-color: %BORDER%; }
.console-title, .console-note, .console-action { color: %MUTED%; }
.console-action:hover { background-color: %ALT%; color: %TEXT%; }
.console-text {
  background-color: %EDITOR_BG%;
  color: %TEXT%;
  font-family: "%MONO%", monospace;
}
.toast { background-color: %ALT%; border-color: %BORDER%; color: %TEXT%; }

.settings-scroll, .settings-body { background-color: %BG%; }
.settings-label { color: %MUTED%; }
.settings-card { background-color: %SURFACE%; border-color: %BORDER%; }
.settings-divider { background-color: %BORDER%; }
.settings-key, .settings-hint, .settings-footer { color: %MUTED%; }
.settings-value, .settings-value-title, .settings-path { color: %TEXT%; }
.settings-value.mono { color: %TEXT%; font-family: "%MONO%", monospace; }
.settings-value-title.bad { color: %ERROR%; }
.status-dot { background-color: %MUTED%; }
.status-dot.ok { background-color: %SUCCESS%; }
.status-dot.bad { background-color: %ERROR%; }
.settings-inline-button, .settings-done {
  background-color: %ALT%;
  border-color: %BORDER%;
  color: %TEXT%;
}
.settings-inline-button:hover, .settings-done:hover {
  background-color: %SURFACE%;
  border-color: %ACCENT%;
  color: %TEXT%;
}
.settings-switch trough { background-color: %BORDER%; }
.settings-switch:checked trough { background-color: %ACCENT%; }
.settings-switch slider { background-color: %TEXT%; }
.theme-swatches { color: %MUTED%; }
.theme-row-active { color: %SUCCESS%; }

entry, spinbutton {
  background-color: %EDITOR_BG%;
  color: %TEXT%;
  border-color: %BORDER%;
}

/* Everything below used to keep its stock Adwaita grey with a custom theme. */
.settings-titlebar {
  background-color: %SURFACE%;
  border-bottom-color: %BORDER%;
}
.settings-titlebar .settings-header-title { color: %TEXT%; }
.settings-titlebar .settings-header-subtitle { color: %MUTED%; }

.editor gutter,
.editor .line-numbers {
  background-color: %EDITOR_BG%;
  color: %MUTED%;
}
.editor .current-line,
.editor .current-line-number {
  background-color: %ALT%;
}
.editor border { background-color: %EDITOR_BG%; }

textview,
textview text {
  background-color: %EDITOR_BG%;
  color: %TEXT%;
}

.console-filter {
  background-color: %ALT%;
  border-color: %BORDER%;
  color: %MUTED%;
}
.console-filter:hover { color: %TEXT%; }
.console-filter:checked {
  background-color: %ACCENT_BG%;
  border-color: %ACCENT%;
  color: %ACCENT_TEXT%;
}

.settings-path-field {
  background-color: %EDITOR_BG%;
  border-color: %BORDER%;
}
.trace-scroll,
.trace-text {
  background-color: %EDITOR_BG%;
  color: %TEXT%;
  border-color: %BORDER%;
}
.settings-launch {
  background-color: %ACCENT_BG%;
  border-color: %ACCENT%;
  color: %ACCENT_TEXT%;
}
.settings-launch:hover { background-color: %ACCENT_HOVER%; }
.settings-launch-status { color: %MUTED%; }
.settings-launch-status.ok { color: %SUCCESS%; }
.settings-launch-status.bad { color: %ERROR%; }

list,
listview,
.completion-list,
.completion-list > row {
  background-color: %SURFACE%;
  color: %TEXT%;
}

checkbutton { color: %TEXT%; }
checkbutton check {
  background-color: %ALT%;
  border-color: %BORDER%;
}
checkbutton:checked check {
  background-color: %ACCENT%;
  border-color: %ACCENT%;
  color: %ACCENT_TEXT%;
}

.client-cookie { color: %SUCCESS%; }
entry:focus, spinbutton:focus { border-color: %ACCENT%; }

scrollbar slider { background-color: %BORDER%; }
scrollbar slider:hover { background-color: %ACCENT%; }

tooltip { background-color: %ALT%; color: %TEXT%; border-color: %BORDER%; }
"#;

    css.replace("%BG%", &background)
        .replace("%SURFACE%", &surface)
        .replace("%ALT%", &alt)
        .replace("%TEXT%", &text)
        .replace("%MUTED%", &muted)
        .replace("%BORDER%", &border)
        .replace("%ACCENT_BG%", &mix(&accent, &background, 0.78))
        .replace("%ACCENT_HOVER%", &mix(&accent, &background, 0.68))
        .replace("%ACCENT_TEXT%", &accent_text)
        .replace("%ACCENT%", &accent)
        .replace("%SUCCESS_BG%", &mix(&success, &surface, 0.72))
        .replace("%SUCCESS%", &success)
        .replace("%ERROR%", &error)
        .replace("%EDITOR_LAYER%", &editor_layer)
        .replace("%EDITOR_IMAGE%", &editor_image)
        .replace("%EDITOR_BG%", &editor_bg)
        .replace("%FONT%", &sanitize_font(&theme.font_family))
        .replace("%MONO%", &sanitize_font(&theme.mono_family))
        .replace("%SIZE%", &size.to_string())
        .replace("%HEADER%", &header_rule)
}
