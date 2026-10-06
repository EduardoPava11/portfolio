//! Rename the photographs so that their file names carry the colour order:
//! `01-<original>.jpg`, `02-<original>.jpg`, ... A folder listing then shows the
//! colour transition. Any existing `NN-` prefix is stripped first, so running this
//! again after a new `lab` reading renumbers cleanly. Framed copies, order.txt,
//! selection.txt and photos.toml captions are renamed to match.

use crate::content;
use crate::Result;
use std::fs;
use std::path::Path;

/// "07-000075030001" -> "000075030001"; "end" -> "end".
pub fn strip_prefix(stem: &str) -> &str {
    match stem.split_once('-') {
        Some((n, rest)) if !n.is_empty() && n.len() <= 3 && n.chars().all(|c| c.is_ascii_digit()) => rest,
        _ => stem,
    }
}

pub fn run(root: &Path) -> Result<()> {
    let content = content::load_originals(root)?;
    let dir = root.join("content");
    let photos_dir = dir.join(&content.site.photos_dir);
    let framed_dir = dir.join("framed");
    let width = if content.photos.len() >= 100 { 3 } else { 2 };

    let mut pairs: Vec<(String, String)> = Vec::new(); // (old stem, new stem)
    for (i, p) in content.photos.iter().enumerate() {
        let new_stem = format!("{:0width$}-{}", i + 1, strip_prefix(&p.stem), width = width);
        if new_stem != p.stem {
            pairs.push((p.stem.clone(), new_stem));
        }
    }
    if pairs.is_empty() {
        println!("rename: names already carry the colour order");
        return Ok(());
    }
    // Two passes through temporary names so that a swap (03 -> 05 while 05 -> 03) cannot collide.
    let tmp = |s: &str| format!("renaming-{s}");
    for (old, _) in &pairs {
        move_stem(&photos_dir, old, &tmp(old))?;
        move_stem(&framed_dir, old, &tmp(old))?;
    }
    for (old, new) in &pairs {
        move_stem(&photos_dir, &tmp(old), new)?;
        move_stem(&framed_dir, &tmp(old), new)?;
    }
    for (old, new) in &pairs {
        println!("  {old}  ->  {new}");
    }

    // Lists and captions follow.
    for file in ["order.txt", "selection.txt"] {
        let path = dir.join(file);
        if let Ok(text) = fs::read_to_string(&path) {
            let out: Vec<String> = text
                .lines()
                .map(|l| {
                    let t = l.trim();
                    pairs.iter().find(|(o, _)| o == t).map(|(_, n)| n.clone()).unwrap_or_else(|| l.to_string())
                })
                .collect();
            fs::write(&path, out.join("\n") + "\n")?;
        }
    }
    let toml_path = dir.join("photos.toml");
    let mut toml_text = fs::read_to_string(&toml_path)?;
    for (old, new) in &pairs {
        toml_text = toml_text.replace(&format!("file = \"{old}"), &format!("file = \"{new}"));
    }
    fs::write(&toml_path, toml_text)?;
    println!("rename: {} files renumbered in colour order", pairs.len());
    Ok(())
}

/// Rename every file in `dir` whose stem is `old` to the same extension under `new`.
fn move_stem(dir: &Path, old: &str, new: &str) -> Result<()> {
    let Ok(rd) = fs::read_dir(dir) else { return Ok(()) };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.file_stem().and_then(|s| s.to_str()) == Some(old) {
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            let target = dir.join(if ext.is_empty() { new.to_string() } else { format!("{new}.{ext}") });
            fs::rename(&path, &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::strip_prefix;
    #[test]
    fn prefixes() {
        assert_eq!(strip_prefix("07-000075030001"), "000075030001");
        assert_eq!(strip_prefix("55-end"), "end");
        assert_eq!(strip_prefix("end"), "end");
        assert_eq!(strip_prefix("000075030009-1 (dragged)"), "000075030009-1 (dragged)");
        assert_eq!(strip_prefix("12-000075030009-1 (dragged)"), "000075030009-1 (dragged)");
    }
}
