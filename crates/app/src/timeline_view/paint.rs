use std::collections::{HashMap, HashSet};

use egui::{Align2, Color32, FontId, Painter, Rect, Stroke, StrokeKind};
use timeline::{Clip, ClipId, Gain, Label, Sequence, Ticks, Timecode, TrackKind};

use super::clip_art::{self, ClipArea};
use super::clip_marks::{
    keyframe_marks, speed_marks, sync_badge, through_edit_marks, transition_marks,
};
use super::layout::{clip_rect, Row};
use super::rubber_band;
use super::scale::{label_interval, TimeScale};
use crate::clip_art::ClipArt;

pub const PLAYHEAD: Color32 = Color32::from_rgb(230, 70, 60);
const VIDEO_CLIP: Color32 = Color32::from_rgb(64, 104, 160);
const AUDIO_CLIP: Color32 = Color32::from_rgb(58, 128, 92);
const CLIP_RADIUS: f32 = 4.0;
const NAME_STRIP: f32 = 17.0;
const DETAIL_WIDTH: f32 = 8.0;
const OFFLINE_CLIP: Color32 = Color32::from_rgb(170, 40, 40);
const MIN_LABEL_SPACING: f32 = 90.0;

/// What's looked up for every clip drawn, worked out once a frame rather than by a
/// search per clip, which slows a long timeline to a crawl.
pub struct ClipStates {
    selected: HashSet<ClipId>,
    sync_offsets: HashMap<ClipId, Ticks>,
}

impl ClipStates {
    pub fn new(sequence: &Sequence, selection: &[ClipId]) -> Self {
        Self {
            selected: selection.iter().copied().collect(),
            sync_offsets: sequence.sync_offsets(),
        }
    }
}

/// A strip along the foot of the ruler over the segments that need rendering to play
/// smoothly: red until rendered, green once a preview is ready, as Premiere's.
pub fn render_bar(
    painter: &Painter,
    ruler: Rect,
    scale: &TimeScale,
    segments: &[(std::ops::Range<Ticks>, bool)],
) {
    for (range, rendered) in segments {
        let (left, right) = (scale.x(range.start), scale.x(range.end));
        if right < ruler.left() || left > ruler.right() {
            continue;
        }
        let color = if *rendered {
            Color32::from_rgb(60, 190, 80)
        } else {
            Color32::from_rgb(210, 50, 45)
        };
        let strip = Rect::from_x_y_ranges(left..=right, ruler.bottom() - 3.0..=ruler.bottom());
        painter.rect_filled(strip, 0.0, color);
    }
}

pub fn ruler(painter: &Painter, rect: Rect, scale: &TimeScale, sequence: &Sequence) {
    let visuals = painter.ctx().global_style().visuals.clone();
    painter.rect_filled(rect, 0.0, visuals.faint_bg_color);
    let (rate, timebase) = (sequence.format.frame_rate, sequence.timebase);
    let interval = label_interval(scale, rate, timebase, MIN_LABEL_SPACING);
    let minor = if interval % 5 == 0 {
        interval / 5
    } else {
        interval
    };
    let first = rate.frame_at(scale.time(rect.left()), timebase).max(0);
    let last = rate.frame_at(scale.time(rect.right()), timebase) + 1;
    let mut frame = first - first % minor;
    let color = visuals.text_color();
    while frame <= last {
        let time = rate.frame_start(frame, timebase);
        let x = scale.x(time);
        let major = frame % interval == 0;
        let height = if major { 10.0 } else { 4.0 };
        painter.line_segment(
            [
                egui::pos2(x, rect.bottom() - height),
                egui::pos2(x, rect.bottom()),
            ],
            Stroke::new(1.0, color.gamma_multiply(0.6)),
        );
        if major {
            painter.text(
                egui::pos2(x + 3.0, rect.top() + 2.0),
                Align2::LEFT_TOP,
                Timecode::new(time, timebase, rate).to_string(),
                FontId::monospace(10.0),
                color,
            );
        }
        frame += minor;
    }
}

pub fn row_background(
    painter: &Painter,
    rect: Rect,
    row: &Row,
    sequence: &Sequence,
    targeted: bool,
) {
    let visuals = painter.ctx().global_style().visuals.clone();
    let mut fill = visuals.extreme_bg_color;
    if targeted {
        fill = fill.lerp_to_gamma(visuals.selection.bg_fill, 0.12);
    }
    painter.rect_filled(
        Rect::from_x_y_ranges(rect.x_range(), row.y_range()),
        0.0,
        fill,
    );
    if sequence.track(row.track).is_some_and(|track| track.locked) {
        painter.rect_filled(
            Rect::from_x_y_ranges(rect.x_range(), row.y_range()),
            0.0,
            Color32::from_black_alpha(90),
        );
    }
    painter.hline(
        rect.x_range(),
        row.bottom(),
        Stroke::new(1.0, visuals.widgets.noninteractive.bg_stroke.color),
    );
}

/// Shades a selected gap like a selected clip's outline.
pub fn gap(painter: &Painter, row: &Row, scale: &TimeScale, range: std::ops::Range<Ticks>) {
    let rect = Rect::from_x_y_ranges(scale.x(range.start)..=scale.x(range.end), row.y_range());
    let selection = painter.ctx().global_style().visuals.selection;
    painter.rect_filled(
        rect.shrink(1.0),
        0.0,
        selection.bg_fill.gamma_multiply(0.35),
    );
    painter.rect_stroke(rect.shrink(1.0), 0.0, selection.stroke, StrokeKind::Inside);
}

pub fn clips(
    painter: &Painter,
    row: &Row,
    scale: &TimeScale,
    sequence: &Sequence,
    states: &ClipStates,
    art: &mut ClipArt,
    volume_lines: bool,
) {
    let Some(track) = sequence.track(row.track) else {
        return;
    };
    let track_color = match row.track.kind {
        TrackKind::Video => VIDEO_CLIP,
        TrackKind::Audio => AUDIO_CLIP,
    };
    let silenced = match row.track.kind {
        TrackKind::Video => track.muted,
        TrackKind::Audio => !sequence.is_heard(track),
    };
    let visible = painter.clip_rect().x_range();
    for clip in &track.clips {
        let dimmed = silenced || clip.disabled;
        let rect = clip_rect(row, scale, clip.start, clip.end());
        if rect.right() < visible.min || rect.left() > visible.max {
            continue;
        }
        let selected = states.selected.contains(&clip.id);
        let offline = art.offline.contains(&clip.source);
        let base = if offline {
            OFFLINE_CLIP
        } else {
            clip.label.map_or(track_color, label_color)
        };
        let mut fill = if selected {
            base.lerp_to_gamma(Color32::WHITE, 0.25)
        } else {
            base
        };
        if dimmed {
            fill = fill.gamma_multiply(0.4);
        }
        // A hair of space between neighbours keeps back-to-back clips apart.
        let body = rect.shrink2(egui::vec2(0.5, 0.0));
        painter.rect_filled(body, CLIP_RADIUS, fill);
        let stroke = if selected {
            Stroke::new(1.5, Color32::WHITE)
        } else {
            Stroke::new(1.0, fill.lerp_to_gamma(Color32::WHITE, 0.18))
        };
        // Nothing more could be made out on a clip this thin, and drawing it all for
        // hundreds of clips zoomed out slows every frame.
        if body.width() < DETAIL_WIDTH {
            painter.rect_stroke(body, CLIP_RADIUS, stroke, StrokeKind::Inside);
            continue;
        }
        let strip = Rect::from_min_max(
            body.left_top(),
            egui::pos2(body.right(), (body.top() + NAME_STRIP).min(body.bottom())),
        );
        if let Some(item) = sequence.media_item(clip.source).filter(|_| !offline) {
            // Thumbnails sit under the name strip when the row is tall enough for both.
            let art_rect = match row.track.kind {
                TrackKind::Video if body.height() > 2.0 * NAME_STRIP => {
                    body.shrink(1.0).with_min_y(strip.bottom())
                }
                _ => body.shrink(1.0),
            };
            let area = ClipArea {
                rect: art_rect,
                clip,
                item,
                timebase: sequence.timebase,
            };
            match row.track.kind {
                TrackKind::Video if art.show_thumbnails => {
                    let tint = if dimmed {
                        Color32::from_gray(100)
                    } else {
                        Color32::WHITE
                    };
                    clip_art::thumbnails(painter, &area, scale, art, tint);
                    if selected {
                        painter.rect_filled(art_rect, 0.0, Color32::from_white_alpha(40));
                    }
                    let corners = egui::CornerRadius {
                        nw: CLIP_RADIUS as u8,
                        ne: CLIP_RADIUS as u8,
                        sw: 0,
                        se: 0,
                    };
                    painter.rect_filled(strip, corners, fill);
                }
                TrackKind::Audio if art.show_waveforms => {
                    // Selection lightens the clip, so its bars go dark to stay visible.
                    let color = if selected {
                        base.gamma_multiply(0.6)
                    } else {
                        fill.lerp_to_gamma(Color32::WHITE, 0.55)
                    };
                    clip_art::waveform(painter, &area, scale, art, color);
                }
                _ => {}
            }
        }
        painter.rect_stroke(body, CLIP_RADIUS, stroke, StrokeKind::Inside);
        transition_marks(painter, row, scale, track, clip);
        super::markers::paint_on_clip(painter, rect, scale, clip);
        match row.track.kind {
            TrackKind::Video => {
                keyframe_marks(painter, rect, scale, clip);
                speed_marks(painter, rect, scale, clip);
            }
            TrackKind::Audio if volume_lines => {
                let envelope = rubber_band::Envelope::clip(clip);
                rubber_band::paint(painter, rect.shrink(1.0), scale, &envelope);
            }
            TrackKind::Audio => {}
        }
        let item = sequence.media_item(clip.source);
        let name = clip_name(sequence, clip);
        let name = if offline {
            format!("{name} [offline]")
        } else {
            name.to_owned()
        };
        let has_both = item.is_some_and(|item| item.video.is_some() && item.has_audio);
        let link = if clip.link.is_none() && has_both {
            " [unlinked]"
        } else {
            ""
        };
        let gain = if row.track.kind == TrackKind::Audio && clip.gain != Gain::UNITY {
            format!(" {:+.1} dB", clip.gain.db())
        } else {
            String::new()
        };
        let speed = match (clip.speed.is_normal(), clip.reverse) {
            _ if clip.remap.is_some() => " [remapped]".to_owned(),
            (true, false) => String::new(),
            (true, true) => " [-100%]".to_owned(),
            (false, reverse) => format!(
                " [{}{}]",
                if reverse { "-" } else { "" },
                crate::speed_dialog::percent_text(clip.speed)
            ),
        };
        let camera = match row.track.kind {
            TrackKind::Video if sequence.multicam_of(clip).is_some() => {
                format!("[MC{}] ", clip.angle + 1)
            }
            _ => String::new(),
        };
        // Held at the view's edge while the clip's start is scrolled out of it.
        let name_left = strip.left().max(visible.min);
        let shown = rect.with_min_x(rect.left().max(visible.min));
        let badge = sync_badge(painter, shown, sequence, states.sync_offsets.get(&clip.id));
        painter.with_clip_rect(strip.shrink(1.0)).text(
            egui::pos2(name_left + 5.0 + badge, strip.center().y),
            Align2::LEFT_CENTER,
            format!("{camera}{name}{speed}{link}{gain}"),
            FontId::proportional(11.0),
            Color32::WHITE,
        );
    }
    through_edit_marks(painter, row, scale, track);
}

pub fn playhead(painter: &Painter, ruler: Rect, bottom: f32, scale: &TimeScale, position: Ticks) {
    let x = scale.x(position);
    if !ruler.x_range().contains(x) {
        return;
    }
    painter.line_segment(
        [egui::pos2(x, ruler.top()), egui::pos2(x, bottom)],
        Stroke::new(1.5, PLAYHEAD),
    );
    let top = ruler.bottom() - 10.0;
    painter.add(egui::Shape::convex_polygon(
        vec![
            egui::pos2(x - 6.0, top),
            egui::pos2(x + 6.0, top),
            egui::pos2(x, ruler.bottom()),
        ],
        PLAYHEAD,
        Stroke::NONE,
    ));
}

/// A vertical guide where a drop or snap will land.
pub fn guide(painter: &Painter, x: f32, top: f32, bottom: f32) {
    painter.line_segment(
        [egui::pos2(x, top), egui::pos2(x, bottom)],
        Stroke::new(1.5, Color32::from_rgb(250, 210, 60)),
    );
}

pub fn clip_name<'a>(sequence: &'a Sequence, clip: &'a Clip) -> &'a str {
    let item = sequence.media_item(clip.source);
    clip.name
        .as_deref()
        .or(item.map(|item| item.name.as_str()))
        .unwrap_or("Missing media")
}

/// Premiere's label colors, toned down so clip names stay readable on them.
pub fn label_color(label: Label) -> Color32 {
    let [r, g, b] = match label {
        Label::Violet => [169, 144, 221],
        Label::Iris => [90, 100, 204],
        Label::Caribbean => [29, 232, 182],
        Label::Lavender => [227, 132, 227],
        Label::Cerulean => [26, 153, 227],
        Label::Forest => [75, 165, 49],
        Label::Rose => [247, 91, 157],
        Label::Mango => [246, 154, 49],
        Label::Purple => [140, 66, 199],
        Label::Blue => [50, 99, 232],
        Label::Teal => [41, 167, 167],
        Label::Magenta => [227, 11, 140],
        Label::Tan => [205, 173, 122],
        Label::Green => [45, 158, 68],
        Label::Brown => [141, 91, 48],
        Label::Yellow => [227, 211, 57],
    };
    Color32::from_rgb(r, g, b).gamma_multiply(0.75)
}
