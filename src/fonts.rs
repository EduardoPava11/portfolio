//! Font proof: the statement, a caption and the name set in every candidate
//! typeface, side by side, so the choice is made by reading. Candidates live in
//! content/fonts.toml. Pick one, copy its `family` and `css` into site.toml, rebuild.
//!
//!   portfolio fonts            -> analysis/fonts/index.html

use crate::content;
use crate::site::esc;
use crate::Result;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Deserialize)]
struct FontsFile {
    font: Vec<Font>,
}

#[derive(Deserialize)]
struct Font {
    name: String,
    /// CSS font-family stack.
    family: String,
    /// Stylesheet that loads it, or empty for a system face.
    #[serde(default)]
    css: String,
    #[serde(default)]
    note: String,
}

pub fn run(root: &Path) -> Result<()> {
    let content = content::load_originals(root)?;
    let fonts: FontsFile = toml::from_str(&fs::read_to_string(root.join("content/fonts.toml"))?)?;
    let out = root.join("analysis/fonts");
    fs::create_dir_all(&out)?;

    let mut links = String::new();
    for f in &fonts.font {
        if !f.css.is_empty() {
            links.push_str(&format!("<link rel=\"stylesheet\" href=\"{}\">\n", esc(&f.css)));
        }
    }
    let para = content.statement.first().cloned().unwrap_or_default();
    let mut html = format!(
        r##"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Font proof</title>
{links}<style>
body {{ margin: 0; padding: 24px 16px; background: #0b0b0a; color: #ece7df; font: 15px/1.5 Georgia, serif; }}
h1 {{ font-weight: normal; font-size: 1.3rem; margin: 0 0 0.3rem; }}
.sub {{ color: #9a938a; margin: 0 0 1.5rem; max-width: 80ch; }}
.grid {{ display: grid; grid-template-columns: repeat(auto-fill, minmax(360px, 1fr)); gap: 24px; }}
.card {{ border: 1px solid #2a2825; padding: 20px; }}
.card .label {{ font: 0.75rem/1.4 ui-monospace, Menlo, monospace; color: #9a938a; margin: 0 0 1rem; }}
.card .name {{ font-size: 1rem; margin: 0 0 0.2rem; }}
.card .nav {{ font-size: 0.72rem; letter-spacing: 0.14em; text-transform: uppercase; color: #9a938a; margin: 0 0 1rem; }}
.card .cap {{ font-size: 0.8rem; color: #9a938a; margin: 0 0 1rem; }}
.card .cap b {{ color: #ece7df; font-weight: normal; }}
.card .prose {{ font-size: 1.05rem; line-height: 1.6; margin: 0; }}
</style></head><body>
<h1>Font proof</h1>
<p class="sub">The same four things the site sets, in each candidate: the name, the two nav words, a caption, the first paragraph of the statement. Pick one and put its <code>family</code> and <code>css</code> into <code>content/site.toml</code>, then rebuild.</p>
<div class="grid">
"##
    );
    for f in &fonts.font {
        html.push_str(&format!(
            r#"<div class="card" style="font-family:{family}">
  <p class="label">{name}<br>{note}</p>
  <p class="name">{site_name}</p>
  <p class="nav">Statement &nbsp; Bio</p>
  <p class="cap"><b>28</b> Autumn 2026</p>
  <p class="prose">{para}</p>
</div>
"#,
            family = esc(&f.family),
            name = esc(&f.name),
            note = esc(&f.note),
            site_name = esc(&content.site.name),
            para = esc(&para),
        ));
    }
    html.push_str("</div></body></html>\n");
    fs::write(out.join("index.html"), html)?;
    println!("fonts: {} candidates -> {}", fonts.font.len(), out.join("index.html").display());
    Ok(())
}
