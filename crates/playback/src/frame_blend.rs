use media::VideoFrame;
use timeline::Ticks;

/// Steps of the cross-fade between two frames.
const FULL: u32 = 256;

/// How far `time` lies from `earlier` toward `later`, in 256ths.
pub(crate) fn weight(earlier: Ticks, later: Ticks, time: Ticks) -> u32 {
    let span = (later - earlier).0;
    if span <= 0 {
        return 0;
    }
    let past = (time - earlier).0.clamp(0, span);
    u32::try_from(i128::from(past) * i128::from(FULL) / i128::from(span)).unwrap_or(0)
}

/// `earlier` faded toward `later` by `weight` 256ths, keeping the earlier frame's time.
pub(crate) fn blend(earlier: &VideoFrame, later: &VideoFrame, weight: u32) -> VideoFrame {
    let weight = weight.min(FULL);
    let pixels = earlier
        .pixels
        .iter()
        .zip(&later.pixels)
        .map(|(&from, &to)| {
            let mixed = u32::from(from) * (FULL - weight) + u32::from(to) * weight;
            ((mixed + FULL / 2) / FULL) as u8
        })
        .collect();
    VideoFrame { pixels, ..*earlier }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn halfway_between_frames_mixes_them_evenly() {
        assert_eq!(weight(Ticks(3600), Ticks(7200), Ticks(5400)), 128);
        assert_eq!(weight(Ticks(3600), Ticks(7200), Ticks(3600)), 0);
        assert_eq!(weight(Ticks(3600), Ticks(7200), Ticks(9000)), 256);
        let frame = |value| VideoFrame {
            pts: Ticks(0),
            width: 1,
            height: 1,
            pixels: vec![value; 4],
        };
        assert_eq!(blend(&frame(0), &frame(255), 128).pixels, vec![128; 4]);
        assert_eq!(blend(&frame(10), &frame(250), 0).pixels, vec![10; 4]);
    }
}
