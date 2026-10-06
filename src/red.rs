//! Red analysis of the body of work.
//!
//! Every photograph is reduced to a working size, converted pixel by pixel to OKLab,
//! and read in OKLCh terms: lightness L, chroma C (distance from grey) and hue h.
//! A pixel is "red" when its chroma clears the floor (so greys and near greys never
//! vote) and its hue falls in the red sector. The sector is derived from the sRGB
//! primaries unless given on the command line.
//!
//! Output in `analysis/red/`:
//!   index.html   contact sheet: every photograph with its red mask, hue wheel
//!                histogram and numbers, in viewing order, plus the whole body
//!   red.csv      the same numbers, one row per photograph
//!   <stem>.png   red mask: red pixels in colour, everything else as grey lightness

use crate::color::{self, hue_in_sector, Oklab};
use crate::content::{self, Photo};
use crate::images;
use crate::site::esc;
use crate::Result;
use rayon::prelude::*;
use std::fs;
use std::path::Path;

const HUE_BINS: usize = 36;

pub struct Options {
    pub chroma_floor: f32,
    pub hue_lo: f32,
    pub hue_hi: f32,
    pub side: u32,
}

impl Options {
    fn parse(args: &[String]) -> Result<Options> {
        let (lo, hi) = color::derived_red_sector();
        let mut o = Options { chroma_floor: 0.04, hue_lo: lo, hue_hi: hi, side: 1200 };
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--chroma-floor" => {
                    o.chroma_floor = args.get(i + 1).ok_or("--chroma-floor needs a value")?.parse()?;
                    i += 2;
                }
                "--hue" => {
                    o.hue_lo = args.get(i + 1).ok_or("--hue needs LO HI")?.parse()?;
                    o.hue_hi = args.get(i + 2).ok_or("--hue needs LO HI")?.parse()?;
                    i += 3;
                }
                "--side" => {
                    o.side = args.get(i + 1).ok_or("--side needs a value")?.parse()?;
                    i += 2;
                }
                other => return Err(format!("unknown option {other}").into()),
            }
        }
        Ok(o)
    }
}

/// What one photograph says about its reds.
pub struct Reds {
    pub stem: String,
    pub title: String,
    pub when: String,
    pub taken: String,
    pub pixels: u64,
    /// Pixels above the chroma floor, any hue.
    pub coloured: u64,
    /// Pixels above the floor and inside the red sector.
    pub red: u64,
    /// Mean OKLab of the red pixels (L, C, circular mean hue).
    pub mean_l: f32,
    pub mean_c: f32,
    pub mean_h: f32,
    /// Chroma weighted hue histogram of all coloured pixels, HUE_BINS bins over 360 degrees.
    pub hue_hist: Vec<f64>,
    /// Lightness distribution of red pixels in 10 bands of L.
    pub red_l_hist: Vec<u64>,
    /// Chroma distribution of red pixels in 10 bands of C from 0 to 0.3.
    pub red_c_hist: Vec<u64>,
}

impl Reds {
    pub fn red_fraction(&self) -> f64 {
        if self.pixels == 0 { 0.0 } else { self.red as f64 / self.pixels as f64 }
    }
    pub fn coloured_fraction(&self) -> f64 {
        if self.pixels == 0 { 0.0 } else { self.coloured as f64 / self.pixels as f64 }
    }
    /// Share of the coloured pixels that are red: how much of the picture's colour is red.
    pub fn red_share_of_colour(&self) -> f64 {
        if self.coloured == 0 { 0.0 } else { self.red as f64 / self.coloured as f64 }
    }
}

pub fn run(root: &Path, args: &[String]) -> Result<()> {
    let opts = Options::parse(args)?;
    let content = content::load_originals(root)?;
    let out = root.join("analysis/red");
    fs::create_dir_all(&out)?;
    if content.photos.is_empty() {
        println!("red: no photographs in content/photos/ yet; wrote an empty report to {}", out.display());
    }

    let results: Vec<Reds> = content
        .photos
        .par_iter()
        .map(|p| analyse(p, &opts, &out))
        .collect::<Result<Vec<_>>>()?;

    fs::write(out.join("red.csv"), csv(&results))?;
    fs::write(out.join("index.html"), report(&results, &opts, &content.site.name))?;

    println!(
        "red: {} photographs, sector {:.1} to {:.1} deg, chroma floor {:.3} -> {}",
        results.len(),
        opts.hue_lo,
        opts.hue_hi,
        opts.chroma_floor,
        out.display()
    );
    for r in &results {
        println!(
            "  {:<32} red {:5.1}%  of colour {:5.1}%  L {:.2}  C {:.3}  h {:5.1}",
            r.stem,
            100.0 * r.red_fraction(),
            100.0 * r.red_share_of_colour(),
            r.mean_l,
            r.mean_c,
            r.mean_h
        );
    }
    Ok(())
}

fn analyse(photo: &Photo, o: &Options, out: &Path) -> Result<Reds> {
    let img = images::fit(&images::open_upright(photo)?, o.side);
    let (w, h) = img.dimensions();
    let mut mask = image::RgbImage::new(w, h);

    let mut coloured = 0u64;
    let mut red = 0u64;
    let (mut sum_l, mut sum_c, mut sum_a, mut sum_b) = (0f64, 0f64, 0f64, 0f64);
    let mut hue_hist = vec![0f64; HUE_BINS];
    let mut red_l_hist = vec![0u64; 10];
    let mut red_c_hist = vec![0u64; 10];

    for (x, y, px) in img.enumerate_pixels() {
        let lab = Oklab::from_srgb8(px[0], px[1], px[2]);
        let c = lab.chroma();
        let is_coloured = c >= o.chroma_floor;
        let is_red = is_coloured && hue_in_sector(lab.hue(), o.hue_lo, o.hue_hi);
        if is_coloured {
            coloured += 1;
            let bin = ((lab.hue() / 360.0 * HUE_BINS as f32) as usize).min(HUE_BINS - 1);
            hue_hist[bin] += c as f64;
        }
        if is_red {
            red += 1;
            sum_l += lab.l as f64;
            sum_c += c as f64;
            // Circular mean: average the unit vectors, not the angles.
            sum_a += (lab.a / c) as f64;
            sum_b += (lab.b / c) as f64;
            red_l_hist[((lab.l * 10.0) as usize).min(9)] += 1;
            red_c_hist[((c / 0.3 * 10.0) as usize).min(9)] += 1;
            mask.put_pixel(x, y, *px);
        } else {
            // Grey of the same lightness: for a neutral, linear value = L cubed.
            let g = color::linear_to_srgb(lab.l * lab.l * lab.l);
            mask.put_pixel(x, y, image::Rgb([g, g, g]));
        }
    }

    mask.save(out.join(format!("{}.png", photo.stem)))?;

    let n = red.max(1) as f64;
    let mean_h = if red == 0 { 0.0 } else { (sum_b.atan2(sum_a).to_degrees().rem_euclid(360.0)) as f32 };
    Ok(Reds {
        stem: photo.stem.clone(),
        title: photo.title(),
        when: photo.when(),
        taken: photo.taken.clone().unwrap_or_default(),
        pixels: (w as u64) * (h as u64),
        coloured,
        red,
        mean_l: (sum_l / n) as f32,
        mean_c: (sum_c / n) as f32,
        mean_h,
        hue_hist,
        red_l_hist,
        red_c_hist,
    })
}

fn csv(rs: &[Reds]) -> String {
    let mut s = String::from("file,title,when,taken,pixels,coloured_pct,red_pct,red_share_of_colour_pct,red_mean_L,red_mean_C,red_mean_hue_deg\n");
    for r in rs {
        s.push_str(&format!(
            "{},\"{}\",{},{},{},{:.2},{:.2},{:.2},{:.4},{:.4},{:.1}\n",
            r.stem,
            r.title.replace('"', "\"\""),
            r.when,
            r.taken,
            r.pixels,
            100.0 * r.coloured_fraction(),
            100.0 * r.red_fraction(),
            100.0 * r.red_share_of_colour(),
            r.mean_l,
            r.mean_c,
            r.mean_h
        ));
    }
    s
}

/// A 36 bar histogram, each bar painted in the hue it counts, with the red sector shaded.
fn hue_svg(hist: &[f64], o: &Options) -> String {
    let max = hist.iter().cloned().fold(0.0, f64::max).max(1e-9);
    let (w, h, pad) = (360.0, 80.0, 4.0);
    let bw = w / HUE_BINS as f64;
    let mut s = format!("<svg viewBox=\"0 0 {w} {}\" class=\"hist\" role=\"img\" aria-label=\"hue histogram\">", h + pad * 2.0);
    // Red sector band along the bottom.
    for i in 0..HUE_BINS {
        let mid = (i as f32 + 0.5) * 360.0 / HUE_BINS as f32;
        if hue_in_sector(mid, o.hue_lo, o.hue_hi) {
            s.push_str(&format!("<rect x=\"{:.1}\" y=\"{}\" width=\"{bw:.1}\" height=\"{pad}\" fill=\"#a3321f\" opacity=\"0.5\"/>", i as f64 * bw, h + pad));
        }
    }
    for (i, v) in hist.iter().enumerate() {
        let bh = v / max * h;
        let [r, g, b] = Oklab::from_lch(0.65, 0.17, (i as f32 + 0.5) * 360.0 / HUE_BINS as f32).to_srgb8();
        s.push_str(&format!(
            "<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\" fill=\"rgb({r},{g},{b})\"/>",
            i as f64 * bw,
            pad + h - bh,
            bw - 0.5,
            bh
        ));
    }
    s.push_str("</svg>");
    s
}

fn bars_svg(hist: &[u64], label: &str) -> String {
    let max = *hist.iter().max().unwrap_or(&1).max(&1) as f64;
    let (w, h) = (120.0, 40.0);
    let bw = w / hist.len() as f64;
    let mut s = format!("<svg viewBox=\"0 0 {w} {h}\" class=\"bars\" role=\"img\" aria-label=\"{label}\">");
    for (i, v) in hist.iter().enumerate() {
        let bh = *v as f64 / max * h;
        s.push_str(&format!("<rect x=\"{:.1}\" y=\"{:.1}\" width=\"{:.1}\" height=\"{:.1}\"/>", i as f64 * bw, h - bh, bw - 1.0, bh));
    }
    s.push_str("</svg>");
    s
}

fn report(rs: &[Reds], o: &Options, name: &str) -> String {
    // The whole body: sum the histograms, pool the pixel counts.
    let mut body_hue = vec![0f64; HUE_BINS];
    let (mut pixels, mut coloured, mut red) = (0u64, 0u64, 0u64);
    for r in rs {
        for (a, b) in body_hue.iter_mut().zip(&r.hue_hist) {
            *a += b;
        }
        pixels += r.pixels;
        coloured += r.coloured;
        red += r.red;
    }
    let pct = |a: u64, b: u64| if b == 0 { 0.0 } else { 100.0 * a as f64 / b as f64 };

    let mut h = format!(
        r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>Red analysis</title>
<style>
body {{ margin: 0; padding: 24px 16px; background: #141311; color: #ece7df; font: 15px/1.5 Georgia, serif; }}
h1, h2 {{ font-weight: normal; }}
h1 {{ font-size: 1.4rem; margin: 0 0 0.2rem; }}
.sub {{ color: #a39c91; margin: 0 0 1.5rem; }}
.body {{ display: grid; grid-template-columns: 1fr 2fr; gap: 24px; align-items: end; max-width: 1100px; margin-bottom: 2rem; padding-bottom: 1.5rem; border-bottom: 1px solid #2d2a26; }}
.card {{ display: grid; grid-template-columns: minmax(200px, 1fr) minmax(200px, 1fr) 2fr; gap: 20px; align-items: start; max-width: 1100px; padding: 1.2rem 0; border-bottom: 1px solid #2d2a26; }}
.card img {{ width: 100%; height: auto; display: block; }}
.card h2 {{ font-size: 1.05rem; margin: 0; }}
.when {{ color: #a39c91; }}
table.n {{ border-collapse: collapse; margin-top: 0.6rem; font-variant-numeric: tabular-nums; }}
table.n td {{ padding: 2px 10px 2px 0; }}
table.n td:first-child {{ color: #a39c91; }}
svg.hist {{ width: 100%; max-width: 360px; height: auto; display: block; }}
svg.bars {{ width: 120px; height: 40px; }}
svg.bars rect {{ fill: #d9634c; }}
.mini {{ display: flex; gap: 20px; margin-top: 0.6rem; color: #a39c91; font-size: 0.85rem; }}
.mini div {{ display: grid; gap: 2px; }}
@media (max-width: 760px) {{ .card, .body {{ grid-template-columns: 1fr; }} }}
</style></head><body>
<h1>Red analysis: {name}</h1>
<p class="sub">OKLCh. Red sector {lo:.1} to {hi:.1} degrees; chroma floor {floor:.3}; analysed at {side} px on the long side. Masks keep red pixels in colour and show everything else as grey of the same lightness.</p>
<div class="body">
  <div>
    <h2>The whole body</h2>
    <table class="n">
      <tr><td>photographs</td><td>{count}</td></tr>
      <tr><td>coloured pixels</td><td>{col:.1}%</td></tr>
      <tr><td>red pixels</td><td>{redp:.1}%</td></tr>
      <tr><td>red share of colour</td><td>{share:.1}%</td></tr>
    </table>
  </div>
  <div>{body_hist}</div>
</div>
"#,
        name = esc(name),
        lo = o.hue_lo,
        hi = o.hue_hi,
        floor = o.chroma_floor,
        side = o.side,
        count = rs.len(),
        col = pct(coloured, pixels),
        redp = pct(red, pixels),
        share = pct(red, coloured),
        body_hist = hue_svg(&body_hue, o),
    );

    for r in rs {
        let swatch = if r.red > 0 {
            let [cr, cg, cb] = Oklab::from_lch(r.mean_l, r.mean_c, r.mean_h).to_srgb8();
            format!("<span style=\"display:inline-block;width:1em;height:1em;vertical-align:-0.15em;background:rgb({cr},{cg},{cb});margin-right:6px\"></span>")
        } else {
            String::new()
        };
        h.push_str(&format!(
            r#"<div class="card">
  <img src="../../docs/img/thumb/{stem}.jpg" alt="{title}">
  <img src="{stem}.png" alt="red mask of {title}">
  <div>
    <h2>{swatch}{title} <span class="when">{when}</span></h2>
    <table class="n">
      <tr><td>red pixels</td><td>{redp:.1}%</td></tr>
      <tr><td>coloured pixels</td><td>{col:.1}%</td></tr>
      <tr><td>red share of colour</td><td>{share:.1}%</td></tr>
      <tr><td>mean red L / C / h</td><td>{l:.2} / {c:.3} / {hue:.0} deg</td></tr>
    </table>
    {hist}
    <div class="mini">
      <div>{lbars}<span>red lightness, dark to light</span></div>
      <div>{cbars}<span>red chroma, 0 to 0.3</span></div>
    </div>
  </div>
</div>
"#,
            stem = esc(&r.stem),
            title = esc(&r.title),
            when = esc(&r.when),
            redp = 100.0 * r.red_fraction(),
            col = 100.0 * r.coloured_fraction(),
            share = 100.0 * r.red_share_of_colour(),
            l = r.mean_l,
            c = r.mean_c,
            hue = r.mean_h,
            hist = hue_svg(&r.hue_hist, o),
            lbars = bars_svg(&r.red_l_hist, "red lightness"),
            cbars = bars_svg(&r.red_c_hist, "red chroma"),
        ));
    }
    h.push_str("</body></html>\n");
    h
}
