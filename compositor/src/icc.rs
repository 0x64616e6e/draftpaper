//! ICC profiles' calibration curves (issue #50): the `vcgt` tag, which X11 tools (xcalib, colord) load into
//! a display's gamma table, as a table of samples or as a formula per channel. Read here and resampled to
//! the size of the display's table (gamma.rs). The rest of a profile (its matrix, its LUTs: full colour
//! management) is not used.

/// The three ramps (red, green, blue) for a gamma table of `size` entries, from a profile's bytes; None
/// when it is not an ICC profile or has no calibration curves.
pub fn ramps(bytes: &[u8], size: usize) -> Option<[Vec<u16>; 3]> {
    if size < 2 || bytes.len() < 132 || &bytes[36..40] != b"acsp" {
        return None;
    }
    let be32 = |at: usize| bytes.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    // the tag table: count, then (signature, offset, size) each
    let count = be32(128)? as usize;
    let (offset, len) = (0..count.min(1024)).find_map(|i| {
        let at = 132 + 12 * i;
        (bytes.get(at..at + 4)? == b"vcgt").then(|| Some((be32(at + 4)? as usize, be32(at + 8)? as usize)))?
    })?;
    let tag = bytes.get(offset..offset.checked_add(len)?)?;
    if tag.len() < 12 || &tag[0..4] != b"vcgt" {
        return None;
    }
    let tag_be32 = |at: usize| tag.get(at..at + 4).map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
    let tag_be16 = |at: usize| tag.get(at..at + 2).map(|b| u16::from_be_bytes([b[0], b[1]]));
    let resample = |f: &dyn Fn(usize, f64) -> f64| -> [Vec<u16>; 3] {
        std::array::from_fn(|ch| {
            (0..size)
                .map(|i| {
                    let x = i as f64 / (size - 1) as f64;
                    (f(ch, x).clamp(0.0, 1.0) * 65535.0).round() as u16
                })
                .collect()
        })
    };
    match tag_be32(8)? {
        // a table: channels, entries per channel, bytes per entry (1 or 2), then the samples, channel by channel
        0 => {
            let (channels, entries, width) = (tag_be16(12)? as usize, tag_be16(14)? as usize, tag_be16(16)? as usize);
            if !(channels == 1 || channels == 3) || entries < 2 || !(width == 1 || width == 2) || tag.len() < 18 + channels * entries * width {
                return None;
            }
            let sample = |ch: usize, k: usize| -> f64 {
                let at = 18 + (ch.min(channels - 1) * entries + k) * width;
                if width == 1 { tag[at] as f64 / 255.0 } else { u16::from_be_bytes([tag[at], tag[at + 1]]) as f64 / 65535.0 }
            };
            // linear between the samples
            Some(resample(&|ch, x| {
                let p = x * (entries - 1) as f64;
                let k = (p.floor() as usize).min(entries - 2);
                let t = p - k as f64;
                sample(ch, k) * (1.0 - t) + sample(ch, k + 1) * t
            }))
        }
        // a formula: per channel gamma, min and max (s15Fixed16): min + (max - min) x^gamma
        1 => {
            let fixed = |at: usize| tag_be32(at).map(|v| v as i32 as f64 / 65536.0);
            let mut f = [(1.0, 0.0, 1.0); 3];
            for (ch, item) in f.iter_mut().enumerate() {
                let at = 12 + ch * 12;
                *item = (fixed(at)?, fixed(at + 4)?, fixed(at + 8)?);
            }
            if f.iter().any(|(g, _, _)| *g <= 0.0) {
                return None;
            }
            Some(resample(&|ch, x| {
                let (g, lo, hi) = f[ch];
                lo + (hi - lo) * x.powf(g)
            }))
        }
        _ => None,
    }
}

/// The ramps that change nothing: a straight line from 0 to the top.
pub fn identity(size: usize) -> [Vec<u16>; 3] {
    let line: Vec<u16> = (0..size).map(|i| ((i as f64 / (size.max(2) - 1) as f64) * 65535.0).round() as u16).collect();
    [line.clone(), line.clone(), line]
}

#[cfg(test)]
pub mod tests {
    /// A profile with only a `vcgt` tag holding `tag` (its type and data, after "vcgt" and the reserved word).
    pub fn profile(tag: &[u8]) -> Vec<u8> {
        let mut p = vec![0u8; 128];
        p[36..40].copy_from_slice(b"acsp");
        p.extend_from_slice(&1u32.to_be_bytes());
        let offset = 128 + 4 + 12;
        let body = [b"vcgt".as_slice(), &[0, 0, 0, 0], tag].concat();
        p.extend_from_slice(b"vcgt");
        p.extend_from_slice(&(offset as u32).to_be_bytes());
        p.extend_from_slice(&(body.len() as u32).to_be_bytes());
        p.extend_from_slice(&body);
        let size = p.len() as u32;
        p[0..4].copy_from_slice(&size.to_be_bytes());
        p
    }

    /// A table profile warming the screen: red to the top, green to 90 %, blue to 75 %, `entries` samples.
    pub fn warm(entries: u16) -> Vec<u8> {
        let mut tag = Vec::new();
        tag.extend_from_slice(&0u32.to_be_bytes());
        tag.extend_from_slice(&3u16.to_be_bytes());
        tag.extend_from_slice(&entries.to_be_bytes());
        tag.extend_from_slice(&2u16.to_be_bytes());
        for top in [1.0, 0.9, 0.75] {
            for k in 0..entries {
                let v = (k as f64 / (entries - 1) as f64 * top * 65535.0).round() as u16;
                tag.extend_from_slice(&v.to_be_bytes());
            }
        }
        profile(&tag)
    }

    #[test]
    fn a_table_resampled() {
        let r = super::ramps(&warm(16), 256).unwrap();
        assert_eq!((r[0][0], r[0][255]), (0, 65535));
        assert_eq!(r[1][255], (0.9f64 * 65535.0).round() as u16);
        assert_eq!(r[2][255], (0.75f64 * 65535.0).round() as u16);
        // linear between the samples: halfway, half the top
        assert!((r[2][128] as i32 - (0.75 * 65535.0 * 128.0 / 255.0) as i32).abs() < 300, "{}", r[2][128]);
    }

    #[test]
    fn a_formula() {
        let mut tag = Vec::new();
        tag.extend_from_slice(&1u32.to_be_bytes());
        for (g, lo, hi) in [(1.0f64, 0.0f64, 1.0f64), (1.0, 0.0, 0.9), (2.2, 0.0, 0.8)] {
            for v in [g, lo, hi] {
                tag.extend_from_slice(&((v * 65536.0).round() as i32).to_be_bytes());
            }
        }
        let r = super::ramps(&profile(&tag), 1024).unwrap();
        assert_eq!(r[0][1023], 65535);
        // 0.9 as s15Fixed16 is 58982/65536: a hair under 0.9
        assert!((r[1][1023] as i32 - (0.9f64 * 65535.0).round() as i32).abs() <= 1, "{}", r[1][1023]);
        // blue: 0.8 x^2.2, at x = 0.5
        let x = 511.0 / 1023.0;
        assert!((r[2][511] as f64 - 0.8 * f64::powf(x, 2.2) * 65535.0).abs() < 2.0);
    }

    #[test]
    fn not_a_profile_or_no_curves() {
        assert!(super::ramps(b"not a profile", 256).is_none());
        let mut p = profile(&[0, 0, 0, 9]);
        assert!(super::ramps(&p, 256).is_none());
        p[36] = b'x';
        assert!(super::ramps(&p, 256).is_none());
        assert_eq!(super::identity(3), [vec![0, 32768, 65535], vec![0, 32768, 65535], vec![0, 32768, 65535]]);
    }
}
