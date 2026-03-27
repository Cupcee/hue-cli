/// Convert sRGB (0–255 each) to CIE 1931 xy chromaticity coordinates.
/// The Hue API uses xy + brightness instead of RGB.
pub fn rgb_to_xy(r: u8, g: u8, b: u8) -> (f64, f64) {
    // Normalize to [0, 1]
    let r = r as f64 / 255.0;
    let g = g as f64 / 255.0;
    let b = b as f64 / 255.0;

    // Apply sRGB gamma correction (linearize)
    let r = gamma(r);
    let g = gamma(g);
    let b = gamma(b);

    // Wide RGB D65 conversion matrix
    let x = r * 0.664511 + g * 0.154324 + b * 0.162028;
    let y = r * 0.283881 + g * 0.668433 + b * 0.047685;
    let z = r * 0.000088 + g * 0.072310 + b * 0.986039;

    let sum = x + y + z;
    if sum == 0.0 {
        return (0.0, 0.0);
    }

    (x / sum, y / sum)
}

fn gamma(val: f64) -> f64 {
    if val > 0.04045 {
        ((val + 0.055) / 1.055).powf(2.4)
    } else {
        val / 12.92
    }
}
