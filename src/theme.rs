use anyhow::{Context, Result, bail};
use include_dir::{Dir, include_dir};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    env, fmt,
    path::{Path, PathBuf},
    str::FromStr,
    sync::LazyLock,
};
use syntect::{
    highlighting::{
        Color as SyntectColor, ScopeSelectors, StyleModifier, Theme, ThemeItem, ThemeSet,
        ThemeSettings,
    },
    html::{ClassStyle, css_for_theme_with_class_style},
};

const SYNTAX_PREFIX: &str = "syntect-";
static BUNDLED: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/themes");
static SYNTECT_THEMES: LazyLock<ThemeSet> = LazyLock::new(ThemeSet::load_defaults);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Light,
    Dark,
}

#[derive(Debug, Clone)]
pub struct ThemeConfig {
    pub name: String,
    pub light: ThemeVariant,
    pub dark: ThemeVariant,
    system: bool,
    appearance: Option<Appearance>,
}

#[derive(Debug, Clone)]
pub struct ThemeVariant {
    pub colors: ThemeColors,
    pub syntax_theme: String,
    syntax: Theme,
    syntax_theme_file: Option<PathBuf>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(try_from = "String")]
pub struct HexColor([u8; 3]);

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ThemeOverrides {
    name: Option<String>,
    preset: Option<String>,
    #[serde(default)]
    light: ThemeVariantOverrides,
    #[serde(default)]
    dark: ThemeVariantOverrides,
}

#[derive(Debug, Clone, Deserialize, Default)]
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
    syntax_theme_file: Option<String>,
}

#[derive(Debug, Clone)]
struct Palette {
    primary: Primary,
    normal: Ansi,
    bright: Ansi,
}
#[derive(Debug, Clone, Deserialize)]
struct AlacrittyFile {
    colors: AlacrittyColors,
}
#[derive(Debug, Clone, Deserialize)]
struct AlacrittyColors {
    primary: Primary,
    normal: Ansi,
    bright: Ansi,
}
#[derive(Debug, Clone, Deserialize)]
struct Primary {
    background: HexColor,
    foreground: HexColor,
}
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct Ansi {
    black: HexColor,
    red: HexColor,
    green: HexColor,
    yellow: HexColor,
    blue: HexColor,
    magenta: HexColor,
    cyan: HexColor,
    white: HexColor,
}

#[derive(Debug, Clone)]
pub struct ThemeCatalog {
    entries: Vec<CatalogEntry>,
}
#[derive(Debug, Clone)]
struct CatalogEntry {
    name: String,
    source: ThemeSource,
    result: std::result::Result<Palette, String>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeSource {
    Bundled,
    User,
    System,
}
#[derive(Debug, Clone, Serialize)]
pub struct ThemePickerEntry {
    pub name: String,
    pub source: ThemeSource,
    pub appearance: Option<Appearance>,
    pub error: Option<String>,
    pub active: bool,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self::resolve(ThemeOverrides::default(), Path::new("."))
            .expect("bundled default themes must be valid")
    }
}

impl ThemeConfig {
    pub fn resolve(overrides: ThemeOverrides, config_dir: &Path) -> Result<Self> {
        let catalog = ThemeCatalog::load(true);
        Self::resolve_with(
            &overrides,
            config_dir,
            dirs::home_dir().as_deref(),
            &catalog,
        )
    }
    pub fn named(name: &str) -> Result<Self> {
        let catalog = ThemeCatalog::load(true);
        if name == "system" {
            return Self::resolve_with(
                &ThemeOverrides::default(),
                Path::new("."),
                dirs::home_dir().as_deref(),
                &catalog,
            );
        }
        let variant = catalog.variant(name)?;
        let appearance = variant.appearance();
        Ok(Self {
            name: name.into(),
            light: variant.clone(),
            dark: variant,
            system: false,
            appearance: Some(appearance),
        })
    }
    fn resolve_with(
        overrides: &ThemeOverrides,
        config_dir: &Path,
        home: Option<&Path>,
        catalog: &ThemeCatalog,
    ) -> Result<Self> {
        if overrides.name.is_some() && overrides.preset.is_some() {
            bail!("`theme.name` and legacy `theme.preset` are mutually exclusive");
        }
        let name = overrides
            .name
            .clone()
            .or_else(|| overrides.preset.clone())
            .unwrap_or_else(|| "system".into());
        let (mut light, mut dark, system) = if name == "system" {
            (bundled_variant("light")?, bundled_variant("dark")?, true)
        } else {
            let variant = catalog
                .variant(&name)
                .with_context(|| format!("configured theme `{name}` is unavailable"))?;
            (variant.clone(), variant, false)
        };
        let inferred_appearance = (!system).then(|| light.appearance());
        light.apply(
            overrides.light.clone(),
            "theme.light",
            config_dir,
            home,
            catalog,
        )?;
        dark.apply(
            overrides.dark.clone(),
            "theme.dark",
            config_dir,
            home,
            catalog,
        )?;
        Ok(Self {
            name,
            light,
            dark,
            system,
            appearance: inferred_appearance,
        })
    }
    pub fn css(&self) -> Result<String> {
        if self.system {
            Ok(format!(
                ":root {{ color-scheme: light dark; }}\n{}\n{}\n@media (prefers-color-scheme: dark) {{\n{}\n{}\n}}",
                self.light.colors.css_variables(),
                self.light.syntax_css()?,
                self.dark.colors.css_variables(),
                self.dark.syntax_css()?
            ))
        } else {
            let active = if self.appearance == Some(Appearance::Light) {
                &self.light
            } else {
                &self.dark
            };
            Ok(format!(
                ":root {{ color-scheme: {}; }}\n{}\n{}",
                appearance_name(active.appearance()),
                active.colors.css_variables(),
                active.syntax_css()?
            ))
        }
    }
}

impl ThemeCatalog {
    pub fn load(include_user: bool) -> Self {
        Self::load_from_dir(include_user.then(user_theme_dir).flatten().as_deref())
    }
    fn load_from_dir(user_dir: Option<&Path>) -> Self {
        let mut map = BTreeMap::new();
        for file in BUNDLED.files() {
            if file.path().extension().and_then(|v| v.to_str()) != Some("toml") {
                continue;
            }
            let Some(name) = file.path().file_stem().and_then(|v| v.to_str()) else {
                continue;
            };
            let result = file
                .contents_utf8()
                .ok_or_else(|| "theme is not UTF-8".into())
                .and_then(parse_palette);
            map.insert(
                name.to_owned(),
                CatalogEntry {
                    name: name.to_owned(),
                    source: ThemeSource::Bundled,
                    result,
                },
            );
        }
        if let Some(dir) = user_dir
            && let Ok(files) = std::fs::read_dir(dir)
        {
            for file in files.flatten() {
                let path = file.path();
                if !path.is_file() || path.extension().and_then(|v| v.to_str()) != Some("toml") {
                    continue;
                }
                let Some(name) = path.file_stem().and_then(|v| v.to_str()).map(str::to_owned)
                else {
                    continue;
                };
                let result = std::fs::read_to_string(&path)
                    .map_err(|e| format!("failed to read {}: {e}", path.display()))
                    .and_then(|s| parse_palette(&s));
                map.insert(
                    name.clone(),
                    CatalogEntry {
                        name,
                        source: ThemeSource::User,
                        result,
                    },
                );
            }
        }
        map.remove("system");
        let mut entries: Vec<CatalogEntry> = map.into_values().collect();
        entries.sort_by(|a, b| {
            a.name
                .to_lowercase()
                .cmp(&b.name.to_lowercase())
                .then(a.name.cmp(&b.name))
        });
        entries.insert(
            0,
            CatalogEntry {
                name: "system".into(),
                source: ThemeSource::System,
                result: Err(String::new()),
            },
        );
        Self { entries }
    }
    fn palette(&self, name: &str) -> Result<&Palette> {
        let entry = self
            .entries
            .iter()
            .find(|e| e.name == name)
            .with_context(|| format!("theme `{name}` was not found"))?;
        entry
            .result
            .as_ref()
            .map_err(|e| anyhow::anyhow!("invalid theme `{name}`: {e}"))
    }
    fn variant(&self, name: &str) -> Result<ThemeVariant> {
        Ok(ThemeVariant::from_palette(name, self.palette(name)?))
    }
    pub fn picker_entries(&self, active: &str) -> Vec<ThemePickerEntry> {
        self.entries
            .iter()
            .map(|entry| {
                if entry.name == "system" {
                    return ThemePickerEntry {
                        name: entry.name.clone(),
                        source: entry.source,
                        appearance: None,
                        error: None,
                        active: active == entry.name,
                    };
                }
                match &entry.result {
                    Ok(p) => {
                        let v = ThemeVariant::from_palette(&entry.name, p);
                        let appearance = Some(v.appearance());
                        ThemePickerEntry {
                            name: entry.name.clone(),
                            source: entry.source,
                            appearance,
                            error: None,
                            active: active == entry.name,
                        }
                    }
                    Err(error) => ThemePickerEntry {
                        name: entry.name.clone(),
                        source: entry.source,
                        appearance: None,
                        error: Some(error.clone()),
                        active: active == entry.name,
                    },
                }
            })
            .collect()
    }
    pub fn css_for(&self, name: &str) -> Result<String> {
        if name == "system" {
            let o = ThemeOverrides::default();
            return ThemeConfig::resolve_with(
                &o,
                Path::new("."),
                dirs::home_dir().as_deref(),
                self,
            )?
            .css();
        }
        let v = self.variant(name)?;
        let appearance = v.appearance();
        ThemeConfig {
            name: name.into(),
            light: v.clone(),
            dark: v,
            system: false,
            appearance: Some(appearance),
        }
        .css()
    }
}

impl ThemeVariant {
    fn from_palette(name: &str, p: &Palette) -> Self {
        Self {
            colors: semantic_colors(p),
            syntax_theme: name.into(),
            syntax: palette_syntax_theme(name, p),
            syntax_theme_file: None,
        }
    }
    fn appearance(&self) -> Appearance {
        appearance(self.colors.background)
    }
    fn apply(
        &mut self,
        o: ThemeVariantOverrides,
        context: &str,
        config_dir: &Path,
        home: Option<&Path>,
        catalog: &ThemeCatalog,
    ) -> Result<()> {
        macro_rules! apply_color {
            ($f:ident) => {
                if let Some(v) = o.$f {
                    self.colors.$f = parse_config_color(&v, stringify!($f), context)?;
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
        if o.syntax_theme.is_some() && o.syntax_theme_file.is_some() {
            bail!(
                "`{context}.syntax_theme` and `{context}.syntax_theme_file` are mutually exclusive"
            )
        }
        if let Some(id) = o.syntax_theme {
            self.syntax = syntax_theme(&id, catalog)?;
            self.syntax_theme = id;
            self.syntax_theme_file = None
        } else if let Some(value) = o.syntax_theme_file {
            let path = resolve_theme_path(&value, config_dir, home).with_context(|| {
                format!("invalid `{context}.syntax_theme_file` value `{value}`")
            })?;
            self.syntax = ThemeSet::get_theme(&path)
                .with_context(|| format!("failed to load syntax theme file {}", path.display()))?;
            self.syntax_theme_file = Some(path)
        }
        Ok(())
    }
    fn syntax_css(&self) -> Result<String> {
        generate_syntax_css(
            &self.syntax,
            self.syntax_theme_file
                .as_ref()
                .and_then(|p| p.to_str())
                .unwrap_or(&self.syntax_theme),
        )
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
            self.error
        )
    }
}
impl TryFrom<String> for HexColor {
    type Error = anyhow::Error;
    fn try_from(v: String) -> Result<Self> {
        v.parse()
    }
}
impl FromStr for HexColor {
    type Err = anyhow::Error;
    fn from_str(v: &str) -> Result<Self> {
        if v.len() != 7 || !v.starts_with('#') {
            bail!("expected a color in #RRGGBB format")
        };
        let c = |r| {
            u8::from_str_radix(&v[r], 16)
                .map_err(|_| anyhow::anyhow!("expected a color in #RRGGBB format"))
        };
        Ok(Self([c(1..3)?, c(3..5)?, c(5..7)?]))
    }
}
impl fmt::Display for HexColor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.0[0], self.0[1], self.0[2])
    }
}
impl From<HexColor> for SyntectColor {
    fn from(c: HexColor) -> Self {
        Self {
            r: c.0[0],
            g: c.0[1],
            b: c.0[2],
            a: 255,
        }
    }
}

fn parse_palette(s: &str) -> std::result::Result<Palette, String> {
    toml::from_str::<AlacrittyFile>(s)
        .map(|p| Palette {
            primary: p.colors.primary,
            normal: p.colors.normal,
            bright: p.colors.bright,
        })
        .map_err(|e| e.to_string())
}
fn bundled_variant(name: &str) -> Result<ThemeVariant> {
    let path = format!("{name}.toml");
    let file = BUNDLED
        .get_file(&path)
        .with_context(|| format!("bundled theme `{name}` is missing"))?;
    let source = file
        .contents_utf8()
        .with_context(|| format!("bundled theme `{name}` is not UTF-8"))?;
    let palette = parse_palette(source).map_err(anyhow::Error::msg)?;
    Ok(ThemeVariant::from_palette(name, &palette))
}
fn parse_config_color(v: &str, f: &str, c: &str) -> Result<HexColor> {
    v.parse()
        .map_err(|e| anyhow::anyhow!("invalid `{c}.{f}` value `{v}`: {e}"))
}
fn blend(a: HexColor, b: HexColor, n: u16) -> HexColor {
    HexColor(std::array::from_fn(|i| {
        ((u16::from(a.0[i]) * (100 - n) + u16::from(b.0[i]) * n) / 100) as u8
    }))
}
fn semantic_colors(p: &Palette) -> ThemeColors {
    let b = p.primary.background;
    let f = p.primary.foreground;
    ThemeColors {
        background: b,
        text: f,
        surface: blend(b, f, 7),
        sidebar_background: blend(b, f, 5),
        code_background: blend(b, f, 6),
        border: blend(b, f, 22),
        divider: blend(b, f, 15),
        muted_text: blend(b, f, 62),
        heading: p.normal.magenta,
        link: p.normal.blue,
        accent: p.normal.cyan,
        search_match: p.normal.yellow,
        error: p.normal.red,
    }
}
fn appearance(c: HexColor) -> Appearance {
    let linear = |v: u8| {
        let x = f64::from(v) / 255.0;
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    if 0.2126 * linear(c.0[0]) + 0.7152 * linear(c.0[1]) + 0.0722 * linear(c.0[2]) > 0.179 {
        Appearance::Light
    } else {
        Appearance::Dark
    }
}
fn appearance_name(a: Appearance) -> &'static str {
    match a {
        Appearance::Light => "light",
        Appearance::Dark => "dark",
    }
}
fn user_theme_dir() -> Option<PathBuf> {
    env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|p| p.join(".config")))
        .map(|p| p.join("mdglance/themes"))
}
pub fn syntax_class_style() -> ClassStyle {
    ClassStyle::SpacedPrefixed {
        prefix: SYNTAX_PREFIX,
    }
}
fn generate_syntax_css(t: &Theme, d: &str) -> Result<String> {
    let mut t = t.clone();
    t.settings.background = None;
    css_for_theme_with_class_style(&t, syntax_class_style())
        .map_err(|e| anyhow::anyhow!("failed to generate syntax CSS for `{d}`: {e}"))
}
fn syntax_theme(id: &str, c: &ThemeCatalog) -> Result<Theme> {
    let n = match id {
        "inspired-github" => Some("InspiredGitHub"),
        "solarized-dark" => Some("Solarized (dark)"),
        "solarized-light" => Some("Solarized (light)"),
        "base16-eighties-dark" => Some("base16-eighties.dark"),
        "base16-mocha-dark" => Some("base16-mocha.dark"),
        "base16-ocean-dark" => Some("base16-ocean.dark"),
        "base16-ocean-light" => Some("base16-ocean.light"),
        _ => None,
    };
    if let Some(n) = n {
        return SYNTECT_THEMES
            .themes
            .get(n)
            .cloned()
            .context("bundled Syntect theme is missing");
    };
    Ok(palette_syntax_theme(id, c.palette(id)?))
}
fn palette_syntax_theme(name: &str, p: &Palette) -> Theme {
    let r = [
        ("comment", p.bright.black),
        ("string", p.bright.green),
        ("constant.numeric, constant.language", p.bright.yellow),
        ("keyword, storage", p.bright.magenta),
        ("entity.name.function, support.function", p.bright.blue),
        (
            "entity.name.type, entity.name.class, support.type",
            p.bright.cyan,
        ),
        ("keyword.operator", p.bright.cyan),
        ("invalid", p.normal.red),
    ];
    Theme {
        name: Some(name.into()),
        settings: ThemeSettings {
            foreground: Some(p.primary.foreground.into()),
            ..Default::default()
        },
        scopes: r
            .into_iter()
            .map(|(s, c)| ThemeItem {
                scope: s.parse::<ScopeSelectors>().expect("valid scopes"),
                style: StyleModifier {
                    foreground: Some(c.into()),
                    ..Default::default()
                },
            })
            .collect(),
        ..Default::default()
    }
}
fn resolve_theme_path(v: &str, d: &Path, h: Option<&Path>) -> Result<PathBuf> {
    if v == "~" {
        return h
            .map(Path::to_path_buf)
            .context("cannot expand `~` because the home directory is unavailable");
    };
    if let Some(v) = v.strip_prefix("~/") {
        return Ok(h
            .context("cannot expand `~/` because the home directory is unavailable")?
            .join(v));
    };
    if v.starts_with('~') {
        bail!("only `~` and `~/` home-directory expansion are supported")
    };
    let p = PathBuf::from(v);
    Ok(if p.is_absolute() { p } else { d.join(p) })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_packaged_themes_parse() {
        let c = ThemeCatalog::load(false);
        assert_eq!(c.entries.len(), 9);
        for e in &c.entries[1..] {
            assert!(e.result.is_ok(), "{}: {:?}", e.name, e.result)
        }
    }
    #[test]
    fn accepts_unknown_sections() {
        let mut s = include_str!("../themes/dark.toml").to_owned();
        s.push_str("\n[colors.cursor]\ntext='#ffffff'\ncursor='#000000'\n[window]\nopacity=0.9\n");
        assert!(parse_palette(&s).is_ok())
    }
    #[test]
    fn rejects_missing_and_bad_colors() {
        assert!(parse_palette("[colors.primary]\nbackground='#fff'").is_err())
    }
    #[test]
    fn maps_colors() {
        let p = parse_palette(include_str!("../themes/gruvbox.toml")).unwrap();
        let c = semantic_colors(&p);
        assert_eq!(c.heading.to_string(), "#b16286");
        assert_eq!(c.link.to_string(), "#458588");
        let css = generate_syntax_css(&palette_syntax_theme("x", &p), "x").unwrap();
        assert!(css.contains("#928374"));
        assert!(css.contains("#b8bb26"))
    }
    #[test]
    fn infers_light_dark() {
        assert_eq!(appearance("#ffffff".parse().unwrap()), Appearance::Light);
        assert_eq!(appearance("#002b36".parse().unwrap()), Appearance::Dark)
    }
    #[test]
    fn config_alias_and_conflict() {
        let o: ThemeOverrides = toml::from_str("preset='gruvbox'").unwrap();
        assert_eq!(
            ThemeConfig::resolve(o, Path::new(".")).unwrap().name,
            "gruvbox"
        );
        let o: ThemeOverrides = toml::from_str("name='dark'\npreset='light'").unwrap();
        assert!(ThemeConfig::resolve(o, Path::new(".")).is_err())
    }
    #[test]
    fn variant_overrides_follow_inferred_palette_appearance() {
        let o: ThemeOverrides =
            toml::from_str("name='dark'\n[light]\nheading='#111111'\n[dark]\nheading='#abcdef'")
                .unwrap();
        let css = ThemeConfig::resolve(o, Path::new("."))
            .unwrap()
            .css()
            .unwrap();
        assert!(css.contains("--heading: #abcdef"));
        assert!(!css.contains("--heading: #111111"));
    }
    #[test]
    fn system_composes_files() {
        let css = ThemeConfig::default().css().unwrap();
        assert_eq!(ThemeConfig::default().name, "system");
        assert!(css.contains("prefers-color-scheme: dark"));
        assert!(css.contains("#ffffff"));
        assert!(css.contains("#0d1117"))
    }
    #[test]
    fn standard_themes_use_github_like_semantics() {
        for name in ["light", "dark"] {
            let theme = bundled_variant(name).unwrap();
            assert_eq!(theme.colors.heading, theme.colors.text);
            assert_eq!(theme.colors.accent, theme.colors.link);
        }
    }
    #[test]
    fn user_override_and_invalid_report() {
        let d = env::temp_dir().join(format!("mdglance-theme-test-{}", std::process::id()));
        let _ = std::fs::create_dir(&d);
        std::fs::write(d.join("dark.toml"), "bad").unwrap();
        let c = ThemeCatalog::load_from_dir(Some(&d));
        let e = c.entries.iter().find(|e| e.name == "dark").unwrap();
        assert_eq!(e.source, ThemeSource::User);
        assert!(e.result.is_err());
        let _ = std::fs::remove_file(d.join("dark.toml"));
        let _ = std::fs::remove_dir(d);
    }
}
