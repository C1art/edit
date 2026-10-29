use timeline::FieldOrder;

/// Rebuilds the rows of the second field of tightly packed RGBA `pixels` from the
/// first field's rows around them, so the picture shows one moment instead of two.
/// Each missing pixel is the mean of the pair above and below that differ least
/// along a slight diagonal, which keeps slanted edges from turning to stairs.
pub(crate) fn deinterlace(pixels: &mut [u8], width: usize, height: usize, order: FieldOrder) {
    let kept_parity = match order {
        FieldOrder::Progressive => return,
        FieldOrder::UpperFirst => 0,
        FieldOrder::LowerFirst => 1,
    };
    if width == 0 || height < 2 {
        return;
    }
    let stride = width * 4;
    let mut rebuilt = vec![0u8; stride];
    for y in (0..height).filter(|y| y % 2 != kept_parity) {
        let above = y.checked_sub(1).unwrap_or(y + 1);
        let below = if y + 1 < height { y + 1 } else { above };
        let row = |index: usize| &pixels[index * stride..][..stride];
        let (above, below) = (row(above), row(below));
        let at = |row: &[u8], x: usize| -> [u8; 4] {
            let start = x * 4;
            [row[start], row[start + 1], row[start + 2], row[start + 3]]
        };
        for (x, out) in rebuilt.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let pair = |slant: isize| {
                let shifted = |x: usize, by: isize| x.saturating_add_signed(by).min(width - 1);
                (at(above, shifted(x, slant)), at(below, shifted(x, -slant)))
            };
            let difference = |(up, down): ([u8; 4], [u8; 4])| -> u32 {
                up[..3]
                    .iter()
                    .zip(&down[..3])
                    .map(|(&a, &b)| u32::from(a.abs_diff(b)))
                    .sum()
            };
            // Straight down wins ties, so flat areas aren't shifted sideways.
            let (up, down) = [0, -1, 1]
                .map(pair)
                .into_iter()
                .min_by_key(|&pair| difference(pair))
                .unwrap_or_else(|| pair(0));
            *out = std::array::from_fn(|channel| {
                (u16::from(up[channel]) + u16::from(down[channel])).div_ceil(2) as u8
            });
        }
        pixels[y * stride..][..stride].copy_from_slice(&rebuilt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Grey levels by row, four pixels wide.
    fn rows(levels: &[u8]) -> Vec<u8> {
        levels
            .iter()
            .flat_map(|&level| [[level, level, level, 255]; 4])
            .flatten()
            .collect()
    }

    #[test]
    fn the_second_field_is_rebuilt_from_the_first() {
        let combed = rows(&[10, 200, 30, 200, 50, 200]);
        let mut upper = combed.clone();
        deinterlace(&mut upper, 4, 6, FieldOrder::UpperFirst);
        assert_eq!(upper, rows(&[10, 20, 30, 40, 50, 50]));

        let mut lower = combed.clone();
        deinterlace(&mut lower, 4, 6, FieldOrder::LowerFirst);
        assert_eq!(lower, rows(&[200; 6]));

        let mut progressive = combed.clone();
        deinterlace(&mut progressive, 4, 6, FieldOrder::Progressive);
        assert_eq!(progressive, combed);
    }

    #[test]
    fn a_slanted_edge_is_followed_instead_of_blurred() {
        // A dark area whose edge moves one pixel right every row; the middle row
        // belongs to the dropped field.
        let edge = |dark: usize| (0..4).map(move |x| if x < dark { 0 } else { 255 });
        let picture: Vec<u8> = [1, 9, 3]
            .into_iter()
            .flat_map(|dark| edge(dark).flat_map(|level| [level, level, level, 255]))
            .collect();
        let mut rebuilt = picture.clone();
        deinterlace(&mut rebuilt, 4, 3, FieldOrder::UpperFirst);
        let middle: Vec<u8> = rebuilt[16..32].iter().step_by(4).copied().collect();
        assert_eq!(middle, [0, 0, 255, 255]);
    }
}
