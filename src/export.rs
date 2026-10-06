//! The submission set. Exposure's open calls want JPEGs, at most 5 MB each, named
//! `01_LastName.jpeg` with the sequence number first. This writes exactly that from
//! `content/selection.txt` (stems, in order), using the framed copies when the site
//! does, and lowers JPEG quality one step at a time until each file fits.
//!
//! If selection.txt does not exist it is written with every photograph in colour
//! order, so the first run of `export` produces a file to prune.

use crate::content;
use crate::images;
use crate::Result;
use image::codecs::jpeg::JpegEncoder;
use std::fs;
use std::path::Path;

/// Exposure: "maximum of 5MB in size". Taken as 5,000,000 bytes, the stricter reading.
const MAX_BYTES: usize = 5_000_000;
/// North West Showcase: 8 to 15 images. International Open Call: 5 to 10.
const SHOWCASE: (usize, usize) = (8, 15);
const INTERNATIONAL: (usize, usize) = (5, 10);

pub fn run(root: &Path) -> Result<()> {
    let content = content::load(root)?;
    let last = content.site.last_name.clone().ok_or("site.toml needs last_name for the file names")?;
    let sel_path = root.join("content/selection.txt");
    if !sel_path.exists() {
        let mut s = String::from("# Submission selection written by `portfolio export`: every photograph in colour order.\n# Delete lines until the set is right (North West Showcase 8 to 15, International 5 to 10),\n# reorder if you like, then run `portfolio export` again.\n");
        for p in &content.photos {
            s.push_str(&p.stem);
            s.push('\n');
        }
        fs::write(&sel_path, s)?;
        println!("export: wrote {} with all {} photographs; prune it and run again", sel_path.display(), content.photos.len());
        return Ok(());
    }
    let stems: Vec<String> = fs::read_to_string(&sel_path)?
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect();

    let out = root.join("export");
    if out.exists() {
        fs::remove_dir_all(&out)?;
    }
    fs::create_dir_all(&out)?;

    println!("export: {} photographs -> {}", stems.len(), out.display());
    for (i, stem) in stems.iter().enumerate() {
        let photo = content
            .photos
            .iter()
            .find(|p| &p.stem == stem)
            .ok_or_else(|| format!("selection.txt lists {stem} but there is no such photograph"))?;
        let img = images::open_display(photo)?.to_rgb8();
        let name = format!("{:02}_{}.jpeg", i + 1, last);
        let path = out.join(&name);
        let mut quality = 95u8;
        let bytes = loop {
            let mut buf = Vec::new();
            img.write_with_encoder(JpegEncoder::new_with_quality(&mut buf, quality))?;
            if buf.len() <= MAX_BYTES || quality <= 60 {
                fs::write(&path, &buf)?;
                break buf.len();
            }
            quality -= 1;
        };
        if bytes > MAX_BYTES {
            return Err(format!("{name} is still {bytes} bytes at quality {quality}; the image is too large for 5 MB").into());
        }
        println!("  {name:<20} {:>5.2} MB  q{quality}  {}x{}  from {stem}", bytes as f64 / 1e6, img.width(), img.height());
    }

    let n = stems.len();
    let fits = |(lo, hi): (usize, usize)| n >= lo && n <= hi;
    println!(
        "export: {n} files. North West Showcase (8 to 15): {}. International Open Call (5 to 10): {}.",
        if fits(SHOWCASE) { "OK" } else { "NOT within range" },
        if fits(INTERNATIONAL) { "OK" } else { "NOT within range" }
    );
    Ok(())
}
