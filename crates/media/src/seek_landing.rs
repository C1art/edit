use timeline::Ticks;

/// Where the last seek was meant to land, until its first picture shows whether it did.
/// Containers without a keyframe index, such as MPEG program and transport streams, can
/// land past the keyframe a picture needs, so the decoder's first picture comes late.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Landing {
    pub(crate) position: Ticks,
    /// How much earlier than `position` the demuxer was asked for, in microseconds.
    pub(crate) lead: i64,
    pub(crate) late: bool,
}

impl Landing {
    pub(crate) fn after_picture(self, pts: Ticks) -> Option<Self> {
        match self.late || pts > self.position {
            true => Some(Self { late: true, ..self }),
            false => None,
        }
    }
}
