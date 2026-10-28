/// How finely the player decodes pictures: below full, decoded frames are shrunk
/// before compositing, so heavy media plays more smoothly. Export always uses full.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Resolution {
    #[default]
    Full,
    Half,
    Quarter,
}

impl Resolution {
    pub const ALL: [Self; 3] = [Self::Full, Self::Half, Self::Quarter];

    pub fn name(self) -> &'static str {
        match self {
            Self::Full => "Full",
            Self::Half => "1/2",
            Self::Quarter => "1/4",
        }
    }

    pub(crate) fn divisor(self) -> u32 {
        match self {
            Self::Full => 1,
            Self::Half => 2,
            Self::Quarter => 4,
        }
    }

    /// The decoded size of a `width` by `height` picture, or `None` to keep it.
    pub(crate) fn shrink(self, width: u32, height: u32) -> Option<(u32, u32)> {
        let divisor = self.divisor();
        (divisor > 1).then(|| ((width / divisor).max(2), (height / divisor).max(2)))
    }
}
