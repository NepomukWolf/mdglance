use std::{fmt, str::FromStr, sync::LazyLock};

use anyhow::{Result, bail};
use serde::Deserialize;
use syntect::{
    highlighting::{
        Color as SyntectColor, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSet,
        ThemeSettings,
    },
    html::{ClassStyle, css_for_theme_with_class_style},
};

const SYNTAX_PREFIX: &str = "syntect-";
static DEFAULT_THEMES: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThemePreset {
    System,
    Light,
    Dark,
    TokyoNight,
    Gruvbox,
}

#[derive(Debug, Clone)]
pub struct ThemeConfig {
    pub preset: ThemePreset,
    pub light: ThemeVariant,
    pub dark: ThemeVariant,
}

#[derive(Debug, Clone)]
pub struct ThemeVariant {
    pub colors: ThemeColors,
    pub syntax_theme: String,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    pub background: HexColor,
    pub surface: HexColor,
    pub text: HexColor,
    pub muted_text: HexColor,
    pub heading: HexColor,
    pub link: HexColor,
    pub border: HexColor,
    pub divider: HexColor,
    pub code_background: HexColor,
    pub sidebar_background: HexColor,
    pub accent: HexColor,
    pub search_match: HexColor,
    pub error: HexColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HexColor([u8; 3]);

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ThemeOverrides {
    preset: Option<String>,
    #[serde(default)]
    light: ThemeVariantOverrides,
    #[serde(default)]
    dark: ThemeVariantOverrides,
}

#[derive(Debug, Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct ThemeVariantOverrides {
    background: Option<String>,
    surface: Option<String>,
    text: Option<String>,
    muted_text: Option<String>,
    heading: Option<String>,
    link: Option<String>,
    border: Option<String>,
    divider: Option<String>,
    code_background: Option<String>,
    sidebar_background: Option<String>,
    accent: Option<String>,
    search_match: Option<String>,
    error: Option<String>,
    syntax_theme: Option<String>,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self::for_preset(ThemePreset::System)
    }
}

impl ThemeConfig {
    pub fn resolve(overrides: ThemeOverrides) -> Result<Self> {
        let preset = overrides
            .preset
            .as_deref()
            .map(ThemePreset::from_str)
            .transpose()?
            .unwrap_or(ThemePreset::System);
        let mut config = Self::for_preset(preset);
        config.light.apply(overrides.light, "theme.light")?;
        config.dark.apply(overrides.dark, "theme.dark")?;
        Ok(config)
    }

    pub fn css(&self) -> Result<String> {
        match self.preset {
            ThemePreset::System => Ok(format!(
                ":root {{ color-scheme: light dark; }}\n{}\n{}\n@media (prefers-color-scheme: dark) {{\n{}\n{}\n}}",
                self.light.colors.css_variables(),
                syntax_css(&self.light.syntax_theme)?,
                self.dark.colors.css_variables(),
                syntax_css(&self.dark.syntax_theme)?,
            )),
            ThemePreset::Light => Ok(format!(
                ":root {{ color-scheme: light; }}\n{}\n{}",
                self.light.colors.css_variables(),
                syntax_css(&self.light.syntax_theme)?,
            )),
            ThemePreset::Dark | ThemePreset::TokyoNight | ThemePreset::Gruvbox => Ok(format!(
                ":root {{ color-scheme: dark; }}\n{}\n{}",
                self.dark.colors.css_variables(),
                syntax_css(&self.dark.syntax_theme)?,
            )),
        }
    }

    fn for_preset(preset: ThemePreset) -> Self {
        let standard_light = ThemeVariant {
            colors: standard_light(),
            syntax_theme: "inspired-github".into(),
        };
        let standard_dark = ThemeVariant {
            colors: standard_dark(),
            syntax_theme: "base16-ocean-dark".into(),
        };

        match preset {
            ThemePreset::System | ThemePreset::Light | ThemePreset::Dark => Self {
                preset,
                light: standard_light,
                dark: standard_dark,
            },
            ThemePreset::TokyoNight => Self {
                preset,
                light: standard_light,
                dark: ThemeVariant {
                    colors: tokyo_night(),
                    syntax_theme: "tokyo-night".into(),
                },
            },
            ThemePreset::Gruvbox => Self {
                preset,
                light: standard_light,
                dark: ThemeVariant {
                    colors: gruvbox(),
                    syntax_theme: "gruvbox".into(),
                },
            },
        }
    }
}

impl ThemeVariant {
    fn apply(&mut self, overrides: ThemeVariantOverrides, context: &str) -> Result<()> {
        macro_rules! apply_color {
            ($field:ident) => {
                if let Some(value) = overrides.$field {
                    self.colors.$field = parse_config_color(&value, stringify!($field), context)?;
                }
            };
        }

        apply_color!(background);
        apply_color!(surface);
        apply_color!(text);
        apply_color!(muted_text);
        apply_color!(heading);
        apply_color!(link);
        apply_color!(border);
        apply_color!(divider);
        apply_color!(code_background);
        apply_color!(sidebar_background);
        apply_color!(accent);
        apply_color!(search_match);
        apply_color!(error);

        if let Some(theme) = overrides.syntax_theme {
            validate_syntax_theme(&theme)?;
            self.syntax_theme = theme;
        }
        Ok(())
    }
}

impl ThemeColors {
    fn css_variables(self) -> String {
        format!(
            ":root {{\n  --background: {};\n  --surface: {};\n  --text: {};\n  --muted-text: {};\n  --heading: {};\n  --link: {};\n  --border: {};\n  --divider: {};\n  --code-background: {};\n  --sidebar-background: {};\n  --accent: {};\n  --search-match: {};\n  --error: {};\n}}",
            self.background,
            self.surface,
            self.text,
            self.muted_text,
            self.heading,
            self.link,
            self.border,
            self.divider,
            self.code_background,
            self.sidebar_background,
            self.accent,
            self.search_match,
            self.error,
        )
    }
}

impl FromStr for ThemePreset {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        match value {
            "system" => Ok(Self::System),
            "light" => Ok(Self::Light),
            "dark" => Ok(Self::Dark),
            "tokyo-night" => Ok(Self::TokyoNight),
            "gruvbox" => Ok(Self::Gruvbox),
            _ => bail!("unknown theme preset `{value}`"),
        }
    }
}

impl FromStr for HexColor {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        if value.len() != 7 || !value.starts_with('#') {
            bail!("expected a color in #RRGGBB format");
        }
        let component = |range| {
            u8::from_str_radix(&value[range], 16)
                .map_err(|_| anyhow::anyhow!("expected a color in #RRGGBB format"))
        };
        Ok(Self([component(1..3)?, component(3..5)?, component(5..7)?]))
    }
}

impl fmt::Display for HexColor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "#{:02x}{:02x}{:02x}",
            self.0[0], self.0[1], self.0[2]
        )
    }
}

fn parse_config_color(value: &str, field: &str, context: &str) -> Result<HexColor> {
    value
        .parse()
        .map_err(|error| anyhow::anyhow!("invalid `{context}.{field}` value `{value}`: {error}"))
}

fn color(value: &str) -> HexColor {
    value.parse().expect("built-in theme colors must be valid")
}

fn standard_light() -> ThemeColors {
    ThemeColors {
        background: color("#ffffff"),
        surface: color("#f6f8fa"),
        text: color("#1f2328"),
        muted_text: color("#59636e"),
        heading: color("#1f2328"),
        link: color("#0969da"),
        border: color("#d1d9e0"),
        divider: color("#d8dee4"),
        code_background: color("#f6f8fa"),
        sidebar_background: color("#f6f8fa"),
        accent: color("#0969da"),
        search_match: color("#bf8700"),
        error: color("#cf222e"),
    }
}

fn standard_dark() -> ThemeColors {
    ThemeColors {
        background: color("#0d1117"),
        surface: color("#161b22"),
        text: color("#f0f6fc"),
        muted_text: color("#9198a1"),
        heading: color("#f0f6fc"),
        link: color("#4493f8"),
        border: color("#3d444d"),
        divider: color("#3d444d"),
        code_background: color("#161b22"),
        sidebar_background: color("#151b23"),
        accent: color("#4493f8"),
        search_match: color("#d29922"),
        error: color("#f85149"),
    }
}

// Palette adapted from Tokyo Night Night by Folke Lemaitre (Apache-2.0).
fn tokyo_night() -> ThemeColors {
    ThemeColors {
        background: color("#1a1b26"),
        surface: color("#24283b"),
        text: color("#c0caf5"),
        muted_text: color("#a9b1d6"),
        heading: color("#bb9af7"),
        link: color("#7aa2f7"),
        border: color("#414868"),
        divider: color("#3b4261"),
        code_background: color("#16161e"),
        sidebar_background: color("#16161e"),
        accent: color("#7dcfff"),
        search_match: color("#e0af68"),
        error: color("#f7768e"),
    }
}

// Palette adapted from Gruvbox Dark Medium by Pavel Pertsev (MIT/X11).
fn gruvbox() -> ThemeColors {
    ThemeColors {
        background: color("#282828"),
        surface: color("#3c3836"),
        text: color("#ebdbb2"),
        muted_text: color("#a89984"),
        heading: color("#fabd2f"),
        link: color("#83a598"),
        border: color("#504945"),
        divider: color("#665c54"),
        code_background: color("#1d2021"),
        sidebar_background: color("#1d2021"),
        accent: color("#8ec07c"),
        search_match: color("#d79921"),
        error: color("#fb4934"),
    }
}

pub fn syntax_class_style() -> ClassStyle {
    ClassStyle::SpacedPrefixed {
        prefix: SYNTAX_PREFIX,
    }
}

fn syntax_css(id: &str) -> Result<String> {
    let mut theme = syntax_theme(id)?;
    theme.settings.background = None;
    css_for_theme_with_class_style(&theme, syntax_class_style())
        .map_err(|error| anyhow::anyhow!("failed to generate syntax CSS for `{id}`: {error}"))
}

fn validate_syntax_theme(id: &str) -> Result<()> {
    syntax_theme(id).map(|_| ())
}

fn syntax_theme(id: &str) -> Result<Theme> {
    let bundled_name = match id {
        "inspired-github" => Some("InspiredGitHub"),
        "solarized-dark" => Some("Solarized (dark)"),
        "solarized-light" => Some("Solarized (light)"),
        "base16-eighties-dark" => Some("base16-eighties.dark"),
        "base16-mocha-dark" => Some("base16-mocha.dark"),
        "base16-ocean-dark" => Some("base16-ocean.dark"),
        "base16-ocean-light" => Some("base16-ocean.light"),
        "tokyo-night" => return Ok(palette_syntax_theme("Tokyo Night", tokyo_syntax_palette())),
        "gruvbox" => return Ok(palette_syntax_theme("Gruvbox", gruvbox_syntax_palette())),
        _ => bail!("unknown syntax theme `{id}`"),
    };
    Ok(DEFAULT_THEMES
        .themes
        .get(bundled_name.expect("bundled theme name must exist"))
        .cloned()
        .expect("Syntect default theme must exist"))
}

struct SyntaxPalette {
    foreground: HexColor,
    comment: HexColor,
    string: HexColor,
    number: HexColor,
    keyword: HexColor,
    function: HexColor,
    type_name: HexColor,
    operator: HexColor,
    invalid: HexColor,
}

fn tokyo_syntax_palette() -> SyntaxPalette {
    SyntaxPalette {
        foreground: color("#c0caf5"),
        comment: color("#565f89"),
        string: color("#9ece6a"),
        number: color("#ff9e64"),
        keyword: color("#bb9af7"),
        function: color("#7aa2f7"),
        type_name: color("#7dcfff"),
        operator: color("#89ddff"),
        invalid: color("#f7768e"),
    }
}

fn gruvbox_syntax_palette() -> SyntaxPalette {
    SyntaxPalette {
        foreground: color("#ebdbb2"),
        comment: color("#928374"),
        string: color("#b8bb26"),
        number: color("#d3869b"),
        keyword: color("#fb4934"),
        function: color("#fabd2f"),
        type_name: color("#8ec07c"),
        operator: color("#fe8019"),
        invalid: color("#cc241d"),
    }
}

fn palette_syntax_theme(name: &str, palette: SyntaxPalette) -> Theme {
    let rules = [
        ("comment", palette.comment),
        ("string", palette.string),
        ("constant.numeric, constant.language", palette.number),
        ("keyword, storage", palette.keyword),
        ("entity.name.function, support.function", palette.function),
        (
            "entity.name.type, entity.name.class, support.type",
            palette.type_name,
        ),
        ("keyword.operator", palette.operator),
        ("invalid", palette.invalid),
    ];
    let scopes = rules
        .into_iter()
        .map(|(scope, foreground)| ThemeItem {
            scope: scope
                .parse::<ScopeSelectors>()
                .expect("built-in scopes must parse"),
            style: StyleModifier {
                foreground: Some(foreground.into()),
                ..StyleModifier::default()
            },
        })
        .collect();
    Theme {
        name: Some(name.into()),
        settings: ThemeSettings {
            foreground: Some(palette.foreground.into()),
            ..ThemeSettings::default()
        },
        scopes,
        ..Theme::default()
    }
}

impl From<HexColor> for SyntectColor {
    fn from(color: HexColor) -> Self {
        Self {
            r: color.0[0],
            g: color.0[1],
            b: color.0[2],
            a: 0xff,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_only_six_digit_hex_colors() {
        assert_eq!(
            "#7aa2f7".parse::<HexColor>().unwrap().to_string(),
            "#7aa2f7"
        );
        assert!("blue".parse::<HexColor>().is_err());
        assert!("#fff".parse::<HexColor>().is_err());
    }

    #[test]
    fn custom_syntax_themes_generate_prefixed_css() {
        for name in ["tokyo-night", "gruvbox"] {
            let css = syntax_css(name).unwrap();
            assert!(css.contains(".syntect-code"));
            assert!(css.contains(".syntect-comment"));
        }
    }

    #[test]
    fn resolves_preset_and_variant_overrides() {
        let overrides: ThemeOverrides = toml::from_str(
            r##"
preset = "tokyo-night"

[dark]
heading = "#abcdef"
syntax_theme = "solarized-dark"
"##,
        )
        .unwrap();
        let theme = ThemeConfig::resolve(overrides).unwrap();

        assert_eq!(theme.preset, ThemePreset::TokyoNight);
        assert_eq!(theme.dark.colors.heading.to_string(), "#abcdef");
        assert_eq!(theme.dark.syntax_theme, "solarized-dark");
        assert_eq!(theme.dark.colors.background.to_string(), "#1a1b26");
    }

    #[test]
    fn supports_every_documented_syntax_theme() {
        for name in [
            "inspired-github",
            "solarized-dark",
            "solarized-light",
            "base16-eighties-dark",
            "base16-mocha-dark",
            "base16-ocean-dark",
            "base16-ocean-light",
            "tokyo-night",
            "gruvbox",
        ] {
            assert!(syntax_theme(name).is_ok(), "failed to load {name}");
        }
    }

    #[test]
    fn rejects_invalid_theme_values() {
        let bad_color: ThemeOverrides =
            toml::from_str("[dark]\nbackground = \"transparent\"").unwrap();
        assert!(ThemeConfig::resolve(bad_color).is_err());

        let bad_syntax: ThemeOverrides =
            toml::from_str("[dark]\nsyntax_theme = \"unknown\"").unwrap();
        assert!(ThemeConfig::resolve(bad_syntax).is_err());

        let bad_preset: ThemeOverrides = toml::from_str("preset = \"unknown\"").unwrap();
        assert!(ThemeConfig::resolve(bad_preset).is_err());
    }

    #[test]
    fn system_css_contains_live_light_and_dark_variants() {
        let css = ThemeConfig::default().css().unwrap();

        assert!(css.contains("color-scheme: light dark"));
        assert!(css.contains("@media (prefers-color-scheme: dark)"));
        assert!(css.contains("--background: #ffffff"));
        assert!(css.contains("--background: #0d1117"));
    }

    #[test]
    fn fixed_presets_emit_only_the_active_variant() {
        let overrides: ThemeOverrides = toml::from_str("preset = \"tokyo-night\"").unwrap();
        let css = ThemeConfig::resolve(overrides).unwrap().css().unwrap();

        assert!(css.contains("color-scheme: dark"));
        assert!(css.contains("--background: #1a1b26"));
        assert!(!css.contains("prefers-color-scheme"));
        assert!(!css.contains("--background: #ffffff"));
    }

    #[test]
    fn changing_syntax_theme_changes_generated_css() {
        assert_ne!(
            syntax_css("tokyo-night").unwrap(),
            syntax_css("gruvbox").unwrap()
        );
    }
}
