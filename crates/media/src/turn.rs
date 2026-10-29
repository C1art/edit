use crate::spare_pixels;

/// How far clockwise a stream's pictures turn to stand upright. Phones record sideways
/// pictures with a display matrix saying so rather than turning the pixels themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Turn {
    #[default]
    None,
    Quarter,
    Half,
    ThreeQuarters,
}

impl Turn {
    /// The nearest quarter turn to `degrees`; matrices that aren't a turn, or skew,
    /// leave pictures as they are.
    pub(crate) fn clockwise(degrees: f64) -> Self {
        if !degrees.is_finite() {
            return Self::None;
        }
        match (degrees / 90.0).round().rem_euclid(4.0) as u8 {
            1 => Self::Quarter,
            2 => Self::Half,
            3 => Self::ThreeQuarters,
            _ => Self::None,
        }
    }

    /// `(width, height)` after the turn.
    pub(crate) fn sides<T>(self, (width, height): (T, T)) -> (T, T) {
        match self {
            Self::Quarter | Self::ThreeQuarters => (height, width),
            Self::None | Self::Half => (width, height),
        }
    }

    /// Turns a tightly packed RGBA picture, returning its new width and height.
    pub(crate) fn apply(self, width: u32, height: u32, pixels: Vec<u8>) -> (u32, u32, Vec<u8>) {
        if self == Self::None {
            return (width, height, pixels);
        }
        let (w, h) = (width as usize, height as usize);
        let mut turned = spare_pixels::take(pixels.len());
        let (turned_width, turned_height) = self.sides((width, height));
        let source = |x: usize, y: usize| {
            let at = (y * w + x) * 4;
            &pixels[at..at + 4]
        };
        for (index, out) in turned.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (x, y) = (index % turned_width as usize, index / turned_width as usize);
            let from = match self {
                Self::Quarter => source(y, h - 1 - x),
                Self::Half => source(w - 1 - x, h - 1 - y),
                Self::ThreeQuarters => source(w - 1 - y, x),
                Self::None => source(x, y),
            };
            out.copy_from_slice(from);
        }
        spare_pixels::give(pixels);
        (turned_width, turned_height, turned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 3 by 2 picture whose pixels are numbered row by row: `0 1 2 / 3 4 5`.
    fn numbered() -> Vec<u8> {
        (0..6).flat_map(|n| [n, 0, 0, 255]).collect()
    }

    fn numbers(pixels: &[u8]) -> Vec<u8> {
        pixels
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[0])
            .collect()
    }

    #[test]
    fn turns_move_each_pixel_to_its_place() {
        let cases = [
            (Turn::Quarter, (2, 3), vec![3, 0, 4, 1, 5, 2]),
            (Turn::Half, (3, 2), vec![5, 4, 3, 2, 1, 0]),
            (Turn::ThreeQuarters, (2, 3), vec![2, 5, 1, 4, 0, 3]),
        ];
        for (turn, size, expected) in cases {
            let (width, height, pixels) = turn.apply(3, 2, numbered());
            assert_eq!((width, height), size, "{turn:?}");
            assert_eq!(numbers(&pixels), expected, "{turn:?}");
        }
    }

    #[test]
    fn angles_round_to_the_nearest_quarter_turn() {
        assert_eq!(Turn::clockwise(-90.0), Turn::ThreeQuarters);
        assert_eq!(Turn::clockwise(89.6), Turn::Quarter);
        assert_eq!(Turn::clockwise(-180.0), Turn::Half);
        assert_eq!(Turn::clockwise(f64::NAN), Turn::None);
    }
}
