//! Writes `docs/`: the site. Form follows function: the function is a juror or a
//! visitor looking at the pictures in sequence, on a phone or a wall sized screen,
//! and finding the words when they want them. So:
//!
//! - one picture per screen, scroll snapped, nothing else on the screen but a small
//!   name, two small words (Statement, Bio) and the sequence itself;
//! - the sequence is drawn as a strip of each picture's CIELAB centroid along the
//!   bottom edge: progress bar, navigation and the colour thesis in one element;
//! - every picture is served as AVIF with a JPEG fallback, in two sizes, and carries
//!   a 24 pixel inline placeholder so the frame shows its colour before it loads;
//! - statement and bio open in a panel over the pictures, each with a copy button.

use crate::content::{self, Content, Photo};
use crate::images;
use crate::Result;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Long side of the two delivered sizes.
const SIZES: [u32; 2] = [1200, 2000];
/// Long side of the inline placeholder.
const TINY: u32 = 24;

struct Rendered {
    w: u32,
    h: u32,
    /// base64 JPEG of the TINY placeholder.
    tiny: String,
}

pub fn build(root: &Path) -> Result<()> {
    let content = content::load(root)?;
    let centroids = read_centroids(root, &content)?;
    let docs = root.join("docs");
    fs::create_dir_all(docs.join("img"))?;

    let rendered: Vec<Rendered> = content
        .photos
        .par_iter()
        .map(|p| render(p, &docs))
        .collect::<Result<Vec<_>>>()?;

    fs::write(docs.join("style.css"), fs::read_to_string(root.join("assets/style.css"))?)?;
    fs::write(docs.join("site.js"), fs::read_to_string(root.join("assets/site.js"))?)?;
    fs::write(docs.join(".nojekyll"), "")?;
    fs::write(docs.join("index.html"), index_html(&content, &rendered, &centroids))?;

    println!(
        "site: {} photographs x {} sizes x avif+jpeg, {} statement paragraphs, {} bio paragraphs -> {}",
        content.photos.len(),
        SIZES.len(),
        content.statement.len(),
        content.bio.len(),
        docs.display()
    );
    Ok(())
}

/// The strip needs each picture's centroid from the CIELAB reading. A missing or
/// stale reading is an error, not a guess: run `portfolio lab`.
fn read_centroids(root: &Path, c: &Content) -> Result<HashMap<String, String>> {
    let path = root.join("analysis/lab/lab.csv");
    let text = fs::read_to_string(&path).map_err(|_| format!("{} is missing; run `portfolio lab` first", path.display()))?;
    let mut map = HashMap::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() >= 14 {
            map.insert(cols[1].to_string(), cols[13].to_string());
        }
    }
    for p in &c.photos {
        if !map.contains_key(&p.stem) {
            return Err(format!("{} has no row for {}; run `portfolio lab` again", path.display(), p.stem).into());
        }
    }
    Ok(map)
}

fn render(p: &Photo, docs: &Path) -> Result<Rendered> {
    let img = images::open_display(p)?;
    for side in SIZES {
        let jpg = docs.join("img").join(format!("{}-{side}.jpg", p.stem));
        let avif = docs.join("img").join(format!("{}-{side}.avif", p.stem));
        if images::up_to_date(&p.display, &jpg) && images::up_to_date(&p.display, &avif) {
            continue;
        }
        let fitted = images::fit(&img, side);
        images::write_jpeg(&fitted, &jpg)?;
        images::write_avif(&fitted, &avif)?;
    }
    let tiny = images::fit(&img, TINY);
    let mut buf = Vec::new();
    tiny.write_with_encoder(image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 60))?;
    Ok(Rendered { w: img.width(), h: img.height(), tiny: base64(&buf) })
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len() * 4 / 3 + 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
        let n = (b[0] as u32) << 16 | (b[1] as u32) << 8 | b[2] as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

pub fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn index_html(c: &Content, rendered: &[Rendered], centroids: &HashMap<String, String>) -> String {
    let site = &c.site;
    let n = c.photos.len();
    let mut h = String::new();
    h.push_str(&format!(
        r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">
<title>{title}</title>
<meta name="description" content="{tagline}">
<meta name="theme-color" content="#0b0b0a">
<link rel="canonical" href="{base}">
<link rel="stylesheet" href="style.css">
</head>
<body>
<header class="top">
  <a class="name" href="#p01">{name}</a>
  <nav>
    <button type="button" data-panel="statement">Statement</button>
    <button type="button" data-panel="bio">Bio</button>
  </nav>
</header>
<main class="reel" id="reel">
"##,
        title = esc(&site.title),
        tagline = esc(&site.tagline),
        base = esc(&site.base_url),
        name = esc(&site.name),
    ));

    for (i, (p, r)) in c.photos.iter().zip(rendered).enumerate() {
        h.push_str(&slide(p, r, i, n));
    }

    // Closing screen: the words, the contact, and the way back to the start.
    h.push_str(&format!(
        r##"<section class="slide colophon" id="colophon">
  <div>
    <h1>{name}</h1>
    <p class="tagline">{tagline}</p>
    <p class="actions"><button type="button" data-panel="statement">Artist statement</button><button type="button" data-panel="bio">Bio</button></p>
    <p class="contact">"##,
        name = esc(&site.name),
        tagline = esc(&site.tagline),
    ));
    let mut contact: Vec<String> = Vec::new();
    if let Some(e) = &site.email {
        contact.push(format!("<a href=\"mailto:{0}\">{0}</a>", esc(e)));
    }
    if let Some(ig) = &site.instagram {
        contact.push(format!("<a href=\"https://www.instagram.com/{0}/\">@{0}</a>", esc(ig)));
    }
    h.push_str(&contact.join(" &middot; "));
    h.push_str("</p>\n    <p class=\"back\"><a href=\"#p01\">Back to the first picture</a></p>\n  </div>\n</section>\n</main>\n");

    // The sequence as a strip of centroids.
    h.push_str("<nav class=\"strip\" aria-label=\"Sequence\">\n");
    for (i, p) in c.photos.iter().enumerate() {
        h.push_str(&format!(
            "<a href=\"#p{n:02}\" data-n=\"{n}\" style=\"background-color:{hex}\" aria-label=\"Picture {n}\"></a>",
            n = i + 1,
            hex = centroids[&p.stem]
        ));
    }
    h.push_str("</nav>\n");

    // The panel with the words.
    h.push_str("<dialog id=\"panel\" aria-labelledby=\"panel-title\">\n<button type=\"button\" class=\"close\" aria-label=\"Close\">&times;</button>\n");
    h.push_str(&text_block("statement", "Artist statement", &c.statement));
    h.push_str(&text_block("bio", "Bio", &c.bio));
    h.push_str("</dialog>\n<script src=\"site.js\"></script>\n</body>\n</html>\n");
    h
}

fn slide(p: &Photo, r: &Rendered, i: usize, total: usize) -> String {
    let n = i + 1;
    let title = p.meta.title.clone();
    let when = p.when();
    let mut cap = format!("<span class=\"n\">{n:02}</span>");
    if let Some(t) = &title {
        cap.push_str(&format!(" <span class=\"t\">{}</span>", esc(t)));
    }
    let mut detail = Vec::new();
    if let Some(place) = &p.meta.place {
        detail.push(esc(place));
    }
    if !when.is_empty() {
        detail.push(esc(&when));
    }
    if !detail.is_empty() {
        cap.push_str(&format!(" <span class=\"when\">{}</span>", detail.join(", ")));
    }
    if let Some(long) = &p.meta.caption {
        cap.push_str(&format!(" <span class=\"long\">{}</span>", esc(long)));
    }
    let alt = match &title {
        Some(t) => format!("{t}, picture {n} of {total}"),
        None => format!("Picture {n} of {total}"),
    };
    let stem = esc(&p.stem);
    let srcset = |ext: &str| SIZES.iter().map(|s| format!("img/{stem}-{s}.{ext} {s}w")).collect::<Vec<_>>().join(", ");
    // The first two pictures load eagerly; the rest wait until they are near.
    let loading = if i < 2 { "eager" } else { "lazy" };
    format!(
        r#"<section class="slide" id="p{n:02}" data-n="{n}">
  <picture>
    <source type="image/avif" srcset="{avif}" sizes="100vw">
    <img src="img/{stem}-{big}.jpg" srcset="{jpg}" sizes="100vw" width="{w}" height="{h}" loading="{loading}" decoding="async" alt="{alt}" style="background-image:url(data:image/jpeg;base64,{tiny})">
  </picture>
  <p class="cap">{cap}</p>
</section>
"#,
        avif = srcset("avif"),
        jpg = srcset("jpg"),
        big = SIZES[SIZES.len() - 1],
        w = r.w,
        h = r.h,
        alt = esc(&alt),
        tiny = r.tiny,
    )
}

fn text_block(id: &str, heading: &str, paragraphs: &[String]) -> String {
    let mut s = format!(
        "<section class=\"text\" data-for=\"{id}\" hidden>\n<div class=\"section-head\"><h2 id=\"panel-title-{id}\">{heading}</h2><button type=\"button\" class=\"copy\" data-copy=\"{id}-text\" aria-live=\"polite\">Copy</button></div>\n<div id=\"{id}-text\" class=\"prose\">\n"
    );
    for p in paragraphs {
        s.push_str(&format!("<p>{}</p>\n", esc(p)));
    }
    s.push_str("</div>\n</section>\n");
    s
}
