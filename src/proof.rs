//! Mat proof: the candidate mat laws rendered on seven pictures spread across the
//! sequence, one row per law, so the choice is made from pictures rather than words.
//! Rendering here is a mock of matte's geometry (6% border, bottom weight 1.15) at
//! small size; the real frames are cut by `portfolio frame`.
//!
//!   portfolio proof            -> analysis/mats/index.html

use crate::content;
use crate::images;
use crate::lab::{self, Lab, MatOptions, Reading};
use crate::site::esc;
use crate::Result;
use image::{Rgb, RgbImage};
use rayon::prelude::*;
use std::fs;
use std::path::Path;

const SAMPLES: usize = 7;
const SIDE: u32 = 360;

/// (label, flags)
const LAWS: &[(&str, &str)] = &[
    ("Own hue, paper register", "--hue own --register paper --floor mute --intensity statement"),
    ("Own hue, ink register", "--hue own --register ink --floor mute --intensity statement"),
    ("Own hue, mid register (current)", "--hue own --register mid --floor mute --intensity statement"),
    ("Opponent hue, mid register", "--hue opponent --register mid --floor mute --intensity statement"),
    ("Opponent hue, paper register", "--hue opponent --register paper --floor mute --intensity balanced"),
    ("Opponent hue, ink register", "--hue opponent --register ink --floor mute --intensity balanced"),
    ("Split hue, mid register", "--hue split --register mid --floor mute --intensity statement"),
    ("Own hue, light register, balanced centre", "--hue own --register light --floor mute --intensity balanced"),
];

pub fn run(root: &Path) -> Result<()> {
    let content = content::load_originals(root)?;
    let out = root.join("analysis/mats");
    fs::create_dir_all(&out)?;
    let scratch = out.join("read");
    fs::create_dir_all(&scratch)?;

    // Read the whole body once (the mats depend on the set's statistics), in order.
    let mut readings: Vec<Reading> = content.photos.par_iter().map(|p| lab::read(p, &scratch)).collect::<Result<Vec<_>>>()?;
    lab::order_by_colour(&mut readings);
    let n = readings.len();
    let picks: Vec<usize> = (0..SAMPLES).map(|k| (k * (n - 1)) / (SAMPLES - 1).max(1)).collect();

    // Small upright copies of the sample pictures.
    let samples: Vec<(usize, RgbImage)> = picks
        .par_iter()
        .map(|&i| -> Result<(usize, RgbImage)> {
            let photo = content.photos.iter().find(|p| p.stem == readings[i].stem).ok_or("sample vanished")?;
            Ok((i, images::fit(&images::open_upright(photo)?, SIDE)))
        })
        .collect::<Result<Vec<_>>>()?;

    let mut html = String::from(
        r##"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Mat proof</title><style>
body { margin: 0; padding: 24px 16px; background: #0b0b0a; color: #ece7df; font: 15px/1.5 Georgia, serif; }
h1 { font-weight: normal; font-size: 1.3rem; margin: 0 0 0.3rem; }
.sub { color: #9a938a; margin: 0 0 1.5rem; max-width: 80ch; }
.law { margin: 0 0 2rem; }
.law h2 { font-weight: normal; font-size: 1rem; margin: 0 0 0.2rem; }
.law code { color: #9a938a; font-size: 0.85rem; }
.row { display: flex; gap: 6px; margin-top: 0.6rem; overflow-x: auto; }
.row img { height: 150px; width: auto; display: block; }
.row figure { margin: 0; text-align: center; color: #9a938a; font-size: 0.75rem; }
</style></head><body>
<h1>Mat proof</h1>
<p class="sub">Each row is one mat law applied across the sequence: pictures 1, 10, 19, 28, 37, 46, 55 from left to right, so the arc from the quiet ends to the full centre is visible. Pick a row, then run <code>portfolio lab &lt;flags&gt;</code> and <code>portfolio frame --force</code>. Geometry here is a mock at small size; matte cuts the real frames.</p>
"##,
    );

    for (li, (label, flags)) in LAWS.iter().enumerate() {
        let args: Vec<String> = flags.split_whitespace().map(str::to_string).collect();
        let o = MatOptions::parse(&args)?;
        let (l, floor, peak) = lab::derive_mats(&mut readings, &o);
        html.push_str(&format!(
            "<div class=\"law\"><h2>{}</h2><code>portfolio lab {}</code> <code>L* {l:.0}, C* {floor:.0} to {peak:.0}</code>\n<div class=\"row\">",
            esc(label),
            esc(flags)
        ));
        for (i, img) in &samples {
            let mat = readings[*i].mat.unwrap();
            let framed = mock_frame(img, mat);
            let name = format!("law{li}-{:02}.jpg", i + 1);
            images::write_jpeg(&framed, &out.join(&name))?;
            html.push_str(&format!("<figure><img src=\"{name}\" alt=\"\"><figcaption>{:02} {}</figcaption></figure>", i + 1, mat.hex()));
        }
        html.push_str("</div></div>\n");
    }
    html.push_str("</body></html>\n");
    fs::write(out.join("index.html"), html)?;
    let _ = fs::remove_dir_all(&scratch);
    println!("proof: {} laws x {} pictures -> {}", LAWS.len(), SAMPLES, out.join("index.html").display());
    Ok(())
}

/// Matte's default geometry at proof scale: border 6% of the long edge, bottom 1.15x.
fn mock_frame(img: &RgbImage, mat: Lab) -> RgbImage {
    let (w, h) = img.dimensions();
    let b = (w.max(h) as f32 * 0.06).round() as u32;
    let bottom = (b as f32 * 1.15).round() as u32;
    let [r, g, bl] = mat.to_srgb8();
    let mut canvas = RgbImage::from_pixel(w + 2 * b, h + b + bottom, Rgb([r, g, bl]));
    image::imageops::overlay(&mut canvas, img, b as i64, b as i64);
    canvas
}
