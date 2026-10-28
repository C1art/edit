use timeline::{Keyframe, Param, Ticks, Timebase};

/// How a track's mixer fader treats its volume keyframes during playback, as in
/// Premiere's Audio Track Mixer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum AutomationMode {
    /// Follows the keyframes; moving the fader changes the track's volume instead.
    #[default]
    Read,
    /// Records from the first touch until playback stops.
    Latch,
    /// Records while the fader is held, then returns to the keyframes.
    Touch,
    /// Records the fader for the whole of playback, touched or not.
    Write,
}

impl AutomationMode {
    pub(crate) const ALL: [Self; 4] = [Self::Read, Self::Latch, Self::Touch, Self::Write];

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Read => "Read",
            Self::Latch => "Latch",
            Self::Touch => "Touch",
            Self::Write => "Write",
        }
    }

    pub(crate) fn tip(self) -> &'static str {
        match self {
            Self::Read => "The fader follows the track's volume keyframes.",
            Self::Latch => "Records the fader from when it's first moved until playback stops.",
            Self::Touch => "Records the fader while it's held, then returns to the keyframes.",
            Self::Write => "Records the fader for as long as playback runs.",
        }
    }

    /// Whether a pass records now, given whether one already is and whether the fader is
    /// being held or was just moved.
    pub(crate) fn records(self, recording: bool, held: bool) -> bool {
        match self {
            Self::Read => false,
            Self::Latch => recording || held,
            Self::Touch => held,
            Self::Write => true,
        }
    }
}

/// How long the automation takes to return to what a pass replaced, as Premiere's
/// Automatch Time.
const AUTOMATCH_SECONDS: i64 = 1;
/// Recorded points this close to the line through their neighbours, in dB, are dropped.
const THINNING_DB: f32 = 0.05;

/// The fader levels heard so far in one recording pass, in dB of automation.
#[derive(Debug, Clone, Default)]
pub(crate) struct Pass {
    samples: Vec<(Ticks, f32)>,
}

impl Pass {
    pub(crate) fn last(&self) -> Option<(Ticks, f32)> {
        self.samples.last().copied()
    }

    pub(crate) fn record(&mut self, time: Ticks, db: f32) {
        match self.samples.last_mut() {
            Some(last) if last.0 == time => last.1 = db,
            _ => self.samples.push((time, db)),
        }
    }

    /// `automation` with this pass written over it, ramping back to what was there
    /// over the automatch time after the pass ends.
    pub(crate) fn spliced(&self, automation: &Param, timebase: Timebase) -> Param {
        let automatch =
            Ticks(AUTOMATCH_SECONDS * i64::from(timebase.den()) / i64::from(timebase.num()).max(1));
        spliced(automation, &self.samples, automatch)
    }
}

fn spliced(automation: &Param, samples: &[(Ticks, f32)], automatch: Ticks) -> Param {
    let (Some(&(start, _)), Some(&(end, _))) = (samples.first(), samples.last()) else {
        return automation.clone();
    };
    let resume = end + automatch;
    let mut param = automation.clone();
    param
        .keys
        .retain(|key| key.time < start || key.time > resume);
    let recorded = thinned(samples)
        .into_iter()
        .chain(std::iter::once((resume, automation.value_at(resume))))
        .map(|(time, db)| Keyframe::new(time, (db * 100.0).round() as i32));
    param.keys.extend(recorded);
    param.keys.sort_by_key(|key| key.time);
    param
}

/// Drops the points a straight line through their neighbours already passes through,
/// so a held fader leaves two keyframes rather than one a frame.
fn thinned(samples: &[(Ticks, f32)]) -> Vec<(Ticks, f32)> {
    let mut kept: Vec<(Ticks, f32)> = Vec::with_capacity(samples.len());
    for &sample in samples {
        if let [.., before, middle] = kept[..] {
            if on_line(before, middle, sample) {
                kept.pop();
            }
        }
        kept.push(sample);
    }
    kept
}

fn on_line(before: (Ticks, f32), middle: (Ticks, f32), after: (Ticks, f32)) -> bool {
    let span = (after.0 - before.0).0 as f32;
    let progress = (middle.0 - before.0).0 as f32 / span;
    let expected = before.1 + (after.1 - before.1) * progress;
    (expected - middle.1).abs() <= THINNING_DB
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(param: &Param) -> Vec<(i64, i32)> {
        param
            .keys
            .iter()
            .map(|key| (key.time.0, key.hundredths))
            .collect()
    }

    #[test]
    fn a_held_fader_leaves_only_its_turns() {
        let ramp_then_hold: Vec<(Ticks, f32)> = (0..10)
            .map(|frame| (Ticks(frame * 10), -(frame.min(5) as f32)))
            .collect();
        let thinned = thinned(&ramp_then_hold);
        assert_eq!(
            thinned,
            [(Ticks(0), 0.0), (Ticks(50), -5.0), (Ticks(90), -5.0)]
        );
    }

    #[test]
    fn a_pass_replaces_only_what_it_covered_and_ramps_back() {
        let mut automation = Param::new(0);
        automation.keys = vec![
            Keyframe::new(Ticks(0), 0),
            Keyframe::new(Ticks(150), -600),
            Keyframe::new(Ticks(400), -600),
        ];
        let pass = [(Ticks(100), -2.0), (Ticks(200), -12.0)];
        let spliced = spliced(&automation, &pass, Ticks(100));
        assert_eq!(
            keys(&spliced),
            [(0, 0), (100, -200), (200, -1200), (300, -600), (400, -600)]
        );
    }
}
