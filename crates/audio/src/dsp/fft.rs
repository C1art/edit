use std::f32::consts::TAU;

/// An in-place radix-2 transform of a fixed power-of-two length, with its twiddles and
/// bit-reversed order worked out once.
#[derive(Debug, Clone)]
pub(super) struct Fft {
    twiddles: Vec<(f32, f32)>,
    reversed: Vec<usize>,
}

impl Fft {
    pub(super) fn new(length: usize) -> Self {
        debug_assert!(length.is_power_of_two());
        let bits = length.trailing_zeros();
        let twiddles = (0..length / 2)
            .map(|k| {
                let angle = -TAU * k as f32 / length as f32;
                (angle.cos(), angle.sin())
            })
            .collect();
        let reversed = (0..length)
            .map(|index| index.reverse_bits() >> (usize::BITS - bits))
            .collect();
        Self { twiddles, reversed }
    }

    /// Transforms `re` and `im` together; the inverse leaves out the division by the
    /// length.
    pub(super) fn run(&self, re: &mut [f32], im: &mut [f32], inverse: bool) {
        let length = re.len();
        for (index, &other) in self.reversed.iter().enumerate() {
            if index < other {
                re.swap(index, other);
                im.swap(index, other);
            }
        }
        let mut span = 2;
        while span <= length {
            let half = span / 2;
            let stride = length / span;
            for start in (0..length).step_by(span) {
                for k in 0..half {
                    let (cos, mut sin) = self.twiddles[k * stride];
                    if inverse {
                        sin = -sin;
                    }
                    let (a, b) = (start + k, start + k + half);
                    let t_re = re[b] * cos - im[b] * sin;
                    let t_im = re[b] * sin + im[b] * cos;
                    re[b] = re[a] - t_re;
                    im[b] = im[a] - t_im;
                    re[a] += t_re;
                    im[a] += t_im;
                }
            }
            span *= 2;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cosine_lands_in_its_bin_and_the_inverse_restores_it() {
        let length = 64;
        let signal: Vec<f32> = (0..length)
            .map(|n| (TAU * 5.0 * n as f32 / length as f32).cos())
            .collect();
        let (mut re, mut im) = (signal.clone(), vec![0.0; length]);
        let fft = Fft::new(length);
        fft.run(&mut re, &mut im, false);
        let magnitude = |k: usize| re[k].hypot(im[k]);
        assert!((magnitude(5) - 32.0).abs() < 1e-3);
        assert!((magnitude(59) - 32.0).abs() < 1e-3);
        assert!(magnitude(6) < 1e-3);
        fft.run(&mut re, &mut im, true);
        for (restored, original) in re.iter().zip(&signal) {
            assert!((restored / length as f32 - original).abs() < 1e-5);
        }
    }
}
