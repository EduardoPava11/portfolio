//! Everything the author edits: `content/site.toml`, `content/bio.txt`,
//! `content/statement.txt`, `content/photos.toml` and the files in `content/photos/`.

use crate::Result;
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Clone)]
pub struct Site {
    pub name: String,
    pub title: String,
    pub tagline: String,
    pub base_url: String,
    pub email: Option<String>,
    pub instagram: Option<String>,
    /// Folder of photographs, relative to content/. Default "photos".
    #[serde(default = "default_photos_dir")]
    pub photos_dir: String,
    /// Used for submission file names: 01_LastName.jpeg.
    pub last_name: Option<String>,
    /// Path to the `matte` binary that chooses and draws the borders.
    pub matte_bin: Option<String>,
    /// Show the framed copies from content/framed/ on the site.
    #[serde(default)]
    pub framed: bool,
}

fn default_photos_dir() -> String {
    "photos".to_string()
}

#[derive(Deserialize, Default)]
struct PhotosFile {
    #[serde(default)]
    photo: Vec<PhotoMeta>,
}

#[derive(Deserialize, Clone, Default)]
pub struct PhotoMeta {
    pub file: String,
    pub title: Option<String>,
    pub place: Option<String>,
    pub caption: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Season {
    Winter,
    Spring,
    Summer,
    Autumn,
}

impl Season {
    /// Meteorological seasons for the northern hemisphere: Dec to Feb is winter,
    /// Mar to May spring, Jun to Aug summer, Sep to Nov autumn.
    pub fn from_month(m: u32) -> Option<Season> {
        Some(match m {
            12 | 1 | 2 => Season::Winter,
            3..=5 => Season::Spring,
            6..=8 => Season::Summer,
            9..=11 => Season::Autumn,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self {
            Season::Winter => "Winter",
            Season::Spring => "Spring",
            Season::Summer => "Summer",
            Season::Autumn => "Autumn",
        }
    }
}

#[derive(Clone)]
pub struct Photo {
    pub meta: PhotoMeta,
    pub path: PathBuf,
    /// The file the site shows: the framed copy when `framed = true`, else the original.
    pub display: PathBuf,
    /// File stem, used for every derived file name.
    pub stem: String,
    /// EXIF DateTimeOriginal as "YYYY-MM-DD HH:MM:SS" when present.
    pub taken: Option<String>,
    pub year: Option<u32>,
    pub season: Option<Season>,
    /// EXIF orientation tag, 1 when absent.
    pub orientation: u32,
}

impl Photo {
    /// "Autumn 2025", "2025", or "" depending on what the file knows about itself.
    pub fn when(&self) -> String {
        match (self.season, self.year) {
            (Some(s), Some(y)) => format!("{} {}", s.name(), y),
            (None, Some(y)) => y.to_string(),
            _ => String::new(),
        }
    }
    pub fn title(&self) -> String {
        self.meta.title.clone().unwrap_or_else(|| self.stem.clone())
    }
}

pub struct Content {
    pub site: Site,
    pub bio: Vec<String>,
    pub statement: Vec<String>,
    pub photos: Vec<Photo>,
}

/// Load with the originals as the display files, whatever site.toml says about framing.
/// `lab`, `red` and `frame` measure and frame the photographs themselves.
pub fn load_originals(root: &Path) -> Result<Content> {
    let mut c = load_inner(root, false)?;
    for p in &mut c.photos {
        p.display = p.path.clone();
    }
    Ok(c)
}

pub fn load(root: &Path) -> Result<Content> {
    load_inner(root, true)
}

fn load_inner(root: &Path, want_framed: bool) -> Result<Content> {
    let dir = root.join("content");
    let site: Site = toml::from_str(&fs::read_to_string(dir.join("site.toml"))?)?;
    let bio = paragraphs(&fs::read_to_string(dir.join("bio.txt"))?);
    let statement = paragraphs(&fs::read_to_string(dir.join("statement.txt"))?);
    let listed: PhotosFile = toml::from_str(&fs::read_to_string(dir.join("photos.toml"))?)?;
    let order = read_order(&dir.join("order.txt"));
    let mut photos = load_photos(&dir.join(&site.photos_dir), listed.photo, order)?;
    if site.framed && want_framed {
        for p in &mut photos {
            let framed = dir.join("framed").join(format!("{}.jpg", p.stem));
            if !framed.exists() {
                return Err(format!(
                    "site.toml says framed = true but {} does not exist; run `portfolio frame` first or set framed = false",
                    framed.display()
                )
                .into());
            }
            p.display = framed;
        }
    }
    Ok(Content { site, bio, statement, photos })
}

/// content/order.txt: one file stem per line, written by `portfolio lab`. Lines starting
/// with # are comments. Absent means: photos.toml order, then alphabetical.
fn read_order(path: &Path) -> Option<Vec<String>> {
    let text = fs::read_to_string(path).ok()?;
    Some(
        text.lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_string)
            .collect(),
    )
}

/// Blank line separated paragraphs, trimmed, empties dropped.
fn paragraphs(text: &str) -> Vec<String> {
    text.split("\n\n")
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty())
        .collect()
}

fn is_image(p: &Path) -> bool {
    matches!(
        p.extension().and_then(|e| e.to_str()).map(|e| e.to_ascii_lowercase()).as_deref(),
        Some("jpg" | "jpeg" | "png" | "tif" | "tiff")
    )
}

fn stem_of(p: &Path) -> String {
    p.file_stem().unwrap().to_string_lossy().into_owned()
}

fn load_photos(dir: &Path, listed: Vec<PhotoMeta>, order: Option<Vec<String>>) -> Result<Vec<Photo>> {
    let mut on_disk: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| is_image(p)).collect(),
        Err(e) => return Err(format!("cannot read {}: {e}", dir.display()).into()),
    };
    on_disk.sort();
    let meta_for = |path: &Path| -> PhotoMeta {
        let name = path.file_name().unwrap().to_string_lossy();
        listed
            .iter()
            .find(|m| m.file == name || m.file == stem_of(path))
            .cloned()
            .unwrap_or(PhotoMeta { file: name.into_owned(), ..Default::default() })
    };

    let mut ordered: Vec<(PhotoMeta, PathBuf)> = Vec::new();
    if let Some(order) = order {
        // Colour order from `portfolio lab`; captions still come from photos.toml.
        for stem in order {
            match on_disk.iter().position(|p| stem_of(p) == stem) {
                Some(i) => {
                    let path = on_disk.remove(i);
                    ordered.push((meta_for(&path), path));
                }
                None => eprintln!("warning: order.txt lists {stem} but the photo folder has no such file"),
            }
        }
        for path in on_disk {
            ordered.push((meta_for(&path), path));
        }
        return finish(ordered);
    }
    for meta in listed.clone() {
        match on_disk.iter().position(|p| p.file_name().and_then(|n| n.to_str()) == Some(meta.file.as_str())) {
            Some(i) => {
                let path = on_disk.remove(i);
                ordered.push((meta, path));
            }
            None => eprintln!("warning: photos.toml lists {} but content/photos/ has no such file", meta.file),
        }
    }
    for path in on_disk {
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        ordered.push((PhotoMeta { file, ..Default::default() }, path));
    }
    finish(ordered)
}

fn finish(ordered: Vec<(PhotoMeta, PathBuf)>) -> Result<Vec<Photo>> {
    let mut photos = Vec::with_capacity(ordered.len());
    for (meta, path) in ordered {
        let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
        let (taken, orientation) = read_exif(&path);
        let (year, season) = match &taken {
            Some(t) if t.len() >= 7 => {
                let year = t[0..4].parse().ok();
                let season = t[5..7].parse().ok().and_then(Season::from_month);
                (year, season)
            }
            _ => (None, None),
        };
        let display = path.clone();
        photos.push(Photo { meta, path, display, stem, taken, year, season, orientation });
    }
    Ok(photos)
}

/// (DateTimeOriginal as text, orientation). Missing EXIF is not an error: the file
/// simply has no date and is shown upright as stored.
fn read_exif(path: &Path) -> (Option<String>, u32) {
    let Ok(file) = fs::File::open(path) else { return (None, 1) };
    let mut reader = std::io::BufReader::new(file);
    let Ok(exif) = exif::Reader::new().read_from_container(&mut reader) else { return (None, 1) };
    let taken = exif
        .get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)
        .or_else(|| exif.get_field(exif::Tag::DateTime, exif::In::PRIMARY))
        .map(|f| f.display_value().to_string())
        .filter(|s| s.len() >= 10 && s.as_bytes()[4] == b'-');
    let orientation = exif
        .get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|f| f.value.get_uint(0))
        .unwrap_or(1);
    (taken, orientation)
}
