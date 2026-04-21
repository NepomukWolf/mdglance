use std::{
    collections::{BTreeMap, HashMap},
    env, fmt,
    path::PathBuf,
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use tao::keyboard::{Key, ModifiersState};

const PROJECT_CONFIG_NAME: &str = "mdglance.toml";

#[derive(Debug, Clone)]
pub struct Config {
    pub window: WindowConfig,
    keybindings: BTreeMap<Action, Vec<KeyBinding>>,
}

#[derive(Debug, Clone)]
pub struct WindowConfig {
    pub width: u32,
    pub height: u32,
    pub fullscreen: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    ScrollDown,
    ScrollUp,
    ScrollLeft,
    ScrollRight,
    HalfPageDown,
    HalfPageUp,
    PageDown,
    Top,
    Bottom,
    OpenSearch,
    AcceptSearch,
    NextSearchHit,
    PreviousSearchHit,
    ShowHelp,
    CloseOverlay,
    Quit,
}

#[derive(Debug, Clone, Serialize)]
pub struct KeyBinding {
    pub display: String,
    pub shortcut: Shortcut,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
pub struct Shortcut {
    pub key: String,
    pub shift: Option<bool>,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebConfig {
    pub keybindings: Vec<WebActionBinding>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WebActionBinding {
    pub action: Action,
    pub keys: Vec<String>,
    pub shortcuts: Vec<Shortcut>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    #[serde(default)]
    window: WindowOverrides,
    #[serde(default)]
    keybindings: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct WindowOverrides {
    width: Option<u32>,
    height: Option<u32>,
    fullscreen: Option<bool>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let source = config_source()?;
        let mut config = Self::default();

        let Some(source) = source else {
            return Ok(config);
        };

        let content = std::fs::read_to_string(&source)
            .with_context(|| format!("failed to read {}", source.display()))?;
        let file_config: FileConfig = toml::from_str(&content)
            .with_context(|| format!("failed to parse {}", source.display()))?;

        if let Some(width) = file_config.window.width {
            config.window.width = width;
        }
        if let Some(height) = file_config.window.height {
            config.window.height = height;
        }
        if let Some(fullscreen) = file_config.window.fullscreen {
            config.window.fullscreen = fullscreen;
        }

        for (name, shortcuts) in file_config.keybindings {
            let action = Action::from_config_key(&name).ok_or_else(|| {
                anyhow::anyhow!("unknown action `{name}` in {}", source.display())
            })?;
            let bindings = shortcuts
                .into_iter()
                .map(|shortcut| parse_shortcut(&shortcut))
                .collect::<Result<Vec<_>>>()?;
            config.keybindings.insert(action, bindings);
        }

        config
            .validate()
            .with_context(|| format!("invalid keybindings in {}", source.display()))?;

        Ok(config)
    }

    pub fn web_config(&self) -> WebConfig {
        let keybindings = Action::all()
            .iter()
            .map(|action| {
                let bindings = self.keybindings.get(action).cloned().unwrap_or_default();
                WebActionBinding {
                    action: *action,
                    keys: bindings
                        .iter()
                        .map(|binding| binding.display.clone())
                        .collect(),
                    shortcuts: bindings
                        .into_iter()
                        .map(|binding| binding.shortcut)
                        .collect(),
                }
            })
            .collect();

        WebConfig { keybindings }
    }

    pub fn bindings_for(&self, action: Action) -> &[KeyBinding] {
        self.keybindings
            .get(&action)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn validate(&self) -> Result<()> {
        let mut seen: HashMap<Shortcut, Action> = HashMap::new();

        for action in Action::all() {
            let Some(bindings) = self.keybindings.get(action) else {
                continue;
            };

            for binding in bindings {
                if let Some(previous) = seen.insert(binding.shortcut.clone(), *action) {
                    bail!(
                        "shortcut `{}` is assigned to both `{}` and `{}`",
                        binding.display,
                        previous.config_key(),
                        action.config_key()
                    );
                }
            }
        }

        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        let window = WindowConfig {
            width: 1080,
            height: 860,
            fullscreen: false,
        };

        let keybindings = default_keybindings()
            .into_iter()
            .map(|(action, displays)| {
                let bindings = displays
                    .into_iter()
                    .map(parse_shortcut)
                    .collect::<Result<Vec<_>>>()
                    .expect("default keybindings must parse");
                (action, bindings)
            })
            .collect();

        Self {
            window,
            keybindings,
        }
    }
}

impl Action {
    pub fn all() -> &'static [Action] {
        &[
            Action::ScrollDown,
            Action::ScrollUp,
            Action::ScrollLeft,
            Action::ScrollRight,
            Action::HalfPageDown,
            Action::HalfPageUp,
            Action::PageDown,
            Action::Top,
            Action::Bottom,
            Action::OpenSearch,
            Action::AcceptSearch,
            Action::NextSearchHit,
            Action::PreviousSearchHit,
            Action::ShowHelp,
            Action::CloseOverlay,
            Action::Quit,
        ]
    }

    fn from_config_key(value: &str) -> Option<Self> {
        Some(match value {
            "scroll_down" => Action::ScrollDown,
            "scroll_up" => Action::ScrollUp,
            "scroll_left" => Action::ScrollLeft,
            "scroll_right" => Action::ScrollRight,
            "half_page_down" => Action::HalfPageDown,
            "half_page_up" => Action::HalfPageUp,
            "page_down" => Action::PageDown,
            "top" => Action::Top,
            "bottom" => Action::Bottom,
            "open_search" => Action::OpenSearch,
            "accept_search" => Action::AcceptSearch,
            "next_search_hit" => Action::NextSearchHit,
            "previous_search_hit" => Action::PreviousSearchHit,
            "show_help" => Action::ShowHelp,
            "close_overlay" => Action::CloseOverlay,
            "quit" => Action::Quit,
            _ => return None,
        })
    }

    fn config_key(self) -> &'static str {
        match self {
            Action::ScrollDown => "scroll_down",
            Action::ScrollUp => "scroll_up",
            Action::ScrollLeft => "scroll_left",
            Action::ScrollRight => "scroll_right",
            Action::HalfPageDown => "half_page_down",
            Action::HalfPageUp => "half_page_up",
            Action::PageDown => "page_down",
            Action::Top => "top",
            Action::Bottom => "bottom",
            Action::OpenSearch => "open_search",
            Action::AcceptSearch => "accept_search",
            Action::NextSearchHit => "next_search_hit",
            Action::PreviousSearchHit => "previous_search_hit",
            Action::ShowHelp => "show_help",
            Action::CloseOverlay => "close_overlay",
            Action::Quit => "quit",
        }
    }
}

fn config_source() -> Result<Option<PathBuf>> {
    let project = env::current_dir()
        .context("failed to resolve current working directory")?
        .join(PROJECT_CONFIG_NAME);
    if project.is_file() {
        return Ok(Some(project));
    }

    let Some(home) = config_home_dir() else {
        return Ok(None);
    };
    let global = home.join("mdglance").join("config.toml");
    if global.is_file() {
        return Ok(Some(global));
    }

    Ok(None)
}

fn config_home_dir() -> Option<PathBuf> {
    if let Some(path) = env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(path));
    }

    env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
}

fn default_keybindings() -> Vec<(Action, Vec<&'static str>)> {
    vec![
        (Action::ScrollDown, vec!["j"]),
        (Action::ScrollUp, vec!["k"]),
        (Action::ScrollLeft, vec!["h"]),
        (Action::ScrollRight, vec!["l"]),
        (Action::HalfPageDown, vec!["d"]),
        (Action::HalfPageUp, vec!["u"]),
        (Action::PageDown, vec!["Space"]),
        (Action::Top, vec!["g"]),
        (Action::Bottom, vec!["Shift+G"]),
        (Action::OpenSearch, vec!["/"]),
        (Action::AcceptSearch, vec!["Enter"]),
        (Action::NextSearchHit, vec!["n"]),
        (Action::PreviousSearchHit, vec!["Shift+N"]),
        (Action::ShowHelp, vec!["?"]),
        (Action::CloseOverlay, vec!["Escape"]),
        (Action::Quit, vec!["q", "Cmd+W", "Cmd+Q"]),
    ]
}

fn parse_shortcut(input: &str) -> Result<KeyBinding> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        bail!("shortcut cannot be empty");
    }

    let parts = trimmed.split('+').map(str::trim).collect::<Vec<_>>();
    if parts.iter().any(|part| part.is_empty()) {
        bail!("shortcut `{trimmed}` contains an empty token");
    }

    let mut ctrl = false;
    let mut alt = false;
    let mut meta = false;
    let mut shift = None;
    let mut key_token = None;

    for token in &parts {
        match token.to_ascii_lowercase().as_str() {
            "cmd" | "command" | "meta" | "super" => meta = true,
            "ctrl" | "control" => ctrl = true,
            "alt" | "option" => alt = true,
            "shift" => shift = Some(true),
            _ => {
                if key_token.is_some() {
                    bail!("shortcut `{trimmed}` contains more than one key");
                }
                key_token = Some(*token);
            }
        }
    }

    let key_token =
        key_token.ok_or_else(|| anyhow::anyhow!("shortcut `{trimmed}` is missing a key"))?;
    let key = normalize_key_token(key_token, &mut shift, ctrl || alt || meta)?;

    Ok(KeyBinding {
        display: trimmed.to_string(),
        shortcut: Shortcut {
            key,
            shift,
            ctrl,
            alt,
            meta,
        },
    })
}

fn normalize_key_token(
    token: &str,
    shift: &mut Option<bool>,
    has_non_shift_modifier: bool,
) -> Result<String> {
    let normalized = match token.to_ascii_lowercase().as_str() {
        "space" => " ".to_string(),
        "esc" | "escape" => "Escape".to_string(),
        "enter" | "return" => "Enter".to_string(),
        "tab" => "Tab".to_string(),
        "backspace" => "Backspace".to_string(),
        "delete" => "Delete".to_string(),
        "home" => "Home".to_string(),
        "end" => "End".to_string(),
        "pageup" => "PageUp".to_string(),
        "pagedown" => "PageDown".to_string(),
        "arrowup" | "up" => "ArrowUp".to_string(),
        "arrowdown" | "down" => "ArrowDown".to_string(),
        "arrowleft" | "left" => "ArrowLeft".to_string(),
        "arrowright" | "right" => "ArrowRight".to_string(),
        _ => normalize_printable_key(token, shift, has_non_shift_modifier)?,
    };

    Ok(normalized)
}

fn normalize_printable_key(
    token: &str,
    shift: &mut Option<bool>,
    has_non_shift_modifier: bool,
) -> Result<String> {
    let mut chars = token.chars();
    let Some(first) = chars.next() else {
        bail!("shortcut key cannot be empty");
    };

    if chars.next().is_some() {
        bail!("unsupported key `{token}`");
    }

    if first.is_ascii_alphabetic() {
        if shift.is_none() && !has_non_shift_modifier {
            *shift = Some(first.is_ascii_uppercase());
        }
        return Ok(first.to_ascii_lowercase().to_string());
    }

    Ok(first.to_string())
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.config_key())
    }
}

impl Shortcut {
    pub fn matches_native(&self, key: &Key<'_>, modifiers: ModifiersState) -> bool {
        if self.meta != modifiers.super_key() {
            return false;
        }
        if self.ctrl != modifiers.control_key() {
            return false;
        }
        if self.alt != modifiers.alt_key() {
            return false;
        }
        if let Some(shift) = self.shift
            && shift != modifiers.shift_key()
        {
            return false;
        }

        match key {
            Key::Character(value) => {
                let normalized = value
                    .chars()
                    .next()
                    .map(|ch| ch.to_ascii_lowercase().to_string());
                normalized.as_deref() == Some(self.key.as_str())
            }
            _ => native_named_key(key) == Some(self.key.as_str()),
        }
    }
}

fn native_named_key(key: &Key<'_>) -> Option<&'static str> {
    match key {
        Key::Enter => Some("Enter"),
        Key::Tab => Some("Tab"),
        Key::Space => Some(" "),
        Key::ArrowDown => Some("ArrowDown"),
        Key::ArrowLeft => Some("ArrowLeft"),
        Key::ArrowRight => Some("ArrowRight"),
        Key::ArrowUp => Some("ArrowUp"),
        Key::End => Some("End"),
        Key::Home => Some("Home"),
        Key::PageDown => Some("PageDown"),
        Key::PageUp => Some("PageUp"),
        Key::Backspace => Some("Backspace"),
        Key::Delete => Some("Delete"),
        Key::Escape => Some("Escape"),
        _ => None,
    }
}
