const BLOCKS: [&str; 8] = ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
const MISSING: &str = "·";
const MID_BLOCK: &str = "▄";

pub fn render(data: &[f64]) -> String {
    render_with_max_width(data, data.len())
}

pub fn render_with_max_width(data: &[f64], max_width: usize) -> String {
    if data.is_empty() || max_width == 0 {
        return String::new();
    }

    let slice = if data.len() > max_width {
        &data[data.len() - max_width..]
    } else {
        data
    };

    let finite_values: Vec<f64> = slice.iter().copied().filter(|v| v.is_finite()).collect();
    if finite_values.is_empty() {
        return MISSING.repeat(slice.len());
    }

    let min = finite_values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = finite_values
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);

    slice
        .iter()
        .map(|value| render_point(*value, min, max))
        .collect::<Vec<_>>()
        .join("")
}

fn render_point(value: f64, min: f64, max: f64) -> &'static str {
    if !value.is_finite() {
        return MISSING;
    }

    if (max - min).abs() < f64::EPSILON {
        return MID_BLOCK;
    }

    let normalized = ((value - min) / (max - min)).clamp(0.0, 1.0);
    let index = (normalized * (BLOCKS.len() as f64 - 1.0)).round() as usize;
    BLOCKS[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_empty() {
        assert_eq!(render(&[]), "");
    }

    #[test]
    fn test_render_increasing_sequence() {
        assert_eq!(render(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]), "▁▂▃▄▅▆▇█");
    }

    #[test]
    fn test_render_decreasing_sequence() {
        assert_eq!(render(&[8.0, 7.0, 6.0, 5.0, 4.0, 3.0, 2.0, 1.0]), "█▇▆▅▄▃▂▁");
    }

    #[test]
    fn test_render_single_value() {
        assert_eq!(render(&[42.0]), "▄");
    }

    #[test]
    fn test_render_equal_values() {
        assert_eq!(render(&[5.0, 5.0, 5.0, 5.0, 5.0]), "▄▄▄▄▄");
    }

    #[test]
    fn test_render_with_nan() {
        assert_eq!(render(&[10.0, f64::NAN, 30.0]), "▁·█");
    }

    #[test]
    fn test_render_negative_values() {
        let output = render(&[-50.0, -25.0, 0.0, 25.0, 50.0]);
        assert_eq!(output.chars().count(), 5);
        assert!(output.starts_with('▁'));
        assert!(output.ends_with('█'));
    }

    #[test]
    fn test_render_with_width_limit_keeps_recent_points() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let output = render_with_max_width(&data, 3);
        assert_eq!(output.chars().count(), 3);
        assert!(output.starts_with('▁'));
        assert!(output.ends_with('█'));
    }
}
