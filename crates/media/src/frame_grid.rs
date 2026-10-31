use timeline::{FrameRate, Ticks, Timebase};

/// Moves `pts` onto the start of the nearest frame of `rate` when it is within half a
/// tick of the stream's own timebase, `stream`. Containers with coarse timebases, such
/// as Matroska's milliseconds, stamp pictures up to half a millisecond off the frame
/// grid, and a picture stamped just after its frame starts would show a frame late.
pub(crate) fn on_grid(pts: Ticks, rate: FrameRate, output: Timebase, stream: Timebase) -> Ticks {
    let tolerance = Ticks(1).rescale(stream, output).0 / 2;
    if tolerance == 0 {
        return pts;
    }
    let frame = rate.frame_at(pts, output);
    let nearest = [frame, frame + 1]
        .map(|frame| rate.frame_start(frame, output))
        .into_iter()
        .min_by_key(|start| (*start - pts).0.abs());
    nearest
        .filter(|start| (*start - pts).0.abs() <= tolerance)
        .unwrap_or(pts)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MILLISECONDS: Timebase = match Timebase::new(1, 1000) {
        Some(timebase) => timebase,
        None => panic!(),
    };

    #[test]
    fn millisecond_stamps_snap_to_the_ntsc_frame_they_belong_to() {
        let rate = FrameRate::new(30_000, 1001).unwrap();
        let output = Timebase::HZ_90K;
        // Frame 2 starts at 66.733 ms, which Matroska stamps as 67 ms.
        let stamped = Ticks(67).rescale(MILLISECONDS, output);
        assert_eq!(
            on_grid(stamped, rate, output, MILLISECONDS),
            rate.frame_start(2, output)
        );
    }

    #[test]
    fn stamps_far_from_a_frame_start_and_fine_timebases_are_kept() {
        let rate = FrameRate::new(25, 1).unwrap();
        let output = Timebase::HZ_90K;
        assert_eq!(
            on_grid(Ticks(1800), rate, output, MILLISECONDS),
            Ticks(1800)
        );
        assert_eq!(on_grid(Ticks(3601), rate, output, output), Ticks(3601));
    }
}
