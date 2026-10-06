//! Opening photographs the way a viewer would see them (EXIF orientation applied)
//! and writing the web sized copies.

use crate::content::Photo;
use crate::Result;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::{DynamicImage, ImageReader, RgbImage};
use std::fs;
use std::path::Path;

const JPEG_QUALITY: u8 = 88;

/// Decode the original and rotate so that the pixels are upright.
pub fn open_upright(photo: &Photo) -> Result<DynamicImage> {
    open_path(&photo.path, photo.orientation)
}

/// Decode the file the site shows (framed copy or original).
pub fn open_display(photo: &Photo) -> Result<DynamicImage> {
    let o = if photo.display == photo.path { photo.orientation } else { 1 };
    open_path(&photo.display, o)
}

pub fn open_path(path: &Path, orientation: u32) -> Result<DynamicImage> {
    let img = ImageReader::open(path)?
        .with_guessed_format()?
        .decode()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(apply_orientation(img, orientation))
}

/// EXIF orientation values 1 to 8 (TIFF 6.0, section 8).
fn apply_orientation(img: DynamicImage, o: u32) -> DynamicImage {
    match o {
        2 => img.fliph(),
        3 => img.rotate180(),
        4 => img.flipv(),
        5 => img.rotate90().fliph(),
        6 => img.rotate90(),
        7 => img.rotate270().fliph(),
        8 => img.rotate270(),
        _ => img,
    }
}

/// Shrink so the longest side is at most `side`; never enlarge.
pub fn fit(img: &DynamicImage, side: u32) -> RgbImage {
    let (w, h) = (img.width(), img.height());
    if w.max(h) <= side {
        return img.to_rgb8();
    }
    img.resize(side, side, FilterType::Lanczos3).to_rgb8()
}

pub fn write_jpeg(img: &RgbImage, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out = std::io::BufWriter::new(fs::File::create(path)?);
    let enc = JpegEncoder::new_with_quality(&mut out, JPEG_QUALITY);
    img.write_with_encoder(enc)?;
    Ok(())
}

/// True when `out` exists and is newer than `src`, so the work can be skipped.
pub fn up_to_date(src: &Path, out: &Path) -> bool {
    let (Ok(s), Ok(o)) = (fs::metadata(src), fs::metadata(out)) else { return false };
    match (s.modified(), o.modified()) {
        (Ok(sm), Ok(om)) => om >= sm,
        _ => false,
    }
}

/// AVIF via ravif (pure Rust AV1). Quality 62 at speed 6 lands near half the bytes of
/// the JPEG at the same visual quality on this material.
pub fn write_avif(img: &RgbImage, path: &Path) -> Result<()> {
    use ravif::{Encoder, Img, RGB8};
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let pixels: Vec<RGB8> = img.pixels().map(|p| RGB8::new(p[0], p[1], p[2])).collect();
    let encoded = Encoder::new()
        .with_quality(62.0)
        .with_speed(6)
        .encode_rgb(Img::new(pixels.as_slice(), img.width() as usize, img.height() as usize))
        .map_err(|e| format!("avif {}: {e}", path.display()))?;
    fs::write(path, encoded.avif_file)?;
    Ok(())
}
