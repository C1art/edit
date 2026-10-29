use timeline::ClipId;

/// A transition plays two clips of one track at once.
const DECODERS_PER_TRACK: usize = 2;

/// The decoders of one track, each following one clip so both sides of a transition
/// keep reading forward.
pub(crate) struct ClipDecoders<T> {
    decoders: Vec<(Option<ClipId>, T)>,
}

impl<T> ClipDecoders<T> {
    pub(crate) fn new(make: impl Fn() -> T) -> Self {
        Self {
            decoders: (0..DECODERS_PER_TRACK).map(|_| (None, make())).collect(),
        }
    }

    /// The decoder following `clip`, or else one that no clip in `keep` is using.
    pub(crate) fn for_clip(&mut self, clip: ClipId, keep: &[ClipId]) -> &mut T {
        let index = self
            .decoders
            .iter()
            .position(|(owner, _)| *owner == Some(clip))
            .or_else(|| {
                self.decoders
                    .iter()
                    .position(|(owner, _)| owner.is_none_or(|owner| !keep.contains(&owner)))
            })
            .unwrap_or(0);
        let (owner, decoder) = &mut self.decoders[index];
        *owner = Some(clip);
        decoder
    }

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut T> {
        self.decoders.iter_mut().map(|(_, decoder)| decoder)
    }
}
