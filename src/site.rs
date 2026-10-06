//! Writes `docs/`: one page with the body of work, the artist statement and the bio,
//! plus web sized copies of every photograph.

use crate::content::{self, Content, Photo};
use crate::images;
use crate::Result;
use rayon::prelude::*;
use std::fs;
use std::path::Path;

pub fn build(root: &Path) -> Result<()> {
    let content = content::load(root)?;
    let docs = root.join("docs");
    fs::create_dir_all(&docs)?;

    // Photographs: large view and grid thumbnail, skipped when already current.
    let sizes: Vec<(u32, u32)> = content
        .photos
        .par_iter()
        .map(|p| -> Result<(u32, u32)> {
            let large = docs.join("img/large").join(format!("{}.jpg", p.stem));
            let thumb = docs.join("img/thumb").join(format!("{}.jpg", p.stem));
            let img = images::open_upright(p)?;
            if !images::up_to_date(&p.path, &large) {
                images::write_jpeg(&images::fit(&img, images::LARGE_SIDE), &large)?;
            }
            if !images::up_to_date(&p.path, &thumb) {
                images::write_jpeg(&images::fit(&img, images::THUMB_SIDE), &thumb)?;
            }
            Ok((img.width(), img.height()))
        })
        .collect::<Result<Vec<_>>>()?;

    fs::write(docs.join("style.css"), fs::read_to_string(root.join("assets/style.css"))?)?;
    fs::write(docs.join("site.js"), fs::read_to_string(root.join("assets/site.js"))?)?;
    fs::write(docs.join(".nojekyll"), "")?;
    fs::write(docs.join("index.html"), index_html(&content, &sizes))?;

    println!(
        "site: {} photographs, {} statement paragraphs, {} bio paragraphs -> {}",
        content.photos.len(),
        content.statement.len(),
        content.bio.len(),
        docs.display()
    );
    Ok(())
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn index_html(c: &Content, sizes: &[(u32, u32)]) -> String {
    let site = &c.site;
    let mut h = String::new();
    h.push_str(&format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<meta name="description" content="{tagline}">
<link rel="canonical" href="{base}">
<link rel="stylesheet" href="style.css">
</head>
<body>
<header class="masthead">
  <h1>{name}</h1>
  <p class="tagline">{tagline}</p>
  <nav><a href="#work">Work</a><a href="#statement">Artist statement</a><a href="#bio">Bio</a></nav>
</header>
<main>
"##,
        title = esc(&site.title),
        base = esc(&site.base_url),
        tagline = esc(&site.tagline),
        name = esc(&site.name),
    ));

    // The body of work.
    h.push_str("<section id=\"work\" class=\"work\">\n<h2>Work</h2>\n");
    if c.photos.is_empty() {
        h.push_str("<p class=\"empty\">No photographs yet. Add files to <code>content/photos/</code> and rebuild.</p>\n");
    } else {
        h.push_str("<div class=\"grid\">\n");
        for (i, (p, &(w, hgt))) in c.photos.iter().zip(sizes).enumerate() {
            h.push_str(&figure(p, i, w, hgt));
        }
        h.push_str("</div>\n");
    }
    h.push_str("</section>\n");

    h.push_str(&text_section("statement", "Artist statement", &c.statement));
    h.push_str(&text_section("bio", "Bio", &c.bio));

    h.push_str("</main>\n<footer>\n");
    h.push_str(&format!("<p>{}", esc(&site.name)));
    if let Some(e) = &site.email {
        h.push_str(&format!(" &middot; <a href=\"mailto:{0}\">{0}</a>", esc(e)));
    }
    if let Some(ig) = &site.instagram {
        h.push_str(&format!(" &middot; <a href=\"https://www.instagram.com/{0}/\">@{0}</a>", esc(ig)));
    }
    h.push_str("</p>\n</footer>\n");
    h.push_str(
        r#"<dialog id="view"><button class="close" aria-label="Close">&times;</button><figure><img alt=""><figcaption></figcaption></figure></dialog>
<script src="site.js"></script>
</body>
</html>
"#,
    );
    h
}

fn figure(p: &Photo, index: usize, w: u32, h: u32) -> String {
    let title = p.title();
    let when = p.when();
    let mut cap = esc(&title);
    let mut detail: Vec<String> = Vec::new();
    if let Some(place) = &p.meta.place {
        detail.push(esc(place));
    }
    if !when.is_empty() {
        detail.push(esc(&when));
    }
    if !detail.is_empty() {
        cap.push_str(&format!(" <span class=\"detail\">{}</span>", detail.join(", ")));
    }
    let long = p.meta.caption.as_deref().map(esc).unwrap_or_default();
    format!(
        r#"<figure class="photo" data-index="{index}" data-large="img/large/{stem}.jpg" data-caption="{cap_attr}">
  <a href="img/large/{stem}.jpg"><img src="img/thumb/{stem}.jpg" width="{w}" height="{h}" loading="lazy" alt="{alt}"></a>
  <figcaption>{cap}{long}</figcaption>
</figure>
"#,
        stem = esc(&p.stem),
        cap_attr = esc(&strip_tags(&cap)),
        alt = esc(&title),
        long = if long.is_empty() { String::new() } else { format!("<span class=\"long\">{long}</span>") },
    )
}

fn strip_tags(s: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for ch in s.chars() {
        match ch {
            '<' => inside = true,
            '>' => inside = false,
            _ if !inside => out.push(ch),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"")
}

/// A heading, the paragraphs, and a button that copies the plain text.
fn text_section(id: &str, heading: &str, paragraphs: &[String]) -> String {
    let mut s = format!(
        "<section id=\"{id}\" class=\"text\">\n<div class=\"section-head\"><h2>{heading}</h2><button class=\"copy\" data-copy=\"{id}-text\" aria-live=\"polite\">Copy</button></div>\n<div id=\"{id}-text\" class=\"prose\">\n"
    );
    for p in paragraphs {
        s.push_str(&format!("<p>{}</p>\n", esc(p)));
    }
    s.push_str("</div>\n</section>\n");
    s
}
