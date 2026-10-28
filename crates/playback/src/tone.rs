/// What Color Match and Auto read from a picture, in linear light.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ToneStats {
    /// The mean of each of red, green and blue.
    pub mean: [f32; 3],
    /// The mean of the pixels that are nearly grey, where there are enough of them to
    /// show the picture's cast.
    pub neutral: Option<[f32; 3]>,
    /// The mean of log2 luminance, so a stop of exposure moves it by one.
    pub log_luma: f32,
    /// The standard deviation of log2 luminance, which contrast scales.
    pub log_spread: f32,
    /// The darkest channel value that 1% of the picture lies below: its black point.
    pub low: f32,
    /// Luminance that 1% of the picture lies above.
    pub high: f32,
    /// The mean of each pixel's (max - min) / max, which saturation scales.
    pub chroma: f32,
}

/// Basic Correction values in the units the Lumetri controls show.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct BasicCorrection {
    pub temperature: f32,
    pub tint: f32,
    /// Stops.
    pub exposure: f32,
    pub contrast: f32,
    pub whites: f32,
    pub blacks: f32,
    /// Percent.
    pub saturation: f32,
}

const LUMA: [f32; 3] = [0.2126, 0.7152, 0.0722];
/// The pivot the grade's contrast turns around, and where Auto puts the picture's
/// average.
const MID_GREY: f32 = 0.18;
/// Darker than this counts as black, where log luminance would run off.
const FLOOR: f32 = 1.0 / 1024.0;
/// How far the shader's white balance moves a channel at full temperature or tint.
const BALANCE_GAIN: f32 = 0.3;
/// How far the shader's blacks lift black at their limit.
const BLACK_LIFT: f32 = 0.03;
const AUTO_EXPOSURE_LIMIT: f32 = 2.0;
/// Auto moves white balance, whites and blacks no further than this, so that a picture
/// it misreads is not ruined.
const AUTO_LIMIT: f32 = 0.5;
/// A pixel whose channels differ by less than this share of the brightest counts as
/// grey under a cast.
const NEUTRAL_CHROMA: f32 = 0.4;
/// The share of the picture that must be grey for Auto to balance it.
const NEUTRAL_SHARE: f32 = 0.02;
/// Where Auto puts the brightest 1%.
const AUTO_WHITE: f32 = 0.95;

/// The statistics of an sRGB-encoded RGBA picture, counting only pixels that are
/// mostly opaque; none when there are none.
pub fn tone_stats(rgba: &[u8]) -> Option<ToneStats> {
    let decode: [f32; 256] = std::array::from_fn(|code| srgb_decode(code as f32 / 255.0));
    let pixels: Vec<[f32; 3]> = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] >= 128)
        .map(|pixel| [0, 1, 2].map(|channel| decode[usize::from(pixel[channel])]))
        .collect();
    if pixels.is_empty() {
        return None;
    }
    let count = pixels.len() as f32;
    let mut mean = [0.0; 3];
    let mut chroma = 0.0;
    let (mut greys, mut grey_sum) = (0, [0.0; 3]);
    let mut lumas = Vec::with_capacity(pixels.len());
    let mut leasts = Vec::with_capacity(pixels.len());
    for rgb in &pixels {
        for (sum, value) in mean.iter_mut().zip(rgb) {
            *sum += value / count;
        }
        let peak = rgb[0].max(rgb[1]).max(rgb[2]);
        let least = rgb[0].min(rgb[1]).min(rgb[2]);
        leasts.push(least);
        if peak > FLOOR {
            let pixel_chroma = (peak - least) / peak;
            chroma += pixel_chroma / count;
            // Near black, noise and rounding make any pixel look colored.
            if pixel_chroma < NEUTRAL_CHROMA && peak > 0.02 {
                greys += 1;
                grey_sum
                    .iter_mut()
                    .zip(rgb)
                    .for_each(|(sum, value)| *sum += value);
            }
        }
        lumas.push(
            rgb.iter()
                .zip(LUMA)
                .map(|(value, weight)| value * weight)
                .sum::<f32>(),
        );
    }
    let logs: Vec<f32> = lumas.iter().map(|luma| luma.max(FLOOR).log2()).collect();
    let log_luma = logs.iter().sum::<f32>() / count;
    let variance = logs.iter().map(|log| (log - log_luma).powi(2)).sum::<f32>() / count;
    let neutral =
        (greys as f32 >= count * NEUTRAL_SHARE).then(|| grey_sum.map(|sum| sum / greys as f32));
    Some(ToneStats {
        mean,
        neutral,
        log_luma,
        log_spread: variance.sqrt(),
        low: percentile(&mut leasts, 0.01),
        high: percentile(&mut lumas, 0.99),
        chroma,
    })
}

fn percentile(values: &mut [f32], share: f32) -> f32 {
    let index = ((values.len() - 1) as f32 * share).round() as usize;
    *values.select_nth_unstable_by(index, f32::total_cmp).1
}

/// A correction that neutralizes the picture's cast, brings its average to mid grey
/// and spreads its tones from black to near white, as Lumetri's Auto does.
pub fn auto_correction(stats: &ToneStats) -> BasicCorrection {
    let (temperature, tint) = stats
        .neutral
        .map_or((0.0, 0.0), |neutral| balance(neutral, 1.0, 1.0));
    let exposure =
        (MID_GREY.log2() - stats.log_luma).clamp(-AUTO_EXPOSURE_LIMIT, AUTO_EXPOSURE_LIMIT);
    let gain = exposure.exp2();
    let (high, low) = (stats.high * gain, stats.low * gain);
    // Whites only reach tones above 0.4, as the shader's smoothstep has it.
    let whites = if high > 0.5 {
        (AUTO_WHITE / high).log2() / smoothstep(0.4, 1.0, high)
    } else {
        AUTO_LIMIT
    };
    // Black is pulled down to where the darkest tones are; it is never lifted.
    let blacks = -(low / BLACK_LIFT) / (1.0 - smoothstep(0.0, MID_GREY, low));
    BasicCorrection {
        temperature: percent(temperature.clamp(-AUTO_LIMIT, AUTO_LIMIT)),
        tint: percent(tint.clamp(-AUTO_LIMIT, AUTO_LIMIT)),
        exposure,
        contrast: 0.0,
        whites: percent(whites.clamp(-AUTO_LIMIT, AUTO_LIMIT)),
        blacks: percent(blacks.clamp(-AUTO_LIMIT, 0.0)),
        saturation: 100.0,
    }
}

/// A correction that gives `current` the cast, average brightness, contrast and
/// saturation of `reference`.
pub fn matched_correction(current: &ToneStats, reference: &ToneStats) -> BasicCorrection {
    let contrast = if current.log_spread > f32::EPSILON {
        (reference.log_spread / current.log_spread - 1.0).clamp(-1.0, 1.0)
    } else {
        0.0
    };
    // Contrast raises each channel to a power, which raises their ratios to it too.
    let [red, green, blue] = reference.mean.map(|value| value.max(FLOOR));
    let undo_contrast = |ratio: f32| ratio.powf(1.0 / (1.0 + contrast));
    let (temperature, tint) = balance(
        current.mean,
        undo_contrast(red / blue),
        undo_contrast(green / red),
    );
    // Contrast turns the logs around mid grey, so exposure first puts the average
    // where the turn lands it on the reference's.
    let pivot = MID_GREY.log2();
    let exposure = pivot + (reference.log_luma - pivot) / (1.0 + contrast) - current.log_luma;
    let saturation = if current.chroma > f32::EPSILON {
        (reference.chroma / current.chroma).clamp(0.0, 2.0)
    } else {
        1.0
    };
    BasicCorrection {
        temperature: percent(temperature),
        tint: percent(tint),
        exposure: exposure.clamp(-5.0, 5.0),
        contrast: percent(contrast),
        whites: 0.0,
        blacks: 0.0,
        saturation: saturation * 100.0,
    }
}

/// Temperature and tint, each -1 to 1, that bring the mean's red to blue ratio to
/// `red_to_blue` and green to red ratio to `green_to_red`. The shader scales red by
/// 1 + 0.3t and blue by 1 - 0.3t for temperature t, and green by 1 - 0.3n for tint n.
fn balance(mean: [f32; 3], red_to_blue: f32, green_to_red: f32) -> (f32, f32) {
    let [red, green, blue] = mean.map(|value| value.max(FLOOR));
    let temperature =
        ((red_to_blue * blue - red) / (BALANCE_GAIN * (red + red_to_blue * blue))).clamp(-1.0, 1.0);
    let balanced_red = red * (1.0 + BALANCE_GAIN * temperature);
    let tint = ((1.0 - green_to_red * balanced_red / green) / BALANCE_GAIN).clamp(-1.0, 1.0);
    (temperature, tint)
}

fn percent(unit: f32) -> f32 {
    (unit.clamp(-1.0, 1.0) * 100.0 * 100.0).round() / 100.0
}

fn smoothstep(low: f32, high: f32, x: f32) -> f32 {
    let t = ((x - low) / (high - low)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn srgb_decode(code: f32) -> f32 {
    if code <= 0.04045 {
        code / 12.92
    } else {
        ((code + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode(linear: f32) -> u8 {
        let code = if linear <= 0.003_130_8 {
            linear * 12.92
        } else {
            1.055 * linear.powf(1.0 / 2.4) - 0.055
        };
        (code * 255.0).round() as u8
    }

    /// Half the picture at `dark` and half at `light`, both tinted by `cast`.
    fn picture(dark: f32, light: f32, cast: [f32; 3]) -> Vec<u8> {
        [dark, light]
            .iter()
            .flat_map(|&level| {
                let pixel = cast.map(|gain| encode(level * gain));
                [pixel[0], pixel[1], pixel[2], 255].repeat(50)
            })
            .collect()
    }

    /// The shader's white balance, exposure and contrast on a linear color.
    fn grade(rgb: [f32; 3], correction: &BasicCorrection) -> [f32; 3] {
        let (t, n) = (correction.temperature / 100.0, correction.tint / 100.0);
        let gains = [1.0 + 0.3 * t, 1.0 - 0.3 * n, 1.0 - 0.3 * t];
        let contrast = 1.0 + correction.contrast / 100.0;
        [0, 1, 2].map(|channel| {
            let value = rgb[channel] * gains[channel] * correction.exposure.exp2();
            MID_GREY * (value / MID_GREY).powf(contrast)
        })
    }

    #[test]
    fn auto_neutralizes_a_cast_and_brings_the_average_to_mid_grey() {
        let blue_cast = picture(0.02, 0.2, [0.9, 1.0, 1.12]);
        let stats = tone_stats(&blue_cast).unwrap();
        let correction = auto_correction(&stats);
        assert!(correction.temperature > 0.0, "{correction:?}");
        let [red, green, blue] = grade(stats.mean, &correction);
        assert!((red / blue - 1.0).abs() < 0.03 && (green / red - 1.0).abs() < 0.03);
        assert!((stats.log_luma + correction.exposure - MID_GREY.log2()).abs() < 0.01);
    }

    #[test]
    fn auto_leaves_the_balance_of_a_picture_without_greys() {
        let bars: Vec<u8> = [[255, 0, 0, 255], [0, 255, 0, 255], [255, 0, 255, 255]]
            .iter()
            .flat_map(|pixel| pixel.repeat(40))
            .collect();
        let correction = auto_correction(&tone_stats(&bars).unwrap());
        assert_eq!((correction.temperature, correction.tint), (0.0, 0.0));
    }

    #[test]
    fn matching_a_brighter_contrastier_warmer_picture_reproduces_its_statistics() {
        let current = tone_stats(&picture(0.04, 0.16, [1.0, 1.0, 1.0])).unwrap();
        let reference = tone_stats(&picture(0.03, 0.3, [1.2, 1.0, 0.85])).unwrap();
        let correction = matched_correction(&current, &reference);
        let graded = [0.04, 0.16].map(|level| grade([level; 3], &correction));
        let logs = graded.map(|rgb| {
            let luma: f32 = rgb
                .iter()
                .zip(LUMA)
                .map(|(value, weight)| value * weight)
                .sum();
            luma.log2()
        });
        let log_luma = (logs[0] + logs[1]) / 2.0;
        let spread = (logs[1] - logs[0]) / 2.0;
        assert!(
            (log_luma - reference.log_luma).abs() < 0.05,
            "{correction:?}"
        );
        assert!(
            (spread - reference.log_spread).abs() < 0.05,
            "{correction:?}"
        );
        let [red, _, blue] = graded[1];
        let wanted = reference.mean[0] / reference.mean[2];
        assert!((red / blue - wanted).abs() < 0.05, "{red} {blue} {wanted}");
    }

    #[test]
    fn matching_a_picture_to_itself_changes_nothing() {
        let stats = tone_stats(&picture(0.05, 0.3, [1.1, 1.0, 0.9])).unwrap();
        let correction = matched_correction(&stats, &stats);
        assert!(correction.temperature.abs() < 0.01 && correction.tint.abs() < 0.01);
        assert!(correction.exposure.abs() < 1e-4 && correction.contrast.abs() < 0.01);
        assert!((correction.saturation - 100.0).abs() < 0.01);
    }
}
