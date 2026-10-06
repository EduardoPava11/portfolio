//! Colour analysis of the body of work in CIELAB, at full resolution.
//!
//! Every pixel of every photograph (no downsampling, no subsampling) is converted
//! sRGB -> linear -> XYZ (D65) -> CIELAB. Per photograph the program records the
//! L* a* b* centroid, the chroma statistics, the chroma weighted mean hue, and a
//! chroma weighted hue histogram. The body is then ORDERED BY COLOUR: photographs are
//! sorted by their chroma weighted hue angle, and the circle is cut at the largest
//! empty gap between neighbouring photographs so that no colour family is split
//! across the two ends of the sequence.
//!
//! Output:
//!   content/order.txt        the colour order; `build` and `export` follow it
//!   analysis/lab/index.html  contact sheet in colour order with swatches and numbers
//!   analysis/lab/lab.csv     the numbers
//!   analysis/lab/<stem>.jpg  small previews for the sheet

use crate::color::srgb_to_linear;
use crate::content::{self, Photo};
use crate::images;
use crate::site::esc;
use crate::Result;
use image::RgbImage;
use rayon::prelude::*;
use std::fs;
use std::path::Path;

const HUE_BINS: usize = 36;
/// Chroma histogram resolution: 1 C* unit per bin up to this value.
const C_BINS: usize = 160;

#[derive(Clone, Copy, Debug)]
pub struct Lab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

impl Lab {
    /// D65 reference white, 2 degree observer, sRGB primaries.
    pub fn from_srgb8(r: u8, g: u8, b: u8) -> Lab {
        let (r, g, b) = (srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b));
        let x = 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b;
        let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
        let z = 0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b;
        let (fx, fy, fz) = (f(x / 0.950_47), f(y), f(z / 1.088_83));
        Lab { l: 116.0 * fy - 16.0, a: 500.0 * (fx - fy), b: 200.0 * (fy - fz) }
    }
    /// Linear light sRGB, unclamped, so callers can see when a colour leaves the gamut.
    pub fn to_linear(self) -> [f32; 3] {
        let fy = (self.l + 16.0) / 116.0;
        let fx = fy + self.a / 500.0;
        let fz = fy - self.b / 200.0;
        let (x, y, z) = (finv(fx) * 0.950_47, finv(fy), finv(fz) * 1.088_83);
        [
            3.240_454_2 * x - 1.537_138_5 * y - 0.498_531_4 * z,
            -0.969_266_0 * x + 1.876_010_8 * y + 0.041_556_0 * z,
            0.055_643_4 * x - 0.204_025_9 * y + 1.057_225_2 * z,
        ]
    }

    pub fn in_gamut(self) -> bool {
        self.to_linear().iter().all(|v| (-1e-4..=1.0 + 1e-4).contains(v))
    }

    /// Hold L* and hue, bisect on chroma until the colour is displayable.
    pub fn gamut_mapped(self) -> Lab {
        if self.in_gamut() {
            return self;
        }
        let (h, mut lo, mut hi) = (self.hue().to_radians(), 0.0f32, self.chroma());
        for _ in 0..24 {
            let mid = 0.5 * (lo + hi);
            let cand = Lab { l: self.l, a: mid * h.cos(), b: mid * h.sin() };
            if cand.in_gamut() { lo = mid } else { hi = mid }
        }
        Lab { l: self.l, a: lo * h.cos(), b: lo * h.sin() }
    }

    pub fn hex(self) -> String {
        let [r, g, b] = self.to_srgb8();
        format!("#{r:02X}{g:02X}{b:02X}")
    }

    pub fn to_srgb8(self) -> [u8; 3] {
        let fy = (self.l + 16.0) / 116.0;
        let fx = fy + self.a / 500.0;
        let fz = fy - self.b / 200.0;
        let (x, y, z) = (finv(fx) * 0.950_47, finv(fy), finv(fz) * 1.088_83);
        let r = 3.240_454_2 * x - 1.537_138_5 * y - 0.498_531_4 * z;
        let g = -0.969_266_0 * x + 1.876_010_8 * y + 0.041_556_0 * z;
        let b = 0.055_643_4 * x - 0.204_025_9 * y + 1.057_225_2 * z;
        [crate::color::linear_to_srgb(r), crate::color::linear_to_srgb(g), crate::color::linear_to_srgb(b)]
    }
    pub fn chroma(self) -> f32 {
        (self.a * self.a + self.b * self.b).sqrt()
    }
    pub fn hue(self) -> f32 {
        self.b.atan2(self.a).to_degrees().rem_euclid(360.0)
    }
}

fn f(t: f32) -> f32 {
    const D: f32 = 6.0 / 29.0;
    if t > D * D * D { t.cbrt() } else { t / (3.0 * D * D) + 4.0 / 29.0 }
}
fn finv(t: f32) -> f32 {
    const D: f32 = 6.0 / 29.0;
    if t > D { t * t * t } else { 3.0 * D * D * (t - 4.0 / 29.0) }
}

pub struct Reading {
    pub stem: String,
    pub title: String,
    pub pixels: u64,
    pub width: u32,
    pub height: u32,
    /// Plain centroid.
    pub mean: Lab,
    pub median_l: f32,
    pub mean_c: f32,
    pub p90_c: f32,
    /// Chroma weighted mean hue: where the picture's colour actually is.
    pub hue: f32,
    /// Length of the chroma weighted mean vector divided by mean chroma: 1 when all the
    /// colour agrees on one hue, near 0 when hues cancel.
    pub hue_agreement: f32,
    pub hue_hist: Vec<f64>,
    /// The mat colour for this picture: its own hue at the set's register and intensity.
    pub mat: Option<Lab>,
}

/// How the mats sit against the pictures. Registers and intensities follow matte's
/// vocabulary and, like matte's, are taken from the set's own statistics, never fixed.
pub struct MatOptions {
    /// How the mat's hue relates to the picture's: own (same hue), opponent (180 deg
    /// away, the Hering opponent CIELAB is built on), split (150 deg, opposed but off
    /// the exact complement).
    pub hue: String,
    /// dark: 15th percentile of the pictures' median L*; mid: 50th; light: 85th;
    /// ink: halfway from the 5th percentile to black; paper: halfway from the 95th to white.
    pub register: String,
    /// Chroma at the CENTRE of the sequence. mute: half the pictures' mean chroma;
    /// balanced: the mean; statement: the 90th percentile.
    pub intensity: String,
    /// Chroma at the two ENDS of the sequence, same vocabulary.
    pub floor: String,
}

impl MatOptions {
    pub fn parse(args: &[String]) -> Result<MatOptions> {
        let mut o = MatOptions { hue: "own".into(), register: "mid".into(), intensity: "statement".into(), floor: "mute".into() };
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--hue" => {
                    o.hue = args.get(i + 1).ok_or("--hue needs own|opponent|split")?.clone();
                    i += 2;
                }
                "--register" => {
                    o.register = args.get(i + 1).ok_or("--register needs dark|mid|light")?.clone();
                    i += 2;
                }
                "--intensity" => {
                    o.intensity = args.get(i + 1).ok_or("--intensity needs mute|balanced|statement")?.clone();
                    i += 2;
                }
                "--floor" => {
                    o.floor = args.get(i + 1).ok_or("--floor needs mute|balanced|statement")?.clone();
                    i += 2;
                }
                other => return Err(format!("unknown option {other}").into()),
            }
        }
        if !["ink", "dark", "mid", "light", "paper"].contains(&o.register.as_str()) {
            return Err(format!("--register must be ink, dark, mid, light or paper, not {}", o.register).into());
        }
        if !["own", "opponent", "split"].contains(&o.hue.as_str()) {
            return Err(format!("--hue must be own, opponent or split, not {}", o.hue).into());
        }
        for (flag, v) in [("--intensity", &o.intensity), ("--floor", &o.floor)] {
            if !["mute", "balanced", "statement"].contains(&v.as_str()) {
                return Err(format!("{flag} must be mute, balanced or statement, not {v}").into());
            }
        }
        Ok(o)
    }
}

fn sorted_percentile(values: &mut Vec<f32>, q: f32) -> f32 {
    values.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let i = ((values.len() as f32 - 1.0) * q).round() as usize;
    values[i.min(values.len() - 1)]
}

/// The mats as a sequence, not a set of singles. L* is one value for the whole set so
/// the surround never jumps between frames. Hue is each picture's own. Chroma follows
/// a raised cosine envelope over sequence position: the floor at the first and last
/// picture, the full intensity at the centre, so the body opens and closes quietly and
/// peaks in the middle, the way a sequence is paced. Returns (L*, C* floor, C* peak).
pub fn derive_mats(rs: &mut [Reading], o: &MatOptions) -> (f32, f32, f32) {
    let mut ls: Vec<f32> = rs.iter().map(|r| r.median_l).collect();
    let mut cs: Vec<f32> = rs.iter().map(|r| r.mean_c).collect();
    let l = match o.register.as_str() {
        "ink" => 0.5 * sorted_percentile(&mut ls, 0.05),
        "dark" => sorted_percentile(&mut ls, 0.15),
        "light" => sorted_percentile(&mut ls, 0.85),
        "paper" => 0.5 * (sorted_percentile(&mut ls, 0.95) + 100.0),
        _ => sorted_percentile(&mut ls, 0.50),
    };
    let turn: f32 = match o.hue.as_str() {
        "opponent" => 180.0,
        "split" => 150.0,
        _ => 0.0,
    };
    let mean_c = cs.iter().sum::<f32>() / cs.len().max(1) as f32;
    let p90_c = sorted_percentile(&mut cs, 0.90);
    let level = |name: &str| match name {
        "balanced" => mean_c,
        "statement" => p90_c,
        _ => 0.5 * mean_c,
    };
    let (floor, peak) = (level(&o.floor), level(&o.intensity));
    let n = rs.len();
    for (i, r) in rs.iter_mut().enumerate() {
        let t = if n > 1 { i as f32 / (n - 1) as f32 } else { 0.5 };
        let envelope = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * t).cos();
        let c = floor + (peak - floor) * envelope;
        let h = (r.hue + turn).to_radians();
        r.mat = Some(Lab { l, a: c * h.cos(), b: c * h.sin() }.gamut_mapped());
    }
    (l, floor, peak)
}

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let opts = MatOptions::parse(args)?;
    let content = content::load_originals(root)?;
    let out = root.join("analysis/lab");
    fs::create_dir_all(&out)?;
    if content.photos.is_empty() {
        return Err("no photographs to read".into());
    }
    let started = std::time::Instant::now();
    let mut readings: Vec<Reading> = content
        .photos
        .par_iter()
        .map(|p| read(p, &out))
        .collect::<Result<Vec<_>>>()?;
    let pixels: u64 = readings.iter().map(|r| r.pixels).sum();
    println!("lab: {} photographs, {} pixels read in CIELAB in {:.1} s", readings.len(), pixels, started.elapsed().as_secs_f32());

    order_by_colour(&mut readings);
    let (mat_l, mat_floor, mat_peak) = derive_mats(&mut readings, &opts);
    println!(
        "lab: mats in the {} hue at L* {mat_l:.1} ({} register); chroma {mat_floor:.1} ({}) at the ends rising to {mat_peak:.1} ({}) at the centre",
        opts.hue, opts.register, opts.floor, opts.intensity
    );
    fs::write(
        out.join("mat-law.txt"),
        format!("--hue {} --register {} --intensity {} --floor {}\n", opts.hue, opts.register, opts.intensity, opts.floor),
    )?;

    let mut order = String::from("# Colour order written by `portfolio lab`. One file stem per line.\n# Edit by hand if you like; `build` and `export` follow this order.\n");
    for r in &readings {
        order.push_str(&r.stem);
        order.push('\n');
    }
    fs::write(root.join("content/order.txt"), order)?;
    fs::write(out.join("lab.csv"), csv(&readings))?;
    fs::write(out.join("index.html"), report(&readings, &content.site.name))?;

    for (i, r) in readings.iter().enumerate() {
        println!(
            "  {:>2} {:<32} hue {:5.1}  C {:5.1}  L {:5.1}  agree {:.2}  {}  mat {}",
            i + 1,
            r.stem,
            r.hue,
            r.mean_c,
            r.mean.l,
            r.hue_agreement,
            r.mean.hex(),
            r.mat.map(|m| m.hex()).unwrap_or_default()
        );
    }
    println!("lab: order -> content/order.txt, sheet -> {}", out.join("index.html").display());
    Ok(())
}

pub fn read(photo: &Photo, out: &Path) -> Result<Reading> {
    let img = images::open_upright(photo)?;
    let rgb: RgbImage = img.to_rgb8();
    let (w, h) = rgb.dimensions();

    // Full resolution, every pixel. Rows in parallel, each row folds into its own
    // accumulator, then the accumulators are summed.
    let acc = rgb
        .as_raw()
        .par_chunks_exact(w as usize * 3)
        .map(|row| {
            let mut a = Acc::new();
            for px in row.chunks_exact(3) {
                a.add(Lab::from_srgb8(px[0], px[1], px[2]));
            }
            a
        })
        .reduce(Acc::new, Acc::merge);

    // Small preview for the contact sheet.
    let preview = images::fit(&img, 480);
    images::write_jpeg(&preview, &out.join(format!("{}.jpg", photo.stem)))?;

    Ok(acc.finish(photo, w, h))
}

struct Acc {
    n: u64,
    sum_l: f64,
    sum_a: f64,
    sum_b: f64,
    sum_c: f64,
    /// Chroma weighted a and b, for the dominant hue.
    sum_ca: f64,
    sum_cb: f64,
    l_hist: Vec<u64>,
    c_hist: Vec<u64>,
    hue_hist: Vec<f64>,
}

impl Acc {
    fn new() -> Acc {
        Acc {
            n: 0,
            sum_l: 0.0,
            sum_a: 0.0,
            sum_b: 0.0,
            sum_c: 0.0,
            sum_ca: 0.0,
            sum_cb: 0.0,
            l_hist: vec![0; 101],
            c_hist: vec![0; C_BINS],
            hue_hist: vec![0.0; HUE_BINS],
        }
    }
    #[inline]
    fn add(&mut self, p: Lab) {
        let c = p.chroma();
        self.n += 1;
        self.sum_l += p.l as f64;
        self.sum_a += p.a as f64;
        self.sum_b += p.b as f64;
        self.sum_c += c as f64;
        self.sum_ca += (p.a * c) as f64;
        self.sum_cb += (p.b * c) as f64;
        self.l_hist[(p.l.clamp(0.0, 100.0) as usize).min(100)] += 1;
        self.c_hist[(c as usize).min(C_BINS - 1)] += 1;
        if c > 0.0 {
            let bin = ((p.hue() / 360.0 * HUE_BINS as f32) as usize).min(HUE_BINS - 1);
            self.hue_hist[bin] += c as f64;
        }
    }
    fn merge(mut self, o: Acc) -> Acc {
        self.n += o.n;
        self.sum_l += o.sum_l;
        self.sum_a += o.sum_a;
        self.sum_b += o.sum_b;
        self.sum_c += o.sum_c;
        self.sum_ca += o.sum_ca;
        self.sum_cb += o.sum_cb;
        for (x, y) in self.l_hist.iter_mut().zip(&o.l_hist) {
            *x += y;
        }
        for (x, y) in self.c_hist.iter_mut().zip(&o.c_hist) {
            *x += y;
        }
        for (x, y) in self.hue_hist.iter_mut().zip(&o.hue_hist) {
            *x += y;
        }
        self
    }
    fn finish(self, photo: &Photo, w: u32, h: u32) -> Reading {
        let n = self.n.max(1) as f64;
        let mean = Lab { l: (self.sum_l / n) as f32, a: (self.sum_a / n) as f32, b: (self.sum_b / n) as f32 };
        let mean_c = (self.sum_c / n) as f32;
        let (wa, wb) = (self.sum_ca / self.sum_c.max(1e-9), self.sum_cb / self.sum_c.max(1e-9));
        let hue = (wb.atan2(wa).to_degrees().rem_euclid(360.0)) as f32;
        let agreement = if mean_c > 0.0 { ((wa * wa + wb * wb).sqrt() / mean_c as f64) as f32 } else { 0.0 };
        Reading {
            stem: photo.stem.clone(),
            title: photo.title(),
            pixels: self.n,
            width: w,
            height: h,
            mean,
            median_l: percentile(&self.l_hist, 0.5),
            mean_c,
            p90_c: percentile(&self.c_hist, 0.9),
            hue,
            hue_agreement: agreement.min(1.0),
            hue_hist: self.hue_hist,
            mat: None,
        }
    }
}

fn percentile(hist: &[u64], q: f64) -> f32 {
    let total: u64 = hist.iter().sum();
    let target = (total as f64 * q) as u64;
    let mut seen = 0u64;
    for (i, &v) in hist.iter().enumerate() {
        seen += v;
        if seen >= target {
            return i as f32;
        }
    }
    (hist.len() - 1) as f32
}

/// Sort by hue; cut the circle at the widest empty gap so the sequence starts
/// just after the gap and no colour family is torn in two. A photograph whose
/// file is named `end` (with or without a number prefix) is pinned as the last
/// picture: that is the author's closing frame, not the colour's.
pub fn order_by_colour(rs: &mut Vec<Reading>) {
    let mut tail = Vec::new();
    let mut i = 0;
    while i < rs.len() {
        if crate::rename::strip_prefix(&rs[i].stem) == "end" {
            tail.push(rs.remove(i));
        } else {
            i += 1;
        }
    }
    sort_hue_cut_gap(rs);
    rs.extend(tail);
}

fn sort_hue_cut_gap(rs: &mut [Reading]) {
    rs.sort_by(|a, b| a.hue.partial_cmp(&b.hue).unwrap());
    if rs.len() < 2 {
        return;
    }
    let mut best_gap = -1.0f32;
    let mut start = 0usize;
    for i in 0..rs.len() {
        let next = (i + 1) % rs.len();
        let gap = (rs[next].hue - rs[i].hue).rem_euclid(360.0);
        if gap > best_gap {
            best_gap = gap;
            start = next;
        }
    }
    rs.rotate_left(start);
}

fn csv(rs: &[Reading]) -> String {
    let mut s = String::from("order,file,width,height,pixels,mean_L,mean_a,mean_b,median_L,mean_C,p90_C,hue_deg,hue_agreement,centroid_hex,mat_hex\n");
    for (i, r) in rs.iter().enumerate() {
        s.push_str(&format!(
            "{},{},{},{},{},{:.2},{:.2},{:.2},{:.0},{:.2},{:.0},{:.1},{:.3},{},{}\n",
            i + 1,
            r.stem,
            r.width,
            r.height,
            r.pixels,
            r.mean.l,
            r.mean.a,
            r.mean.b,
            r.median_l,
            r.mean_c,
            r.p90_c,
            r.hue,
            r.hue_agreement,
            r.mean.hex(),
            r.mat.map(|m| m.hex()).unwrap_or_default()
        ));
    }
    s
}

fn hue_svg(hist: &[f64]) -> String {
    let max = hist.iter().cloned().fold(0.0, f64::max).max(1e-9);
    let (w, h) = (360.0, 60.0);
    let bw = w / HUE_BINS as f64;
    let mut s = format!("<svg viewBox=\"0 0 {w} {h}\" class=\"hist\" role=\"img\" aria-label=\"hue histogram\">");
    for (i, v) in hist.iter().enumerate() {
        let bh = v / max * h;
        let hue = ((i as f32 + 0.5) * 360.0 / HUE_BINS as f32).to_radians();
        let [r, g, b] = Lab { l: 62.0, a: 45.0 * hue.cos(), b: 45.0 * hue.sin() }.to_srgb8();
        s.push_str(&format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"rgb({r},{g},{b})\"/>",
            i as f64 * bw,
            h - bh,
            bw - 0.5,
            bh
        ));
    }
    s.push_str("</svg>");
    s
}

fn report(rs: &[Reading], name: &str) -> String {
    let mut body_hist = vec![0f64; HUE_BINS];
    for r in rs {
        for (x, y) in body_hist.iter_mut().zip(&r.hue_hist) {
            *x += y;
        }
    }
    let mut h = format!(
        r##"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>CIELAB reading</title>
<style>
body {{ margin: 0; padding: 24px 16px; background: #141311; color: #ece7df; font: 15px/1.5 Georgia, serif; }}
h1, h2 {{ font-weight: normal; }}
h1 {{ font-size: 1.4rem; margin: 0 0 0.2rem; }}
.sub {{ color: #a39c91; margin: 0 0 1.5rem; max-width: 70ch; }}
.strip {{ display: flex; height: 28px; margin: 0 0 1.5rem; max-width: 1100px; }}
.strip a {{ flex: 1; display: block; }}
.body {{ max-width: 1100px; margin-bottom: 2rem; padding-bottom: 1.5rem; border-bottom: 1px solid #2d2a26; }}
.card {{ display: grid; grid-template-columns: 56px minmax(220px, 1fr) 2fr; gap: 20px; align-items: start; max-width: 1100px; padding: 1.2rem 0; border-bottom: 1px solid #2d2a26; }}
.card img {{ width: 100%; height: auto; display: block; }}
.card h2 {{ font-size: 1.05rem; margin: 0; }}
.num {{ font-size: 1.6rem; color: #a39c91; line-height: 1; }}
.swatch {{ width: 56px; height: 56px; margin-top: 8px; border: 1px solid #2d2a26; }}
table.n {{ border-collapse: collapse; margin-top: 0.6rem; font-variant-numeric: tabular-nums; }}
table.n td {{ padding: 2px 10px 2px 0; }}
table.n td:first-child {{ color: #a39c91; }}
svg.hist {{ width: 100%; max-width: 360px; height: auto; display: block; margin-top: 0.6rem; }}
@media (max-width: 760px) {{ .card {{ grid-template-columns: 56px 1fr; }} .card > div:last-child {{ grid-column: 1 / -1; }} }}
</style></head><body>
<h1>CIELAB reading: {name}</h1>
<p class="sub">Every pixel of every photograph at full resolution, sRGB to CIELAB (D65). The body is ordered by chroma weighted hue, with the circle cut at the widest gap between neighbours. The strip below is the sequence as centroids; click a swatch to jump.</p>
<div class="strip">
"##,
        name = esc(name)
    );
    for (i, r) in rs.iter().enumerate() {
        let [sr, sg, sb] = r.mean.to_srgb8();
        h.push_str(&format!("<a href=\"#p{}\" style=\"background:rgb({sr},{sg},{sb})\" title=\"{}\"></a>", i + 1, esc(&r.stem)));
    }
    h.push_str(&format!("</div>\n<div class=\"body\"><h2>The whole body: {} photographs</h2>{}</div>\n", rs.len(), hue_svg(&body_hist)));

    for (i, r) in rs.iter().enumerate() {
        let [sr, sg, sb] = r.mean.to_srgb8();
        h.push_str(&format!(
            r#"<div class="card" id="p{n}">
  <div><div class="num">{n}</div><div class="swatch" style="background:rgb({sr},{sg},{sb})" title="centroid"></div><div class="swatch" style="background:{mat}" title="mat {mat}"></div></div>
  <img src="{stem}.jpg" alt="{title}">
  <div>
    <h2>{title}</h2>
    <table class="n">
      <tr><td>centroid L* a* b*</td><td>{l:.1} / {a:.1} / {b:.1} (#{sr:02X}{sg:02X}{sb:02X})</td></tr>
      <tr><td>median L*</td><td>{ml:.0}</td></tr>
      <tr><td>chroma mean / p90</td><td>{c:.1} / {c90:.0}</td></tr>
      <tr><td>hue (chroma weighted)</td><td>{hue:.1} deg, agreement {agree:.2}</td></tr>
      <tr><td>mat</td><td>{mat}</td></tr>
      <tr><td>size</td><td>{w} x {hh}</td></tr>
    </table>
    {hist}
  </div>
</div>
"#,
            n = i + 1,
            stem = esc(&r.stem),
            title = esc(&r.title),
            mat = r.mat.map(|m| m.hex()).unwrap_or_default(),
            l = r.mean.l,
            a = r.mean.a,
            b = r.mean.b,
            ml = r.median_l,
            c = r.mean_c,
            c90 = r.p90_c,
            hue = r.hue,
            agree = r.hue_agreement,
            w = r.width,
            hh = r.height,
            hist = hue_svg(&r.hue_hist),
        ));
    }
    h.push_str("</body></html>\n");
    h
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_and_black() {
        let w = Lab::from_srgb8(255, 255, 255);
        assert!((w.l - 100.0).abs() < 0.05, "{w:?}");
        assert!(w.chroma() < 0.05);
        let k = Lab::from_srgb8(0, 0, 0);
        assert!(k.l.abs() < 1e-3);
    }

    #[test]
    fn srgb_red_matches_the_textbook() {
        // sRGB red in CIELAB D65 is about L 53.2, a 80.1, b 67.2.
        let r = Lab::from_srgb8(255, 0, 0);
        assert!((r.l - 53.2).abs() < 0.3, "{r:?}");
        assert!((r.a - 80.1).abs() < 0.5, "{r:?}");
        assert!((r.b - 67.2).abs() < 0.5, "{r:?}");
    }

    #[test]
    fn round_trip() {
        for &(r, g, b) in &[(255u8, 0u8, 0u8), (10, 200, 30), (128, 128, 128), (0, 0, 255), (240, 120, 20), (3, 7, 250)] {
            let back = Lab::from_srgb8(r, g, b).to_srgb8();
            assert!((back[0] as i32 - r as i32).abs() <= 1, "{r} {g} {b} -> {back:?}");
            assert!((back[1] as i32 - g as i32).abs() <= 1);
            assert!((back[2] as i32 - b as i32).abs() <= 1);
        }
    }

    fn reading(stem: &str, hue: f32) -> Reading {
        Reading {
            stem: stem.into(),
            title: stem.into(),
            pixels: 1,
            width: 1,
            height: 1,
            mean: Lab { l: 50.0, a: 0.0, b: 0.0 },
            median_l: 50.0,
            mean_c: 10.0,
            p90_c: 10.0,
            hue,
            hue_agreement: 1.0,
            hue_hist: vec![0.0; HUE_BINS],
            mat: None,
        }
    }

    #[test]
    fn colour_order_cuts_the_widest_gap() {
        // Hues 350, 10, 30 form one family; 200 sits alone. Widest gap is 30 -> 200,
        // so the sequence must start at 200 and run 200, 350, 10, 30.
        let mut rs = vec![reading("a", 10.0), reading("b", 200.0), reading("c", 350.0), reading("d", 30.0)];
        order_by_colour(&mut rs);
        let stems: Vec<&str> = rs.iter().map(|r| r.stem.as_str()).collect();
        assert_eq!(stems, ["b", "c", "a", "d"]);
    }

    #[test]
    fn mat_chroma_peaks_at_the_centre_and_rests_at_the_ends() {
        let mut rs: Vec<Reading> = (0..9).map(|i| reading(&format!("p{i}"), 40.0)).collect();
        for (i, r) in rs.iter_mut().enumerate() {
            r.mean_c = 10.0 + i as f32;
            r.median_l = 40.0;
        }
        let o = MatOptions { hue: "own".into(), register: "mid".into(), intensity: "statement".into(), floor: "mute".into() };
        let (_, floor, peak) = derive_mats(&mut rs, &o);
        let c: Vec<f32> = rs.iter().map(|r| r.mat.unwrap().chroma()).collect();
        assert!((c[0] - floor).abs() < 0.05 && (c[8] - floor).abs() < 0.05, "{c:?}");
        assert!((c[4] - peak).abs() < 0.05, "{c:?}");
        assert!(c[2] > c[0] && c[2] < c[4]);
    }

    #[test]
    fn end_is_pinned_last_whatever_its_hue() {
        let mut rs = vec![reading("a", 10.0), reading("33-end", 200.0), reading("c", 350.0)];
        order_by_colour(&mut rs);
        let stems: Vec<&str> = rs.iter().map(|r| r.stem.as_str()).collect();
        assert_eq!(stems, ["c", "a", "33-end"]);
    }
}
