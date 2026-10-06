//! Design check: drives headless Chrome through a matrix of viewports and screens,
//! writes the screenshots into one sheet, and runs the static checks that a phone
//! layout and a bold page need. Fails (exit 1) when a static check fails.
//!
//!   portfolio check            -> analysis/check/index.html

use crate::content;
use crate::site::esc;
use crate::Result;
use std::fs;
use std::path::Path;
use std::process::Command;

const CHROME: &[&str] = &[
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/Applications/Chromium.app/Contents/MacOS/Chromium",
    "/Applications/Brave Browser.app/Contents/MacOS/Brave Browser",
];
/// (name, width, height)
const VIEWPORTS: &[(&str, u32, u32)] = &[
    ("phone", 390, 844),
    ("phone wide", 430, 932),
    ("phone landscape", 844, 390),
    ("tablet", 834, 1194),
    ("laptop", 1440, 900),
];
const SCREENS: &[&str] = &["start", "p01", "p28", "p55", "statement", "bio"];

struct Check {
    name: &'static str,
    pass: bool,
    detail: String,
}

pub fn run(root: &Path) -> Result<()> {
    let docs = root.join("docs");
    let html = fs::read_to_string(docs.join("index.html")).map_err(|_| "docs/index.html is missing; run `portfolio build` first")?;
    let css = fs::read_to_string(docs.join("style.css"))?;
    let js = fs::read_to_string(docs.join("site.js"))?;
    let content = content::load(root)?;
    let out = root.join("analysis/check");
    fs::create_dir_all(&out)?;

    // Static checks: what a phone needs, what bold needs.
    let mut checks = Vec::new();
    let push = |v: &mut Vec<Check>, name: &'static str, pass: bool, detail: String| v.push(Check { name, pass, detail });
    push(&mut checks, "viewport meta with width=device-width", html.contains("width=device-width"), String::new());
    push(&mut checks, "viewport-fit=cover for notched phones", html.contains("viewport-fit=cover"), String::new());
    push(&mut checks, "safe area insets used", css.contains("env(safe-area-inset-top)") && css.contains("env(safe-area-inset-bottom)"), String::new());
    push(&mut checks, "dynamic viewport height (100dvh)", css.contains("100dvh"), String::new());
    push(&mut checks, "coarse pointer rules (bigger targets on touch)", css.contains("(pointer: coarse)"), String::new());
    push(&mut checks, "reduced motion respected", css.contains("prefers-reduced-motion"), String::new());
    let smallest = smallest_rem(&css);
    push(&mut checks, "smallest type at least 0.75rem (12px)", smallest >= 0.75, format!("smallest font-size {smallest:.2}rem"));
    push(&mut checks, "every picture has width and height attributes", html.matches("<img ").count() == html.matches(" width=\"").count(), format!("{} img", html.matches("<img ").count()));
    push(&mut checks, "AVIF source on every picture", html.matches("type=\"image/avif\"").count() == content.photos.len(), String::new());
    push(&mut checks, "lazy loading beyond the first screens", html.contains("loading=\"lazy\""), String::new());
    push(&mut checks, "inline placeholders", html.matches("data:image/jpeg;base64").count() == content.photos.len(), String::new());
    push(&mut checks, "bold weight loaded for the headline", html.contains("wght@") && (html.contains("0,700") || html.contains(";700")), String::new());
    push(&mut checks, "copy buttons present", html.matches("class=\"copy\"").count() >= 2, String::new());
    let shell = html.len() + css.len() + js.len();
    let first_images: u64 = ["img/".to_string()]
        .iter()
        .flat_map(|_| content.photos.iter().take(2))
        .map(|p| fs::metadata(docs.join("img").join(format!("{}-1200.avif", p.stem))).map(|m| m.len()).unwrap_or(0))
        .sum();
    push(&mut checks, "first screen under 600 KB (shell + two AVIFs at 1200)", shell as u64 + first_images < 600_000, format!("{} KB shell + {} KB images", shell / 1000, first_images / 1000));
    push(&mut checks, "no em or en dashes in the page", !html.contains('\u{2013}') && !html.contains('\u{2014}'), String::new());

    // Screenshots through headless Chrome.
    let chrome = CHROME.iter().find(|p| Path::new(p).exists());
    let mut shots: Vec<(String, String, u32, u32)> = Vec::new();
    match chrome {
        None => push(&mut checks, "headless Chrome available", false, "no Chrome, Chromium or Brave in /Applications".into()),
        Some(chrome) => {
            // Chrome will not shrink its window below a few hundred pixels, so each
            // viewport is an iframe of the exact size inside a large window, and the
            // screenshot is cropped to the iframe. The iframe gives the page a true
            // layout viewport, so 100dvh, media queries and the strip all behave.
            let page_url = format!("file://{}/index.html", docs.canonicalize()?.display());
            for (vname, w, h) in VIEWPORTS {
                for screen in SCREENS {
                    let slug = format!("{}-{screen}", vname.replace(' ', "-"));
                    let wrapper = out.join(format!("{slug}.html"));
                    fs::write(
                        &wrapper,
                        format!(
                            "<!doctype html><html><head><meta charset=\"utf-8\"><style>html,body{{margin:0;background:#000}}iframe{{display:block;border:0;width:{w}px;height:{h}px}}</style></head><body><iframe src=\"{page_url}#{screen}\"></iframe></body></html>"
                        ),
                    )?;
                    let raw = out.join(format!("{slug}.raw.png"));
                    let status = Command::new(chrome)
                        .args([
                            "--headless=new",
                            "--disable-gpu",
                            "--hide-scrollbars",
                            "--force-device-scale-factor=1",
                            "--virtual-time-budget=5000",
                            &format!("--window-size={},{}", w + 40, h + 40),
                            &format!("--screenshot={}", raw.display()),
                            &format!("file://{}", wrapper.display()),
                        ])
                        .output()?;
                    let ok = status.status.success() && raw.exists();
                    if ok {
                        let img = image::open(&raw)?;
                        let cropped = img.crop_imm(0, 0, (*w).min(img.width()), (*h).min(img.height()));
                        cropped.save(out.join(format!("{slug}.png")))?;
                        let _ = fs::remove_file(&raw);
                        let _ = fs::remove_file(&wrapper);
                        shots.push((vname.to_string(), screen.to_string(), *w, *h));
                    } else {
                        eprintln!("warning: screenshot failed for {vname} #{screen}");
                    }
                }
            }
            push(&mut checks, "screenshots captured", shots.len() == VIEWPORTS.len() * SCREENS.len(), format!("{} of {}", shots.len(), VIEWPORTS.len() * SCREENS.len()));
        }
    }

    // The sheet.
    let mut page = String::from(
        r##"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Design check</title><style>
body { margin: 0; padding: 24px 16px; background: #0b0b0a; color: #ece7df; font: 15px/1.5 'IBM Plex Sans', system-ui, sans-serif; }
h1 { font-weight: 600; font-size: 1.3rem; margin: 0 0 0.3rem; }
h2 { font-weight: 500; font-size: 1rem; margin: 2rem 0 0.6rem; color: #9a938a; }
ul.checks { list-style: none; padding: 0; margin: 0 0 1rem; max-width: 70ch; }
ul.checks li { padding: 4px 0; border-bottom: 1px solid #2a2825; display: flex; gap: 10px; }
ul.checks .ok { color: #8fd18f; } ul.checks .bad { color: #e07a6a; }
ul.checks small { color: #9a938a; margin-left: auto; }
.row { display: flex; gap: 12px; overflow-x: auto; padding-bottom: 8px; }
.row figure { margin: 0; flex: none; }
.row img { display: block; height: 420px; width: auto; background: #000; border: 1px solid #2a2825; }
.row figcaption { color: #9a938a; font-size: 0.8rem; margin-top: 4px; }
</style></head><body>
<h1>Design check</h1>
"##,
    );
    let failed = checks.iter().filter(|c| !c.pass).count();
    page.push_str(&format!("<p>{} checks, {} failed.</p>\n<ul class=\"checks\">\n", checks.len(), failed));
    for c in &checks {
        page.push_str(&format!(
            "<li><span class=\"{}\">{}</span> {}<small>{}</small></li>\n",
            if c.pass { "ok" } else { "bad" },
            if c.pass { "pass" } else { "FAIL" },
            esc(c.name),
            esc(&c.detail)
        ));
    }
    page.push_str("</ul>\n");
    for (vname, w, h) in VIEWPORTS {
        page.push_str(&format!("<h2>{} ({w} x {h})</h2>\n<div class=\"row\">\n", esc(vname)));
        for (sv, screen, _, _) in shots.iter().filter(|s| &s.0 == vname) {
            let file = format!("{}-{screen}.png", sv.replace(' ', "-"));
            page.push_str(&format!("<figure><img src=\"{file}\" alt=\"\"><figcaption>#{screen}</figcaption></figure>\n"));
        }
        page.push_str("</div>\n");
    }
    page.push_str("</body></html>\n");
    fs::write(out.join("index.html"), page)?;

    for c in &checks {
        println!("  {} {}{}", if c.pass { "pass" } else { "FAIL" }, c.name, if c.detail.is_empty() { String::new() } else { format!(" ({})", c.detail) });
    }
    println!("check: {} screenshots, {} of {} checks failed -> {}", shots.len(), failed, checks.len(), out.join("index.html").display());
    if failed > 0 {
        return Err(format!("{failed} design check(s) failed").into());
    }
    Ok(())
}

/// The smallest font-size written in rem in the stylesheet.
fn smallest_rem(css: &str) -> f32 {
    let mut min = f32::MAX;
    for (i, _) in css.match_indices("font-size:") {
        let rest = &css[i + 10..];
        let token: String = rest.trim_start().chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
        if rest.trim_start()[token.len()..].starts_with("rem") {
            if let Ok(v) = token.parse::<f32>() {
                min = min.min(v);
            }
        }
    }
    if min == f32::MAX { 1.0 } else { min }
}
