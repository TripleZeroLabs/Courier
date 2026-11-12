//! Courier themes: overrides of Facebook's own CSS variables, plus an
//! optional light/dark mode and free-form custom CSS.
//!
//! Facebook styles nearly everything through named CSS variables
//! (`--surface-background`, `--chat-bubble-padding-horizontal`, ...), defined
//! on `<html class="__fb-light-mode">` / `__fb-dark-mode` and on nested
//! elements that force one mode. Overriding them there, with `!important`,
//! restyles the whole app without touching Facebook's generated class names.
//!
//! The settings window only ever sends token *keys* from the tables below
//! and validated values; the variable names and CSS are produced here.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const MODES: &[&str] = &["facebook", "light", "dark"];

/// (key, label, Facebook variables it sets)
pub const COLOR_TOKENS: &[(&str, &str, &[&str])] = &[
    ("accent", "Accent (buttons, links, send, toggles)", &[
        "--accent", "--primary-button-background", "--buttonSendBackgroundColor", "--cursor",
        "--radio-border-color-selected", "--radio-checked-icon-color", "--toggle-active-background",
        "--switch-active", "--progress-ring-blue-foreground", "--mwp-header-button-color",
        "--primary-deemphasized-button-text", "--primary-deemphasized-button-icon", "--blue-link",
        "--text-input-active-text", "--input-label-color-highlighted", "--story-unseen",
        "--badge-background-color-blue",
    ]),
    ("outgoing_bubble", "Your message bubbles", &["--chat-outgoing-message-bubble-background-color"]),
    ("incoming_bubble", "Their message bubbles", &["--chat-incoming-message-bubble-background-color"]),
    ("background", "Window background", &["--web-wash", "--wash"]),
    ("surface", "Panels (chat list, conversation, cards)", &[
        "--surface-background", "--messenger-card-background", "--card-background",
        "--nav-bar-background", "--mwp-header-background-color", "--mwp-message-row-background",
        "--popover-card-background", "--toast-background",
    ]),
    ("input", "Text fields and message box", &[
        "--input-background", "--input-background-hover", "--text-input-bar-background",
    ]),
    ("text", "Text and icons", &["--primary-text", "--primary-icon", "--fds-primary-text", "--fds-primary-icon"]),
    ("secondary_text", "Secondary text and icons", &[
        "--secondary-text", "--secondary-icon", "--placeholder-text", "--placeholder-icon",
        "--section-header-text",
    ]),
    ("divider", "Dividers and borders", &["--divider", "--media-outer-border", "--input-border-color"]),
    ("focus", "Keyboard focus ring", &["--focus-ring-blue"]),
    ("badge", "Unread badges", &["--notification-badge", "--badge-background-color-red"]),
];

/// (key, label, Facebook variables, min, max, Facebook's default) in px.
pub const SIZE_TOKENS: &[(&str, &str, &[&str], u32, u32, u32)] = &[
    ("bubble_radius", "Message bubble corners", &["--chat-bubble-border-radius"], 0, 30, 18),
    ("bubble_padding_x", "Message bubble padding (sides)", &["--chat-bubble-padding-horizontal"], 2, 32, 12),
    ("bubble_padding_y", "Message bubble padding (top/bottom)", &["--chat-bubble-padding-vertical"], 0, 24, 8),
    ("panel_spacing", "Space around panels", &["--messenger-card-spacing"], 0, 40, 16),
    ("panel_radius", "Panel and dialog corners", &[
        "--card-corner-radius", "--messenger-card-corner-radius", "--dialog-corner-radius",
        "--popover-corner-radius", "--card-small-corner-radius", "--card-large-corner-radius",
    ], 0, 32, 12),
    ("list_row_radius", "Chat list row corners", &["--nav-list-cell-corner-radius", "--list-cell-corner-radius"], 0, 24, 8),
    ("button_radius", "Button corners", &[
        "--button-corner-radius", "--button-corner-radius-medium", "--button-corner-radius-large",
        "--button-small-corner-radius", "--button-large-corner-radius",
    ], 0, 24, 8),
    ("input_radius", "Text field corners", &["--input-corner-radius", "--input-field-expanded-corner-radius"], 0, 32, 12),
    ("media_radius", "Photo and video corners", &["--media-corner-radius", "--media-small-corner-radius", "--image-corner-radius"], 0, 32, 12),
    ("border_width", "Text field border width", &["--input-border-width"], 0, 4, 1),
    ("drawer_width", "Chat list drawer width", &["--courier-drawer-width"], 240, 600, 360),
];

pub const MAX_CUSTOM_CSS: usize = 20_000;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Theme {
    /// "facebook" (follow your Facebook setting), "light" or "dark".
    pub mode: String,
    /// Color token key → "#rrggbb" or "#rrggbbaa". Missing = Facebook's.
    pub colors: BTreeMap<String, String>,
    /// Size token key → px. Missing = Facebook's.
    pub sizes: BTreeMap<String, u32>,
    /// Extra CSS, applied after everything else.
    pub custom_css: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self { mode: "facebook".into(), colors: BTreeMap::new(), sizes: BTreeMap::new(), custom_css: String::new() }
    }
}

fn is_hex_color(s: &str) -> bool {
    let Some(h) = s.strip_prefix('#') else { return false };
    (h.len() == 6 || h.len() == 8) && h.bytes().all(|b| b.is_ascii_hexdigit())
}

impl Theme {
    /// Drops unknown keys and invalid values; clamps sizes; caps custom CSS.
    pub fn sanitize(mut self) -> Self {
        if !MODES.contains(&self.mode.as_str()) {
            self.mode = "facebook".into();
        }
        self.colors.retain(|k, v| {
            *v = v.trim().to_ascii_lowercase();
            COLOR_TOKENS.iter().any(|t| t.0 == k) && is_hex_color(v)
        });
        self.sizes.retain(|k, _| SIZE_TOKENS.iter().any(|t| t.0 == k));
        for (k, v) in self.sizes.iter_mut() {
            if let Some(t) = SIZE_TOKENS.iter().find(|t| t.0 == k) {
                *v = (*v).clamp(t.3, t.4);
            }
        }
        if self.custom_css.len() > MAX_CUSTOM_CSS {
            let mut end = MAX_CUSTOM_CSS;
            while !self.custom_css.is_char_boundary(end) {
                end -= 1;
            }
            self.custom_css.truncate(end);
        }
        self
    }

    /// The stylesheet injected into the page (empty when nothing is set).
    pub fn css(&self) -> String {
        let mut decls = String::new();
        for (key, value) in &self.colors {
            if let Some(t) = COLOR_TOKENS.iter().find(|t| t.0 == key) {
                for var in t.2 {
                    decls.push_str(&format!("  {var}: {value} !important;\n"));
                }
                if *key == "focus" {
                    decls.push_str(&format!("  --focus-ring-outline-link: 2px auto {value} !important;\n"));
                    decls.push_str(&format!("  --focus-ring-shadow-default: 0 0 0 2px {value} !important;\n"));
                }
            }
        }
        for (key, px) in &self.sizes {
            if let Some(t) = SIZE_TOKENS.iter().find(|t| t.0 == key) {
                for var in t.2 {
                    decls.push_str(&format!("  {var}: {px}px !important;\n"));
                }
            }
        }
        let mut css = String::new();
        if !decls.is_empty() {
            // Facebook defines its variables on these two classes (on <html>
            // and on nested elements that force a mode), so override there.
            css.push_str(":root, .__fb-light-mode, .__fb-dark-mode {\n");
            css.push_str(&decls);
            css.push_str("}\n");
        }
        if !self.custom_css.trim().is_empty() {
            css.push_str("/* custom CSS */\n");
            css.push_str(&self.custom_css);
            css.push('\n');
        }
        css
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes() {
        let mut t = Theme::default();
        t.mode = "neon".into();
        t.colors.insert("accent".into(), "#FF00AA".into());
        t.colors.insert("text".into(), "red; } body { display:none".into());
        t.colors.insert("nope".into(), "#000000".into());
        t.sizes.insert("bubble_radius".into(), 500);
        t.sizes.insert("nope".into(), 3);
        let t = t.sanitize();
        assert_eq!(t.mode, "facebook");
        assert_eq!(t.colors.len(), 1);
        assert_eq!(t.colors["accent"], "#ff00aa");
        assert_eq!(t.sizes.len(), 1);
        assert_eq!(t.sizes["bubble_radius"], 30);
    }

    #[test]
    fn generates_css() {
        assert_eq!(Theme::default().css(), "");
        let mut t = Theme::default();
        t.colors.insert("outgoing_bubble".into(), "#123456".into());
        t.sizes.insert("drawer_width".into(), 300);
        let css = t.css();
        assert!(css.contains("--chat-outgoing-message-bubble-background-color: #123456 !important;"));
        assert!(css.contains("--courier-drawer-width: 300px !important;"));
    }

    #[test]
    fn caps_custom_css() {
        let mut t = Theme::default();
        t.custom_css = "é".repeat(MAX_CUSTOM_CSS);
        assert!(t.sanitize().custom_css.len() <= MAX_CUSTOM_CSS);
    }
}
