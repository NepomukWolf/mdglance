use anyhow::Result;

pub const APP_JS: &str = include_str!("../assets/app.js");
pub const MERMAID_JS: &str = include_str!("../assets/mermaid.min.js");
pub const STYLE_CSS: &str = include_str!("../assets/style.css");

pub fn js_string_literal(value: &str) -> Result<String> {
    let json = serde_json::to_string(value)?;
    Ok(json.replace("</", "<\\/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mermaid_theme_follows_the_viewer_color_scheme() {
        assert!(APP_JS.contains("getComputedStyle(document.documentElement).colorScheme"));
        assert!(APP_JS.contains("(prefers-color-scheme: dark)"));
        assert!(APP_JS.contains("theme: mermaidDarkMode ? \"dark\" : \"default\""));
        assert!(APP_JS.contains("themeVariables: { darkMode: mermaidDarkMode }"));
    }
}
