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
    fn mermaid_uses_its_default_theme_on_a_light_canvas() {
        assert!(!APP_JS.contains("theme:"));
        assert!(STYLE_CSS.contains(".mermaid {\n  text-align: center;\n  background: #ffffff;"));
    }
}
