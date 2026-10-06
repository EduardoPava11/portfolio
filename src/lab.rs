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
}

pub fn run(root: &Path) -> Result<()> {
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

    let mut order = String::from("# Colour order written by `portfolio lab`. One file stem per line.\n# Edit by hand if you like; `build` and `export` follow this order.\n");
    for r in &readings {
        order.push_str(&r.stem);
        order.push('\n');
    }
    fs::write(root.join("content/order.txt"), order)?;
    fs::write(out.join("lab.csv"), csv(&readings))?;
    fs::write(out.join("index.html"), report(&readings, &content.site.name))?;

    for (i, r) in readings.iter().enumerate() {
        let [sr, sg, sb] = r.mean.to_srgb8();
        println!(
            "  {:>2} {:<32} hue {:5.1}  C {:5.1}  L {:5.1}  agree {:.2}  #{sr:02X}{sg:02X}{sb:02X}",
            i + 1,
            r.stem,
            r.hue,
            r.mean_c,
            r.mean.l,
            r.hue_agreement
        );
    }
    println!("lab: order -> content/order.txt, sheet -> {}", out.join("index.html").display());
    Ok(())
}

fn read(photo: &Photo, out: &Path) -> Result<Reading> {
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
/// just after the gap and no colour family is torn in two.
fn order_by_colour(rs: &mut [Reading]) {
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
    let mut s = String::from("order,file,width,height,pixels,mean_L,mean_a,mean_b,median_L,mean_C,p90_C,hue_deg,hue_agreement,centroid_hex\n");
    for (i, r) in rs.iter().enumerate() {
        let [sr, sg, sb] = r.mean.to_srgb8();
        s.push_str(&format!(
            "{},{},{},{},{},{:.2},{:.2},{:.2},{:.0},{:.2},{:.0},{:.1},{:.3},#{sr:02X}{sg:02X}{sb:02X}\n",
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
            r.hue_agreement
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
  <div><div class="num">{n}</div><div class="swatch" style="background:rgb({sr},{sg},{sb})"></div></div>
  <img src="{stem}.jpg" alt="{title}">
  <div>
    <h2>{title}</h2>
    <table class="n">
      <tr><td>centroid L* a* b*</td><td>{l:.1} / {a:.1} / {b:.1} (#{sr:02X}{sg:02X}{sb:02X})</td></tr>
      <tr><td>median L*</td><td>{ml:.0}</td></tr>
      <tr><td>chroma mean / p90</td><td>{c:.1} / {c90:.0}</td></tr>
      <tr><td>hue (chroma weighted)</td><td>{hue:.1} deg, agreement {agree:.2}</td></tr>
      <tr><td>size</td><td>{w} x {hh}</td></tr>
    </table>
    {hist}
  </div>
</div>
"#,
            n = i + 1,
            stem = esc(&r.stem),
            title = esc(&r.title),
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
}
