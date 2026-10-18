//! Writes a small ICC profile for the tests (compositor/tests/gamma.sh): only calibration curves (a vcgt
//! table) that warm the screen, red to the top, green to 90 %, blue to 75 %.
//!
//!     iccwarm FILE

fn main() {
    let path = std::env::args().nth(1).expect("iccwarm FILE");
    let entries: u16 = 16;
    let mut tag = Vec::new();
    tag.extend_from_slice(b"vcgt");
    tag.extend_from_slice(&[0, 0, 0, 0]);
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
    let mut p = vec![0u8; 128];
    p[36..40].copy_from_slice(b"acsp");
    p.extend_from_slice(&1u32.to_be_bytes());
    p.extend_from_slice(b"vcgt");
    p.extend_from_slice(&(128u32 + 4 + 12).to_be_bytes());
    p.extend_from_slice(&(tag.len() as u32).to_be_bytes());
    p.extend_from_slice(&tag);
    let size = p.len() as u32;
    p[0..4].copy_from_slice(&size.to_be_bytes());
    std::fs::write(path, p).expect("written");
}
