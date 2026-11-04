//! BlurHash encoding (https://blurha.sh): the blurry preview sent with our photos
//! (`info.blurhash`), which other apps (and AlwyChat) draw while the photo loads.

const DIGITS: &[u8] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~";

fn encode83(mut value: u32, length: usize, out: &mut String) {
    let mut chars = vec![b'0'; length];
    for c in chars.iter_mut().rev() {
        *c = DIGITS[(value % 83) as usize];
        value /= 83;
    }
    out.push_str(std::str::from_utf8(&chars).expect("ascii"));
}

fn to_linear(v: u8) -> f32 {
    let x = v as f32 / 255.0;
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

fn to_srgb(v: f32) -> u32 {
    let x = v.clamp(0.0, 1.0);
    let s = if x <= 0.003_130_8 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u32
}

fn sign_pow(v: f32, e: f32) -> f32 {
    v.signum() * v.abs().powf(e)
}

/// The BlurHash of an RGBA image (`rgba.len() == width * height * 4`), with `nx`×`ny` components
/// (1–9 each; 4×3 is the usual). Encode a small copy (32 px): the hash is just as good.
pub fn encode(rgba: &[u8], width: u32, height: u32, nx: u32, ny: u32) -> Option<String> {
    if !(1..=9).contains(&nx) || !(1..=9).contains(&ny) || width == 0 || height == 0 {
        return None;
    }
    if rgba.len() != (width * height * 4) as usize {
        return None;
    }
    let pi = std::f32::consts::PI;
    let mut factors = Vec::with_capacity((nx * ny) as usize);
    for j in 0..ny {
        for i in 0..nx {
            let norm = if i == 0 && j == 0 { 1.0 } else { 2.0 };
            let (mut r, mut g, mut b) = (0.0, 0.0, 0.0);
            for y in 0..height {
                for x in 0..width {
                    let basis = norm
                        * (pi * i as f32 * x as f32 / width as f32).cos()
                        * (pi * j as f32 * y as f32 / height as f32).cos();
                    let p = ((y * width + x) * 4) as usize;
                    r += basis * to_linear(rgba[p]);
                    g += basis * to_linear(rgba[p + 1]);
                    b += basis * to_linear(rgba[p + 2]);
                }
            }
            let scale = 1.0 / (width * height) as f32;
            factors.push([r * scale, g * scale, b * scale]);
        }
    }

    let mut hash = String::new();
    encode83((nx - 1) + (ny - 1) * 9, 1, &mut hash);
    let ac = &factors[1..];
    let max_value = if ac.is_empty() {
        encode83(0, 1, &mut hash);
        1.0
    } else {
        let actual = ac.iter().flatten().fold(0.0f32, |m, v| m.max(v.abs()));
        let quantised = ((actual * 166.0 - 0.5).floor() as i32).clamp(0, 82) as u32;
        encode83(quantised, 1, &mut hash);
        (quantised + 1) as f32 / 166.0
    };
    let dc = factors[0];
    encode83(
        (to_srgb(dc[0]) << 16) + (to_srgb(dc[1]) << 8) + to_srgb(dc[2]),
        4,
        &mut hash,
    );
    for c in ac {
        let q = |v: f32| {
            ((sign_pow(v / max_value, 0.5) * 9.0 + 9.5).floor() as i32).clamp(0, 18) as u32
        };
        encode83(q(c[0]) * 19 * 19 + q(c[1]) * 19 + q(c[2]), 2, &mut hash);
    }
    Some(hash)
}

/// The usual 4×3 BlurHash of a photo, from a 32 px copy.
pub fn of_image(image: &image::DynamicImage) -> Option<String> {
    let small = image.thumbnail(32, 32).to_rgba8();
    encode(small.as_raw(), small.width(), small.height(), 4, 3)
}

#[cfg(test)]
mod tests {
    use super::encode;

    #[test]
    fn solid_colour() {
        // 2×2 pure red: the average colour is red, and nothing varies.
        let red = [255u8, 0, 0, 255].repeat(4);
        let hash = encode(&red, 2, 2, 4, 3).unwrap();
        assert_eq!(hash.len(), 4 + 2 * 4 * 3);
        // Size flag (4×3 → 3 + 2×9 = 21 → "L"), then DC = 0xFF0000 in base 83.
        assert!(hash.starts_with('L'));
        let mut dc = String::new();
        super::encode83(0xFF0000, 4, &mut dc);
        assert_eq!(&hash[2..6], dc);
    }

    #[test]
    fn rejects_bad_input() {
        assert!(encode(&[0; 4], 1, 1, 0, 3).is_none());
        assert!(encode(&[0; 3], 1, 1, 4, 3).is_none());
    }

    #[test]
    fn gradient_for_the_ui_decoder() {
        // Left blue, right orange: printed for the cross-check with blurhash.ts.
        let (w, h) = (16u32, 8u32);
        let mut px = Vec::new();
        for _y in 0..h {
            for x in 0..w {
                if x < w / 2 {
                    px.extend([30, 80, 220, 255])
                } else {
                    px.extend([240, 140, 30, 255])
                }
            }
        }
        println!("HASH={}", encode(&px, w, h, 4, 3).unwrap());
    }
}
