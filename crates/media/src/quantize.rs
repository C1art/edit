/// Colors are binned at five bits a channel, fine enough that neighbouring bins look
/// alike and coarse enough to histogram a whole frame cheaply.
const BITS: u32 = 5;
const BINS: usize = 1 << (3 * BITS);
const COLORS: usize = 256;
const UNMAPPED: u8 = u8::MAX;

/// Ordered dithering spreads the error over a 4 × 4 tile, which keeps still areas still
/// from frame to frame where error diffusion would make them crawl.
const BAYER: [[i16; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];
/// How far, in 8-bit levels, dithering nudges a pixel either way.
const DITHER_REACH: i16 = 6;

/// Picks up to 256 colors for a picture and maps each pixel to one of them.
pub(crate) struct Quantizer {
    counts: Vec<u32>,
    sums: Vec<[u64; 3]>,
    /// The palette index nearest each bin, filled as bins are met.
    nearest: Vec<(bool, u8)>,
    pub(crate) palette: Vec<[u8; 3]>,
}

impl Default for Quantizer {
    fn default() -> Self {
        Self {
            counts: vec![0; BINS],
            sums: vec![[0; 3]; BINS],
            nearest: vec![(false, UNMAPPED); BINS],
            palette: Vec::with_capacity(COLORS),
        }
    }
}

/// A run of bins whose colors are averaged into one palette entry.
struct Bucket {
    start: usize,
    end: usize,
    population: u64,
}

impl Quantizer {
    /// Fills `indices` with one palette index per pixel of `rgba`, `width` pixels a row,
    /// choosing [`Self::palette`] for this picture. Alpha is ignored.
    pub(crate) fn quantize(&mut self, rgba: &[u8], width: usize, indices: &mut [u8]) {
        self.choose_palette(rgba);
        self.nearest.fill((false, UNMAPPED));
        let (pixels, _) = rgba.as_chunks::<4>();
        for (at, (pixel, index)) in pixels.iter().zip(indices.iter_mut()).enumerate() {
            let (x, y) = (at % width.max(1), at / width.max(1));
            let nudge = (BAYER[y % 4][x % 4] * 2 - 15) * DITHER_REACH / 15;
            let channel = |value: u8| (i16::from(value) + nudge).clamp(0, 255) as u8;
            *index = self.nearest_to(bin([
                channel(pixel[0]),
                channel(pixel[1]),
                channel(pixel[2]),
            ]));
        }
    }

    fn choose_palette(&mut self, rgba: &[u8]) {
        self.counts.fill(0);
        self.sums.fill([0; 3]);
        for pixel in rgba.as_chunks::<4>().0 {
            let at = bin([pixel[0], pixel[1], pixel[2]]);
            self.counts[at] += 1;
            for (sum, &value) in self.sums[at].iter_mut().zip(pixel) {
                *sum += u64::from(value);
            }
        }
        let mut used: Vec<usize> = (0..BINS).filter(|&at| self.counts[at] > 0).collect();
        let mut buckets = vec![Bucket {
            start: 0,
            end: used.len(),
            population: self.population(&used),
        }];
        while buckets.len() < COLORS {
            let widest = buckets
                .iter()
                .enumerate()
                .filter(|(_, bucket)| bucket.end - bucket.start > 1)
                .max_by_key(|(_, bucket)| {
                    let (_, extent) = longest_axis(&used[bucket.start..bucket.end]);
                    bucket.population * u64::from(extent)
                })
                .map(|(at, _)| at);
            let Some(widest) = widest else {
                break;
            };
            let Bucket { start, end, .. } = buckets.swap_remove(widest);
            let (axis, _) = longest_axis(&used[start..end]);
            used[start..end].sort_unstable_by_key(|&at| component(at, axis));
            let half = self.population(&used[start..end]) / 2;
            let mut running = 0;
            let split = used[start..end]
                .iter()
                .position(|&at| {
                    running += u64::from(self.counts[at]);
                    running > half
                })
                .unwrap_or(0)
                .clamp(1, end - start - 1);
            for (from, to) in [(start, start + split), (start + split, end)] {
                buckets.push(Bucket {
                    start: from,
                    end: to,
                    population: self.population(&used[from..to]),
                });
            }
        }
        self.palette.clear();
        self.palette.extend(buckets.iter().map(|bucket| {
            let mut sum = [0u64; 3];
            for &at in &used[bucket.start..bucket.end] {
                for (sum, part) in sum.iter_mut().zip(self.sums[at]) {
                    *sum += part;
                }
            }
            sum.map(|part| (part / bucket.population.max(1)) as u8)
        }));
        if self.palette.is_empty() {
            self.palette.push([0; 3]);
        }
    }

    fn population(&self, bins: &[usize]) -> u64 {
        bins.iter().map(|&at| u64::from(self.counts[at])).sum()
    }

    fn nearest_to(&mut self, at: usize) -> u8 {
        if let (true, index) = self.nearest[at] {
            return index;
        }
        let color = [0, 1, 2].map(|axis| i32::from(component(at, axis)) << (8 - BITS) | 4);
        let distance = |entry: &[u8; 3]| -> i32 {
            entry
                .iter()
                .zip(color)
                .map(|(&value, wanted)| (i32::from(value) - wanted).pow(2))
                .sum()
        };
        let index = self
            .palette
            .iter()
            .enumerate()
            .min_by_key(|(_, entry)| distance(entry))
            .map_or(0, |(index, _)| index as u8);
        self.nearest[at] = (true, index);
        index
    }
}

fn bin([r, g, b]: [u8; 3]) -> usize {
    let cut = |value: u8| usize::from(value >> (8 - BITS));
    cut(r) << (2 * BITS) | cut(g) << BITS | cut(b)
}

fn component(at: usize, axis: usize) -> u8 {
    let shift = BITS as usize * (2 - axis);
    ((at >> shift) & ((1 << BITS) - 1)) as u8
}

/// The channel the bins spread furthest along, and how far.
fn longest_axis(bins: &[usize]) -> (usize, u8) {
    (0..3)
        .map(|axis| {
            let values = bins.iter().map(|&at| component(at, axis));
            let low = values.clone().min().unwrap_or(0);
            let high = values.max().unwrap_or(0);
            (axis, high - low)
        })
        .max_by_key(|&(_, extent)| extent)
        .unwrap_or((0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(width: usize, height: usize, color: impl Fn(usize, usize) -> [u8; 3]) -> Vec<u8> {
        (0..width * height)
            .flat_map(|at| {
                let [r, g, b] = color(at % width, at / width);
                [r, g, b, 255]
            })
            .collect()
    }

    #[test]
    fn few_colors_are_kept_exactly() {
        let colors = [[255, 0, 0], [0, 255, 0], [20, 30, 40], [250, 250, 250]];
        let rgba = picture(16, 16, |x, y| colors[(x / 4 + y) % 4]);
        let mut indices = vec![0; 256];
        let mut quantizer = Quantizer::default();
        quantizer.quantize(&rgba, 16, &mut indices);
        for (pixel, &index) in rgba.as_chunks::<4>().0.iter().zip(&indices) {
            assert_eq!(quantizer.palette[usize::from(index)], pixel[..3]);
        }
    }

    #[test]
    fn many_colors_stay_close() {
        let rgba = picture(256, 64, |x, y| [x as u8, (y * 4) as u8, (255 - x) as u8]);
        let mut indices = vec![0; 256 * 64];
        let mut quantizer = Quantizer::default();
        quantizer.quantize(&rgba, 256, &mut indices);
        assert!(quantizer.palette.len() <= COLORS);
        let mut error = 0;
        for (pixel, &index) in rgba.as_chunks::<4>().0.iter().zip(&indices) {
            let entry = quantizer.palette[usize::from(index)];
            let off = |c: usize| (i32::from(entry[c]) - i32::from(pixel[c])).abs();
            error += off(0) + off(1) + off(2);
        }
        let mean = error as f64 / (256.0 * 64.0 * 3.0);
        assert!(mean < 8.0, "{mean}");
    }
}
