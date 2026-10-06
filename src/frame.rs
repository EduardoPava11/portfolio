//! Borders. Delegates to the `matte` CLI (~/MATTE), which measures the set in
//! CAM16-UCS, chooses a mat colour from the pictures' own statistics, and grows the
//! canvas around each photograph without resampling it. Framed copies land in
//! content/framed/<stem>.jpg and are what the site and the export show.
//!
//! Each picture is framed in ITS OWN mat colour: its hue, at the lightness and
//! intensity the CIELAB reading derived for the set (see `lab --register --intensity`).
//! The colour comes from analysis/lab/lab.csv, so run `portfolio lab` first.
//!
//!   portfolio frame                         frame what is missing or stale
//!   portfolio frame --force                 frame everything again
//!   portfolio frame -- --border 0.08        anything else matte accepts

use crate::content;
use crate::Result;
use std::fs;
use std::path::Path;
use std::process::Command;

/// stem -> mat hex from the CIELAB reading.
fn read_mats(root: &Path) -> Result<std::collections::HashMap<String, String>> {
    let path = root.join("analysis/lab/lab.csv");
    let text = fs::read_to_string(&path).map_err(|_| format!("{} is missing; run `portfolio lab` first", path.display()))?;
    let mut map = std::collections::HashMap::new();
    for line in text.lines().skip(1) {
        let cols: Vec<&str> = line.split(',').collect();
        if cols.len() >= 15 && cols[14].starts_with('#') {
            map.insert(cols[1].to_string(), cols[14].to_string());
        }
    }
    if map.is_empty() {
        return Err(format!("{} has no mat_hex column; run `portfolio lab` again", path.display()).into());
    }
    Ok(map)
}

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let content = content::load_originals(root)?;
    let matte = content.site.matte_bin.clone().ok_or("site.toml needs matte_bin = \"/path/to/matte\"")?;
    if !Path::new(&matte).exists() {
        return Err(format!("matte binary not found at {matte}; build it with `cargo build --release` in ~/MATTE").into());
    }
    let mats = read_mats(root)?;
    let lab_csv = root.join("analysis/lab/lab.csv");
    let out = root.join("content/framed");
    fs::create_dir_all(&out)?;
    let extra: Vec<&String> = args.iter().filter(|a| a.as_str() != "--").collect();
    let force = extra.iter().any(|a| a.as_str() == "--force");
    let extra: Vec<&String> = extra.into_iter().filter(|a| a.as_str() != "--force").collect();

    let started = std::time::Instant::now();
    let mut done = 0;
    for p in &content.photos {
        let mat = mats.get(&p.stem).ok_or_else(|| format!("no mat for {} in lab.csv; run `portfolio lab`", p.stem))?;
        let want = out.join(format!("{}.jpg", p.stem));
        if !force && crate::images::up_to_date(&p.path, &want) && crate::images::up_to_date(&lab_csv, &want) {
            continue;
        }
        let mut cmd = Command::new(&matte);
        cmd.arg("frame").arg(&p.path).arg("--color").arg(mat).arg("--out").arg(&out).arg("-f").arg("jpeg").arg("--quality").arg("96");
        cmd.args(&extra);
        let output = cmd.output()?;
        if !output.status.success() {
            return Err(format!("matte frame failed on {}: {}", p.stem, String::from_utf8_lossy(&output.stderr)).into());
        }
        // matte writes <stem>_framed.<ext>; the site wants <stem>.jpg.
        let alt = ["_framed.jpg", "_framed.jpeg"].iter().map(|e| out.join(format!("{}{e}", p.stem))).find(|a| a.exists());
        match alt {
            Some(a) => fs::rename(a, &want)?,
            None => return Err(format!("matte wrote no framed copy for {}", p.stem).into()),
        }
        done += 1;
        println!("  {:<36} mat {mat}  ({:.0} s)", p.stem, started.elapsed().as_secs_f32());
    }
    println!("frame: {done} framed, {} already current, in {}", content.photos.len() - done, out.display());
    Ok(())
}
