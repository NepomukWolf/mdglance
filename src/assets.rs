use anyhow::Result;

pub const APP_JS: &str = include_str!("../assets/app.js");
pub const MERMAID_JS: &str = include_str!("../assets/mermaid.min.js");
pub const STYLE_CSS: &str = include_str!("../assets/style.css");

pub fn js_string_literal(value: &str) -> Result<String> {
    let json = serde_json::to_string(value)?;
    Ok(json.replace("</", "<\\/"))
}
