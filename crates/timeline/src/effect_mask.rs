//! Masks that keep an effect to part of the picture, as Premiere draws them under each
//! effect, beside the opacity masks that keep part of the whole clip.

use serde::{Deserialize, Serialize};

use crate::{Look, Mask, Param, VideoEffect};

/// Whose mask list a mask is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MaskOwner {
    Opacity,
    /// The effect at this place in the look's effects.
    Effect(usize),
}

/// One mask of a look.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaskRef {
    pub owner: MaskOwner,
    pub index: usize,
}

impl Look {
    pub fn masks_of(&self, owner: MaskOwner) -> &[Mask] {
        match owner {
            MaskOwner::Opacity => &self.masks,
            MaskOwner::Effect(effect) => self.effect_masks.get(effect).map_or(&[], Vec::as_slice),
        }
    }

    /// The list to add `owner`'s masks to; `None` for an effect the look doesn't have.
    pub fn masks_of_mut(&mut self, owner: MaskOwner) -> Option<&mut Vec<Mask>> {
        match owner {
            MaskOwner::Opacity => Some(&mut self.masks),
            MaskOwner::Effect(effect) if effect < self.effects.len() => {
                if self.effect_masks.len() <= effect {
                    self.effect_masks.resize_with(effect + 1, Vec::new);
                }
                self.effect_masks.get_mut(effect)
            }
            MaskOwner::Effect(_) => None,
        }
    }

    pub fn mask(&self, at: MaskRef) -> Option<&Mask> {
        self.masks_of(at.owner).get(at.index)
    }

    pub fn mask_mut(&mut self, at: MaskRef) -> Option<&mut Mask> {
        self.masks_of_mut(at.owner)?.get_mut(at.index)
    }

    /// Every mask, the opacity masks first, then each effect's in order.
    pub fn mask_refs(&self) -> impl Iterator<Item = MaskRef> + '_ {
        let owners = std::iter::once(MaskOwner::Opacity)
            .chain((0..self.effect_masks.len()).map(MaskOwner::Effect));
        owners.flat_map(move |owner| {
            (0..self.masks_of(owner).len()).map(move |index| MaskRef { owner, index })
        })
    }

    pub fn remove_effect(&mut self, effect: usize) {
        if effect < self.effects.len() {
            self.effects.remove(effect);
        }
        if effect < self.effect_masks.len() {
            self.effect_masks.remove(effect);
        }
        self.tidy_effect_masks();
    }

    pub fn clear_effects(&mut self) {
        self.effects.clear();
        self.effect_masks.clear();
    }

    /// Adds `effects` after the look's own, each with its masks in `masks`.
    pub fn append_effects(&mut self, look: &Look) {
        if !look.effect_masks.is_empty() {
            self.effect_masks.resize_with(self.effects.len(), Vec::new);
            self.effect_masks.extend(look.effect_masks.iter().cloned());
        }
        self.effects.extend(look.effects.iter().cloned());
        self.tidy_effect_masks();
    }

    /// The look with only its effects and their masks, everything else at its default.
    pub fn effects_only(&self) -> Look {
        Look {
            effects: self.effects.clone(),
            effect_masks: self.effect_masks.clone(),
            ..Look::default()
        }
    }

    pub fn params_of_effects_mut(&mut self) -> impl Iterator<Item = &mut Param> {
        let masks = self.effect_masks.iter_mut().flatten();
        (self.effects.iter_mut())
            .flat_map(VideoEffect::params_mut)
            .chain(masks.flat_map(Mask::params_mut))
    }

    /// Whether any effect reaches only inside its masks.
    pub fn has_effect_masks(&self) -> bool {
        self.effect_masks.iter().any(|masks| !masks.is_empty())
    }

    /// The look with only the effects that reach everywhere, then for each effect kept
    /// to its masks, the look with that effect too and the masks it's kept to.
    pub fn split_by_effect_masks(&self) -> (Look, Vec<(Look, &[Mask])>) {
        let masked = |effect: usize| self.masks_of(MaskOwner::Effect(effect));
        let keeping = |keep: &dyn Fn(usize) -> bool| Look {
            effects: (self.effects.iter().enumerate())
                .filter(|&(effect, _)| keep(effect))
                .map(|(_, effect)| effect.clone())
                .collect(),
            effect_masks: Vec::new(),
            ..self.clone()
        };
        let plain = keeping(&|effect| masked(effect).is_empty());
        let reaching = (0..self.effects.len())
            .filter(|&effect| !masked(effect).is_empty())
            .map(|only| {
                let look = keeping(&|effect| effect == only || masked(effect).is_empty());
                (look, masked(only))
            })
            .collect();
        (plain, reaching)
    }

    /// Drops the empty mask lists at the end, which `masks_of_mut` may have added.
    pub fn tidy_effect_masks(&mut self) {
        self.effect_masks.truncate(self.effects.len());
        while self.effect_masks.last().is_some_and(Vec::is_empty) {
            self.effect_masks.pop();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MaskShape, VideoEffect};

    fn look_with_masked_second_effect() -> Look {
        let mut look = Look {
            effects: vec![VideoEffect::BlackAndWhite; 3],
            ..Look::default()
        };
        if let Some(masks) = look.masks_of_mut(MaskOwner::Effect(1)) {
            masks.push(Mask::new(MaskShape::Rectangle));
        }
        look
    }

    #[test]
    fn removing_an_effect_takes_its_masks_and_keeps_the_rest_beside_theirs() {
        let mut look = look_with_masked_second_effect();
        look.remove_effect(0);
        assert_eq!(look.masks_of(MaskOwner::Effect(0)).len(), 1);
        look.remove_effect(0);
        assert!(look.effect_masks.is_empty());
        assert!(!look.has_effect_masks());
    }

    #[test]
    fn appended_effects_bring_their_masks_to_their_new_places() {
        let mut look = Look::default();
        look.effects.push(VideoEffect::BlackAndWhite);
        look.append_effects(&look_with_masked_second_effect());
        assert_eq!(look.effects.len(), 4);
        assert_eq!(look.masks_of(MaskOwner::Effect(2)).len(), 1);
        assert!(look.masks_of(MaskOwner::Effect(1)).is_empty());
    }

    #[test]
    fn a_split_look_draws_each_masked_effect_over_the_unmasked_ones() {
        let look = look_with_masked_second_effect();
        let (plain, reaching) = look.split_by_effect_masks();
        assert_eq!(plain.effects.len(), 2);
        assert_eq!(reaching.len(), 1);
        assert_eq!(reaching[0].0.effects.len(), 3);
        assert_eq!(reaching[0].1.len(), 1);
    }
}
