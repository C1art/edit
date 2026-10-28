use crate::{Clip, ClipId, MediaItem, Nest, Param, Sequence, SourceId, Ticks, Track};

/// Where the multi-camera view puts each angle and the program, as `[left, top, width,
/// height]` fractions of the frame: the angles in a square grid on the left half and the
/// program on the right, both centered and at the frame's shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MulticamLayout {
    pub angles: usize,
    columns: usize,
}

impl MulticamLayout {
    pub fn new(angles: usize) -> Self {
        let columns = (1..)
            .find(|columns| columns * columns >= angles)
            .unwrap_or(1);
        Self { angles, columns }
    }

    pub fn angle_rect(&self, angle: usize) -> [f32; 4] {
        let cell = 0.5 / self.columns as f32;
        let (column, row) = (angle % self.columns, angle / self.columns);
        [column as f32 * cell, 0.25 + row as f32 * cell, cell, cell]
    }

    pub const PROGRAM: [f32; 4] = [0.5, 0.25, 0.5, 0.5];

    /// The angle whose cell holds `point`, in fractions of the frame.
    pub fn angle_at(&self, [x, y]: [f32; 2]) -> Option<usize> {
        (0..self.angles).find(|&angle| {
            let [left, top, width, height] = self.angle_rect(angle);
            (left..left + width).contains(&x) && (top..top + height).contains(&y)
        })
    }
}

impl Sequence {
    /// The nest of `clip`'s source, when that is a multi-camera source.
    pub fn multicam_of(&self, clip: &Clip) -> Option<&Nest> {
        let nest = self.media_item(clip.source)?.nest.as_ref()?;
        nest.multicam.then_some(nest)
    }

    /// The topmost video track holding a multi-camera clip, whose clips the multi-camera
    /// view shows and switches.
    pub fn multicam_track(&self) -> Option<usize> {
        self.video.iter().rposition(|track| {
            track
                .clips
                .iter()
                .any(|clip| self.multicam_of(clip).is_some())
        })
    }

    /// The multi-camera clip on the multi-camera track at `position`.
    pub fn multicam_clip_at(&self, position: Ticks) -> Option<&Clip> {
        let clip = self.video.get(self.multicam_track()?)?.clip_at(position)?;
        self.multicam_of(clip).is_some().then_some(clip)
    }

    /// This sequence as the multi-camera view shows it, with the same sound: every angle
    /// of the multi-camera track's clips beside the program. None without such a track.
    pub fn multicam_view(&self) -> Option<(Sequence, MulticamLayout)> {
        let track = &self.video[self.multicam_track()?];
        let angles = track
            .clips
            .iter()
            .filter_map(|clip| self.multicam_of(clip))
            .map(|nest| nest.video.len())
            .max()?;
        let layout = MulticamLayout::new(angles);
        let mut view = self.clone();
        let program = SourceId(view.ids.next());
        let duration = self.duration().max(Ticks(1));
        view.media.push(MediaItem {
            id: program,
            name: String::new(),
            path: Default::default(),
            duration,
            video: Some(self.format),
            has_audio: false,
            proxy: None,
            generator: None,
            nest: Some(Nest {
                video: self.video.clone(),
                ..Nest::default()
            }),
            bin: None,
            subclip: None,
            interpreted_rate: None,
            interpreted_aspect: None,
            alpha: Default::default(),
            log: Default::default(),
            offline: false,
            still: false,
            channel_map: None,
        });
        let mut whole = Clip::new(
            ClipId(view.ids.next()),
            program,
            Ticks::ZERO,
            duration,
            Ticks::ZERO,
        );
        self.place(&mut whole, MulticamLayout::PROGRAM);
        view.video = vec![Track {
            clips: vec![whole],
            ..Track::default()
        }];
        for angle in 0..angles {
            let clips = track
                .clips
                .iter()
                .filter(|clip| {
                    self.multicam_of(clip)
                        .is_some_and(|nest| angle < nest.video.len())
                })
                .map(|clip| {
                    let mut shown = Clip {
                        id: ClipId(view.ids.next()),
                        link: None,
                        transition_in: None,
                        transition_out: None,
                        look: Default::default(),
                        angle,
                        ..clip.clone()
                    };
                    self.place(&mut shown, layout.angle_rect(angle));
                    shown
                })
                .collect();
            view.video.push(Track {
                clips,
                ..Track::default()
            });
        }
        view.captions = Default::default();
        Some((view, layout))
    }

    /// Scales and moves `clip`, which fills the frame, into `rect` of it.
    fn place(&self, clip: &mut Clip, [left, top, width, height]: [f32; 4]) {
        let frame = [self.format.width, self.format.height].map(|side| side as f32);
        let center = [left + width / 2.0, top + height / 2.0];
        let motion = &mut clip.look.motion;
        motion.scale = Param::from_value(width * 100.0);
        for ((position, center), side) in motion.position.iter_mut().zip(center).zip(frame) {
            *position = Param::from_value((center - 0.5) * side);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn five_angles_fill_a_three_by_three_grid_in_reading_order() {
        let layout = MulticamLayout::new(5);
        assert_eq!(
            layout.angle_rect(4),
            [0.5 / 3.0, 0.25 + 0.5 / 3.0, 0.5 / 3.0, 0.5 / 3.0]
        );
        assert_eq!(layout.angle_at([0.4, 0.3]), Some(2));
        // The sixth cell is empty, and the program is no angle.
        assert_eq!(layout.angle_at([0.4, 0.5]), None);
        assert_eq!(layout.angle_at([0.7, 0.5]), None);
    }
}
