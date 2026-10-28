/// Levels of the pyramid, each half the size of the one before.
const LEVELS: usize = 3;
/// How far the whole picture is searched for at the coarsest level, in its pixels.
const GLOBAL_REACH: i32 = 8;
/// How far each block's match is refined around the coarser level's, in pixels.
const REFINE_REACH: i32 = 2;
/// Half a block's side, in pixels of whichever level it is matched at.
pub(crate) const HALF_BLOCK: i32 = 6;
const GRID: (i32, i32) = (16, 9);
/// The least mean squared step between neighboring pixels, across the block's weakest
/// direction, of a block worth matching. Flat sky matches anywhere, and a straight edge
/// anywhere along itself, which reads as a turn or zoom that isn't there.
const MIN_CORNERNESS: f32 = 16.0;

/// A grayscale picture, 0 to 255.
pub(crate) struct Gray {
    width: i32,
    height: i32,
    pixels: Vec<f32>,
}

impl Gray {
    pub(crate) fn from_rgba(rgba: &[u8], width: u32, height: u32) -> Self {
        let pixels = rgba
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&[r, g, b, _]| {
                0.2126 * f32::from(r) + 0.7152 * f32::from(g) + 0.0722 * f32::from(b)
            })
            .collect();
        Self {
            width: i32::try_from(width).unwrap_or(0),
            height: i32::try_from(height).unwrap_or(0),
            pixels,
        }
    }

    fn at(&self, x: i32, y: i32) -> f32 {
        self.pixels
            .get((y * self.width + x) as usize)
            .copied()
            .unwrap_or(0.0)
    }

    fn half(&self) -> Self {
        let (width, height) = (self.width / 2, self.height / 2);
        let pixels = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x, y)))
            .map(|(x, y)| {
                let (x, y) = (2 * x, 2 * y);
                (self.at(x, y) + self.at(x + 1, y) + self.at(x, y + 1) + self.at(x + 1, y + 1))
                    / 4.0
            })
            .collect();
        Self {
            width,
            height,
            pixels,
        }
    }

    /// The mean difference between the block around `center` here and the one around
    /// `center + shift` in `other`, if both lie inside their pictures.
    fn difference(
        &self,
        other: &Self,
        center: [i32; 2],
        shift: [i32; 2],
        half: i32,
    ) -> Option<f32> {
        let inside = |picture: &Self, [x, y]: [i32; 2]| {
            x - half >= 0 && y - half >= 0 && x + half < picture.width && y + half < picture.height
        };
        let moved = [center[0] + shift[0], center[1] + shift[1]];
        if !inside(self, center) || !inside(other, moved) {
            return None;
        }
        let mut sum = 0.0;
        for dy in -half..=half {
            for dx in -half..=half {
                let here = self.at(center[0] + dx, center[1] + dy);
                sum += (here - other.at(moved[0] + dx, moved[1] + dy)).abs();
            }
        }
        Some(sum / ((2 * half + 1) * (2 * half + 1)) as f32)
    }

    /// How strongly the block around a point changes in the direction it changes
    /// least: the smaller eigenvalue of its gradients' structure tensor.
    fn cornerness(&self, [x, y]: [i32; 2], half: i32) -> f32 {
        let (mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0);
        for dy in -half..half {
            for dx in -half..half {
                let here = self.at(x + dx, y + dy);
                let across = self.at(x + dx + 1, y + dy) - here;
                let down = self.at(x + dx, y + dy + 1) - here;
                xx += across * across;
                yy += down * down;
                xy += across * down;
            }
        }
        let count = (4 * half * half) as f32;
        let (xx, yy, xy) = (xx / count, yy / count, xy / count);
        (xx + yy) / 2.0 - (((xx - yy) / 2.0).powi(2) + xy * xy).sqrt()
    }
}

/// A picture with its successively halved copies, finest first.
pub(crate) struct Pyramid(Vec<Gray>);

impl Pyramid {
    pub(crate) fn new(picture: Gray) -> Self {
        let mut levels = vec![picture];
        while levels.len() < LEVELS {
            let next = levels[levels.len() - 1].half();
            levels.push(next);
        }
        Self(levels)
    }

    pub(crate) fn size(&self) -> [i32; 2] {
        [self.0[0].width, self.0[0].height]
    }

    /// Where blocks of this picture moved to in `next`, as pairs of their centers in
    /// pixels.
    pub(crate) fn matches(&self, next: &Self) -> Vec<([f64; 2], [f64; 2])> {
        let coarsest = LEVELS - 1;
        let global = best_shift(&self.0[coarsest], &next.0[coarsest]);
        let [width, height] = self.size();
        let margin = HALF_BLOCK + 1;
        let step = |extent: i32, count: i32| (extent - 2 * margin) as f32 / (count - 1) as f32;
        let (step_x, step_y) = (step(width, GRID.0), step(height, GRID.1));
        let centers = (0..GRID.1)
            .flat_map(|row| (0..GRID.0).map(move |column| (column, row)))
            .map(|(column, row)| {
                [
                    margin + (column as f32 * step_x).round() as i32,
                    margin + (row as f32 * step_y).round() as i32,
                ]
            });
        self.match_blocks(next, centers, global.map(|d| d << coarsest))
    }

    /// Where the blocks around `centers` moved to in `next`, as pairs of their centers
    /// in pixels, searched for around `shift` in pixels. Blocks too plain to place are
    /// left out.
    pub(crate) fn match_blocks(
        &self,
        next: &Self,
        centers: impl Iterator<Item = [i32; 2]>,
        shift: [i32; 2],
    ) -> Vec<([f64; 2], [f64; 2])> {
        let coarsest = LEVELS - 1;
        let finest = &self.0[0];
        centers
            .filter(|&center| finest.cornerness(center, HALF_BLOCK) >= MIN_CORNERNESS)
            .filter_map(|center| {
                let mut found = shift;
                for level in (0..coarsest).rev() {
                    let at = center.map(|c| c >> level);
                    let guess = found.map(|d| d >> level);
                    let (refined, _) = refine(&self.0[level], &next.0[level], at, guess)?;
                    found = refined.map(|d| d << level);
                }
                let fine = subpixel(finest, &next.0[0], center, found)?;
                let from = center.map(f64::from);
                Some((from, [from[0] + fine[0], from[1] + fine[1]]))
            })
            .collect()
    }

    /// The shift in pixels, searched for around `guess` on the coarsest level, that best
    /// matches the picture around `samples` to `next`.
    pub(crate) fn region_shift(
        &self,
        next: &Self,
        samples: &[[i32; 2]],
        guess: [i32; 2],
    ) -> [i32; 2] {
        let coarsest = LEVELS - 1;
        let (picture, next) = (&self.0[coarsest], &next.0[coarsest]);
        let guess = guess.map(|d| d >> coarsest);
        let mut best = (guess, f32::INFINITY);
        for dy in -GLOBAL_REACH..=GLOBAL_REACH {
            for dx in -GLOBAL_REACH..=GLOBAL_REACH {
                let shift = [guess[0] + dx, guess[1] + dy];
                let differences = samples
                    .iter()
                    .map(|sample| sample.map(|c| c >> coarsest))
                    .filter_map(|at| picture.difference(next, at, shift, 1));
                let (sum, count) = differences.fold((0.0, 0), |(sum, count), difference| {
                    (sum + difference, count + 1)
                });
                if count == 0 {
                    continue;
                }
                // Ties go to the smaller change, so a plain region keeps its course.
                let mean = sum / count as f32 + 1e-4 * (dx.abs() + dy.abs()) as f32;
                if mean < best.1 {
                    best = (shift, mean);
                }
            }
        }
        best.0.map(|d| d << coarsest)
    }
}

/// The shift of the whole picture that best matches `next`, at `GLOBAL_REACH`.
fn best_shift(picture: &Gray, next: &Gray) -> [i32; 2] {
    let reach = GLOBAL_REACH;
    let mut best = ([0, 0], f32::INFINITY);
    for dy in -reach..=reach {
        for dx in -reach..=reach {
            let mut sum = 0.0;
            let mut count = 0;
            for y in reach..picture.height - reach {
                for x in reach..picture.width - reach {
                    sum += (picture.at(x, y) - next.at(x + dx, y + dy)).abs();
                    count += 1;
                }
            }
            let mean = sum / count.max(1) as f32;
            // Ties go to the smaller shift, so a flat picture reads as still.
            let mean = mean + 1e-4 * (dx.abs() + dy.abs()) as f32;
            if mean < best.1 {
                best = ([dx, dy], mean);
            }
        }
    }
    best.0
}

fn refine(
    picture: &Gray,
    next: &Gray,
    center: [i32; 2],
    guess: [i32; 2],
) -> Option<([i32; 2], f32)> {
    let mut best: Option<([i32; 2], f32)> = None;
    for dy in -REFINE_REACH..=REFINE_REACH {
        for dx in -REFINE_REACH..=REFINE_REACH {
            let shift = [guess[0] + dx, guess[1] + dy];
            let Some(difference) = picture.difference(next, center, shift, HALF_BLOCK) else {
                continue;
            };
            if best.is_none_or(|(_, least)| difference < least) {
                best = Some((shift, difference));
            }
        }
    }
    best
}

/// The block's shift found to a fraction of a pixel, by fitting a parabola through the
/// differences either side of the best whole one on each axis.
fn subpixel(picture: &Gray, next: &Gray, center: [i32; 2], guess: [i32; 2]) -> Option<[f64; 2]> {
    let (shift, least) = refine(picture, next, center, guess)?;
    let at = |offset: [i32; 2]| {
        picture.difference(
            next,
            center,
            [shift[0] + offset[0], shift[1] + offset[1]],
            HALF_BLOCK,
        )
    };
    let along = |before: Option<f32>, after: Option<f32>| match (before, after) {
        (Some(before), Some(after)) => {
            let curve = before - 2.0 * least + after;
            if curve > 1e-6 {
                f64::from(((before - after) / (2.0 * curve)).clamp(-0.5, 0.5))
            } else {
                0.0
            }
        }
        _ => 0.0,
    };
    Some([
        f64::from(shift[0]) + along(at([-1, 0]), at([1, 0])),
        f64::from(shift[1]) + along(at([0, -1]), at([0, 1])),
    ])
}
