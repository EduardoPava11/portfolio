//! sRGB <-> OKLab. OKLab is used because its hue angle and chroma match how people
//! see colour far better than RGB or HSV do, so "red" can be a sector of hue angle
//! and "how red" can be chroma, independently of how bright the pixel is.
//! Reference: Bjorn Ottosson, "A perceptual color space for image processing" (2020).

/// 8 bit sRGB component to linear light.
pub fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Linear light to 8 bit sRGB, clamped.
pub fn linear_to_srgb(v: f32) -> u8 {
    let v = v.clamp(0.0, 1.0);
    let c = if v <= 0.003_130_8 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    };
    (c * 255.0).round() as u8
}

#[derive(Clone, Copy, Debug)]
pub struct Oklab {
    pub l: f32,
    pub a: f32,
    pub b: f32,
}

impl Oklab {
    pub fn from_linear(r: f32, g: f32, b: f32) -> Oklab {
        let l = 0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b;
        let m = 0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b;
        let s = 0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b;
        let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
        Oklab {
            l: 0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s,
            a: 1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s,
            b: 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s,
        }
    }

    pub fn from_srgb8(r: u8, g: u8, b: u8) -> Oklab {
        Oklab::from_linear(srgb_to_linear(r), srgb_to_linear(g), srgb_to_linear(b))
    }

    pub fn to_srgb8(self) -> [u8; 3] {
        let l = self.l + 0.396_337_777_4 * self.a + 0.215_803_757_3 * self.b;
        let m = self.l - 0.105_561_345_8 * self.a - 0.063_854_172_8 * self.b;
        let s = self.l - 0.089_484_177_5 * self.a - 1.291_485_548_0 * self.b;
        let (l, m, s) = (l * l * l, m * m * m, s * s * s);
        let r = 4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s;
        let g = -1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s;
        let b = -0.004_196_086_3 * l - 0.703_418_614_8 * m + 1.707_614_701_0 * s;
        [linear_to_srgb(r), linear_to_srgb(g), linear_to_srgb(b)]
    }

    /// Chroma: distance from the grey axis.
    pub fn chroma(self) -> f32 {
        (self.a * self.a + self.b * self.b).sqrt()
    }

    /// Hue angle in degrees, 0 to 360.
    pub fn hue(self) -> f32 {
        self.b.atan2(self.a).to_degrees().rem_euclid(360.0)
    }

    pub fn from_lch(l: f32, c: f32, h_deg: f32) -> Oklab {
        let h = h_deg.to_radians();
        Oklab { l, a: c * h.cos(), b: c * h.sin() }
    }
}

/// True when `h` lies in the sector from `lo` going counter clockwise to `hi`, both in degrees.
/// Handles sectors that cross 0 (for example 350 to 60).
pub fn hue_in_sector(h: f32, lo: f32, hi: f32) -> bool {
    let span = (hi - lo).rem_euclid(360.0);
    (h - lo).rem_euclid(360.0) <= span
}

/// The red sector derived from the display primaries rather than chosen by hand:
/// it runs from the hue halfway between sRGB magenta and red to the hue halfway
/// between red and yellow. Returns (lo, hi) in degrees.
pub fn derived_red_sector() -> (f32, f32) {
    let red = Oklab::from_srgb8(255, 0, 0).hue();
    let yellow = Oklab::from_srgb8(255, 255, 0).hue();
    let magenta = Oklab::from_srgb8(255, 0, 255).hue();
    let lo = midpoint_hue(magenta, red);
    let hi = midpoint_hue(red, yellow);
    (lo, hi)
}

fn midpoint_hue(a: f32, b: f32) -> f32 {
    let d = (b - a).rem_euclid(360.0);
    (a + d / 2.0).rem_euclid(360.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn white_is_l_one_and_neutral() {
        let w = Oklab::from_srgb8(255, 255, 255);
        assert!((w.l - 1.0).abs() < 1e-3);
        assert!(w.chroma() < 1e-3);
    }

    #[test]
    fn round_trip_srgb() {
        for &(r, g, b) in &[(255u8, 0u8, 0u8), (10, 200, 30), (128, 128, 128), (0, 0, 255), (240, 120, 20)] {
            let back = Oklab::from_srgb8(r, g, b).to_srgb8();
            assert!((back[0] as i32 - r as i32).abs() <= 1, "{r} {g} {b} -> {back:?}");
            assert!((back[1] as i32 - g as i32).abs() <= 1);
            assert!((back[2] as i32 - b as i32).abs() <= 1);
        }
    }

    #[test]
    fn pure_red_sits_inside_the_derived_sector() {
        let (lo, hi) = derived_red_sector();
        assert!(hue_in_sector(Oklab::from_srgb8(255, 0, 0).hue(), lo, hi));
        assert!(!hue_in_sector(Oklab::from_srgb8(0, 255, 0).hue(), lo, hi));
        assert!(!hue_in_sector(Oklab::from_srgb8(0, 0, 255).hue(), lo, hi));
    }

    #[test]
    fn sector_crossing_zero() {
        assert!(hue_in_sector(5.0, 350.0, 60.0));
        assert!(hue_in_sector(355.0, 350.0, 60.0));
        assert!(!hue_in_sector(180.0, 350.0, 60.0));
    }
}
