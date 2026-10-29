use crate::{Marks, Ticks, TimelineError};

/// Where a source monitor edit lands and what of the source it takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditPoints {
    /// In the source monitor's time.
    pub source_in: Ticks,
    pub source_out: Ticks,
    pub at: Ticks,
    /// How long the clips last when their speed changes so the source fills the
    /// sequence marks; none when they play at their own speed.
    pub fill: Option<Ticks>,
}

/// What to give up when both monitors have an in and an out mark spanning different
/// lengths, as Premiere's Fit Clip dialog asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FourPoint {
    /// Changes the clip's speed so the source range fills the sequence range.
    FitToFill,
    /// Takes as much of the source from its in mark as the sequence range lasts.
    IgnoreSourceOut,
    /// Places the whole source range from the sequence in mark.
    IgnoreSequenceOut,
}

/// Three-point editing: any two or three of the four marks decide the edit, the
/// sequence range's length taking precedence over the source's, and an out mark alone
/// back-timing the edit to end there. Without sequence marks the edit lands at the
/// playhead. `four_point` settles an edit marked on both sides with differing lengths;
/// none means there is a choice to make, returned as `Ok(None)`.
pub fn edit_points(
    source: Marks,
    source_end: Ticks,
    sequence: Marks,
    playhead: Ticks,
    four_point: Option<FourPoint>,
) -> Result<Option<EditPoints>, TimelineError> {
    let marked = source.range(source_end);
    let points = |source_in: Ticks, source_out: Ticks, at: Ticks| EditPoints {
        source_in,
        source_out,
        at,
        fill: None,
    };
    let points = match (sequence.mark_in, sequence.mark_out) {
        (Some(mark_in), Some(mark_out)) => {
            let length = mark_out - mark_in;
            match (source.mark_in, source.mark_out) {
                (Some(_), Some(_)) if marked.end - marked.start != length => match four_point {
                    None => return Ok(None),
                    Some(FourPoint::FitToFill) => EditPoints {
                        fill: Some(length),
                        ..points(marked.start, marked.end, mark_in)
                    },
                    Some(FourPoint::IgnoreSourceOut) => {
                        points(marked.start, marked.start + length, mark_in)
                    }
                    Some(FourPoint::IgnoreSequenceOut) => points(marked.start, marked.end, mark_in),
                },
                (None, Some(source_out)) => points(source_out - length, source_out, mark_in),
                _ => points(marked.start, marked.start + length, mark_in),
            }
        }
        (Some(mark_in), None) => points(marked.start, marked.end, mark_in),
        (None, Some(mark_out)) => {
            let at = mark_out - (marked.end - marked.start);
            points(marked.start, marked.end, at)
        }
        (None, None) => points(marked.start, marked.end, playhead),
    };
    if points.source_out <= points.source_in {
        return Err(TimelineError::NonPositiveDuration);
    }
    if points.source_in < Ticks::ZERO || points.source_out > source_end {
        return Err(TimelineError::NotEnoughSource);
    }
    if points.at < Ticks::ZERO {
        return Err(TimelineError::BeforeSequenceStart);
    }
    Ok(Some(points))
}

#[cfg(test)]
mod tests {
    use super::*;

    const END: Ticks = Ticks(1000);
    const PLAYHEAD: Ticks = Ticks(7);

    fn marks(mark_in: Option<i64>, mark_out: Option<i64>) -> Marks {
        Marks {
            mark_in: mark_in.map(Ticks),
            mark_out: mark_out.map(Ticks),
        }
    }

    fn resolve(source: Marks, sequence: Marks, four: Option<FourPoint>) -> (i64, i64, i64) {
        let points = edit_points(source, END, sequence, PLAYHEAD, four)
            .unwrap()
            .unwrap();
        (points.source_in.0, points.source_out.0, points.at.0)
    }

    #[test]
    fn the_sequence_range_sets_the_length_and_an_out_mark_alone_back_times() {
        let source_in = marks(Some(100), None);
        let both = marks(Some(500), Some(600));
        assert_eq!(resolve(source_in, both, None), (100, 200, 500));
        assert_eq!(resolve(marks(None, Some(300)), both, None), (200, 300, 500));
        let source_range = marks(Some(100), Some(150));
        assert_eq!(
            resolve(source_range, marks(None, Some(600)), None),
            (100, 150, 550)
        );
        assert_eq!(
            resolve(source_range, marks(Some(40), None), None),
            (100, 150, 40)
        );
        assert_eq!(resolve(source_range, Marks::default(), None), (100, 150, 7));
    }

    #[test]
    fn differing_four_point_ranges_need_a_choice_and_fit_to_fill_keeps_both() {
        let source = marks(Some(100), Some(150));
        let sequence = marks(Some(500), Some(600));
        assert_eq!(edit_points(source, END, sequence, PLAYHEAD, None), Ok(None));
        let fit = edit_points(source, END, sequence, PLAYHEAD, Some(FourPoint::FitToFill));
        let fit = fit.unwrap().unwrap();
        assert_eq!((fit.source_in, fit.source_out), (Ticks(100), Ticks(150)));
        assert_eq!(fit.fill, Some(Ticks(100)));
        let trimmed = resolve(source, sequence, Some(FourPoint::IgnoreSourceOut));
        assert_eq!(trimmed, (100, 200, 500));
    }

    #[test]
    fn marks_asking_past_the_media_are_refused() {
        let sequence = marks(Some(0), Some(200));
        let late = edit_points(marks(Some(900), None), END, sequence, PLAYHEAD, None);
        assert_eq!(late, Err(TimelineError::NotEnoughSource));
        let early = edit_points(marks(None, Some(50)), END, sequence, PLAYHEAD, None);
        assert_eq!(early, Err(TimelineError::NotEnoughSource));
    }
}
