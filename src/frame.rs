//! Borders. Delegates to the `matte` CLI (~/MATTE), which measures the set in
//! CAM16-UCS, chooses a mat colour from the pictures' own statistics, and grows the
//! canvas around each photograph without resampling it. Framed copies land in
//! content/framed/<stem>.jpg and are what the site and the export show.
//!
//!   portfolio frame                      top ranked candidate
//!   portfolio frame --pick self-light    a candidate id from `matte analyze`
//!   portfolio frame -- --color "#D6B9A1" --border 0.08    anything else matte accepts

use crate::content;
use crate::Result;
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let content = content::load_originals(root)?;
    let matte = content.site.matte_bin.clone().ok_or("site.toml needs matte_bin = \"/path/to/matte\"")?;
    if !Path::new(&matte).exists() {
        return Err(format!("matte binary not found at {matte}; build it with `cargo build --release` in ~/MATTE").into());
    }
    let out = root.join("content/framed");
    fs::create_dir_all(&out)?;

    let mut cmd = Command::new(&matte);
    cmd.arg("frame");
    for p in &content.photos {
        cmd.arg(&p.path);
    }
    cmd.arg("--out").arg(&out).arg("-f").arg("jpeg").arg("--quality").arg("96");
    let extra: Vec<&String> = args.iter().filter(|a| a.as_str() != "--").collect();
    cmd.args(extra);

    println!("frame: running matte on {} photographs", content.photos.len());
    let status = cmd.status()?;
    if !status.success() {
        return Err(format!("matte frame exited with {status}").into());
    }

    // matte names outputs after the source; make sure every photograph has a framed copy
    // at the name the site expects, whatever extension matte chose.
    let mut missing = Vec::new();
    for p in &content.photos {
        let want = out.join(format!("{}.jpg", p.stem));
        if want.exists() {
            continue;
        }
        // matte writes <stem>_framed.<ext>.
        let alt = ["_framed.jpg", "_framed.jpeg", ".jpeg"].iter().map(|e| out.join(format!("{}{e}", p.stem))).find(|a| a.exists());
        match alt {
            Some(a) => fs::rename(a, &want)?,
            None => missing.push(p.stem.clone()),
        }
    }
    if !missing.is_empty() {
        return Err(format!("matte wrote no framed copy for: {}", missing.join(", ")).into());
    }
    println!("frame: {} framed copies in {}", content.photos.len(), out.display());
    Ok(())
}
