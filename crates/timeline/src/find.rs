//! Edit > Find: clips by name and sequence markers by name or comment.

use std::sync::Arc;

use crate::{ClipId, MarkerId, Sequence, Ticks};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FindOperator {
    #[default]
    Contains,
    BeginsWith,
    ExactlyMatches,
}

impl FindOperator {
    pub const ALL: [Self; 3] = [Self::Contains, Self::BeginsWith, Self::ExactlyMatches];

    pub fn name(self) -> &'static str {
        match self {
            Self::Contains => "Contains",
            Self::BeginsWith => "Begins With",
            Self::ExactlyMatches => "Exactly Matches",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FindQuery {
    pub text: String,
    pub operator: FindOperator,
    pub match_case: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Found {
    Clip(ClipId),
    Marker(MarkerId),
}

impl FindQuery {
    pub fn matches(&self, candidate: &str) -> bool {
        let (candidate, text) = match self.match_case {
            true => (candidate.to_owned(), self.text.clone()),
            false => (candidate.to_lowercase(), self.text.to_lowercase()),
        };
        match self.operator {
            FindOperator::Contains => candidate.contains(&text),
            FindOperator::BeginsWith => candidate.starts_with(&text),
            FindOperator::ExactlyMatches => candidate == text,
        }
    }
}

impl Sequence {
    /// Everything matching `query`, earliest first. Clips go by the name they show,
    /// their own or else their media's in `project`.
    pub fn find(&self, project: &Sequence, query: &FindQuery) -> Vec<(Ticks, Found)> {
        if query.text.is_empty() {
            return Vec::new();
        }
        let clips = self
            .tracks()
            .flat_map(|(_, track)| track.clips.iter().map(Arc::as_ref));
        let clips = clips.filter_map(|clip| {
            let name = clip.name.as_deref().or_else(|| {
                let item = project.media_item(clip.source)?;
                Some(item.name.as_str())
            })?;
            query
                .matches(name)
                .then_some((clip.start, Found::Clip(clip.id)))
        });
        let markers = self.markers.iter().filter_map(|marker| {
            let found = query.matches(&marker.name) || query.matches(&marker.comment);
            found.then_some((marker.at, Found::Marker(marker.id)))
        });
        let mut found: Vec<_> = clips.chain(markers).collect();
        found.sort_by_key(|&(at, _)| at);
        found
    }
}

/// The first of `found`, sorted by time, after `at`, wrapping around to the start.
pub fn next_found(found: &[(Ticks, Found)], at: Ticks) -> Option<(Ticks, Found)> {
    found
        .iter()
        .find(|(start, _)| *start > at)
        .or_else(|| found.first())
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operators_and_case_decide_what_matches() {
        let query = |text: &str, operator, match_case| FindQuery {
            text: text.to_owned(),
            operator,
            match_case,
        };
        let candidate = "Interview Take 2";
        assert!(query("take", FindOperator::Contains, false).matches(candidate));
        assert!(!query("take", FindOperator::Contains, true).matches(candidate));
        assert!(query("inter", FindOperator::BeginsWith, false).matches(candidate));
        assert!(!query("take", FindOperator::BeginsWith, false).matches(candidate));
        assert!(query("interview take 2", FindOperator::ExactlyMatches, false).matches(candidate));
        assert!(!query("Interview", FindOperator::ExactlyMatches, false).matches(candidate));
    }

    #[test]
    fn find_next_wraps_around_after_the_last_match() {
        let found = [
            (Ticks(10), Found::Clip(ClipId(1))),
            (Ticks(20), Found::Marker(MarkerId(2))),
        ];
        assert_eq!(next_found(&found, Ticks(10)), Some(found[1]));
        assert_eq!(next_found(&found, Ticks(20)), Some(found[0]));
        assert_eq!(next_found(&[], Ticks(0)), None);
    }
}
