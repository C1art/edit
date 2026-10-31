use std::fmt::Write;

use crate::{MarkerKind, Sequence};

const HEADER: &str = "Marker Name,Description,In,Out,Duration,Marker Type,Color,URL";

/// The sequence markers in time order as comma-separated values, one row each, timed as
/// the sequence shows times. The text starts with a byte order mark, without which Excel
/// reads the file in the system's code page and garbles any name outside ASCII.
pub fn markers_csv(sequence: &Sequence) -> String {
    let mut markers: Vec<_> = sequence.markers.iter().collect();
    markers.sort_by_key(|marker| marker.at);
    let mut text = format!("\u{feff}{HEADER}\r\n");
    for marker in markers {
        let url = match &marker.kind {
            MarkerKind::WebLink { url } => url.as_str(),
            _ => "",
        };
        let fields = [
            field(&marker.name),
            field(&marker.comment),
            sequence.timecode_at(marker.at).to_string(),
            sequence.timecode_at(marker.end()).to_string(),
            sequence.timecode(marker.duration).to_string(),
            marker.kind.name().to_owned(),
            marker.color.name().to_owned(),
            field(url),
        ];
        // Writing to a String cannot fail.
        let _ = write!(text, "{}\r\n", fields.join(","));
    }
    text
}

/// `value` quoted when it holds what would otherwise end the field or row early.
fn field(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Marker, MarkerColor, MarkerId, Ticks, Timebase, VideoFormat};

    #[test]
    fn markers_come_out_in_time_order_with_awkward_text_quoted() {
        let timebase = Timebase::new(1, 25).expect("valid timebase");
        let mut sequence = Sequence::new(timebase, VideoFormat::default());
        let marker = |id, at, duration, name: &str, comment: &str| Marker {
            id: MarkerId(id),
            at: Ticks(at),
            duration: Ticks(duration),
            name: name.into(),
            comment: comment.into(),
            color: MarkerColor::Red,
            kind: MarkerKind::Comment,
        };
        sequence.markers = vec![
            marker(1, 1500, 0, "Ending", "line one\nline \"two\""),
            marker(2, 50, 25, "Intro, cold open", ""),
        ];
        let text = markers_csv(&sequence);
        let rows: Vec<_> = text.split("\r\n").collect();
        assert_eq!(
            rows[1],
            "\"Intro, cold open\",,00:00:02:00,00:00:03:00,00:00:01:00,Comment Marker,Red,"
        );
        assert_eq!(
            rows[2],
            "Ending,\"line one\nline \"\"two\"\"\",00:01:00:00,00:01:00:00,00:00:00:00,\
             Comment Marker,Red,"
        );
    }
}
