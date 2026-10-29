use std::fmt::Debug;

use crate::validate;
use crate::{InNest, Sequence, SourceId, TimelineError};

/// A reversible timeline mutation. `apply` must leave the sequence unchanged when it
/// fails, and `revert` is only called after a successful `apply`.
pub trait EditCommand: Debug + Send {
    /// Shown in the Edit menu, as in "Undo Ripple Trim".
    fn name(&self) -> &'static str;
    fn apply(&mut self, sequence: &mut Sequence) -> Result<(), TimelineError>;
    fn revert(&mut self, sequence: &mut Sequence);
}

/// An edit written as a transformation of the sequence. Wrapped in [`Timeline::apply`],
/// it runs on a copy that must pass validation before replacing the original, and undo
/// restores the copy taken before it, so apply and revert are exact inverses.
pub trait Edit: Debug + Send + 'static {
    fn name(&self) -> &'static str;
    fn edit(&self, sequence: &mut Sequence) -> Result<(), TimelineError>;
}

/// Runs `edit` on a copy of `sequence` and returns the validated result, for previews
/// and for probing which variants of an edit are legal.
pub fn preview(sequence: &Sequence, edit: &impl Edit) -> Result<Sequence, TimelineError> {
    let mut working = sequence.clone();
    edit.edit(&mut working)?;
    validate::check(sequence, &working)?;
    Ok(working)
}

#[derive(Debug)]
struct Snapshot<E> {
    edit: E,
    before: Option<Sequence>,
}

impl<E: Edit> EditCommand for Snapshot<E> {
    fn name(&self) -> &'static str {
        self.edit.name()
    }

    fn apply(&mut self, sequence: &mut Sequence) -> Result<(), TimelineError> {
        let edited = preview(sequence, &self.edit)?;
        self.before = Some(std::mem::replace(sequence, edited));
        Ok(())
    }

    fn revert(&mut self, sequence: &mut Sequence) {
        if let Some(before) = self.before.take() {
            *sequence = before;
        }
    }
}

/// Owns the sequence so that every mutation goes through the undo stack. One of its
/// nested sequences can be open, which then stands in for the sequence: it is what
/// [`Timeline::sequence`] shows and what edits change.
#[derive(Debug)]
pub struct Timeline {
    sequence: Sequence,
    open: Option<(SourceId, Sequence)>,
    undo_stack: Vec<Box<dyn EditCommand>>,
    redo_stack: Vec<Box<dyn EditCommand>>,
    revision: u64,
}

impl Timeline {
    pub fn new(sequence: Sequence) -> Self {
        Self {
            sequence,
            open: None,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            revision: 0,
        }
    }

    /// The open nest, or the project's sequence when none is.
    pub fn sequence(&self) -> &Sequence {
        self.open.as_ref().map_or(&self.sequence, |(_, view)| view)
    }

    /// The project's own sequence, with every nest inside it.
    pub fn project(&self) -> &Sequence {
        &self.sequence
    }

    pub fn open_nest(&self) -> Option<SourceId> {
        self.open.as_ref().map(|(id, _)| *id)
    }

    /// Opens nest `nest`, or returns to the project's sequence with `None`. Returns
    /// whether that changed what is open.
    pub fn set_open_nest(&mut self, nest: Option<SourceId>) -> bool {
        if nest == self.open_nest() {
            return false;
        }
        let view = nest.and_then(|id| Some((id, self.sequence.nest_view(id)?)));
        if nest.is_some() && view.is_none() {
            return false;
        }
        self.open = view;
        self.revision += 1;
        true
    }

    /// Rebuilds the open nest's view after the sequence changed, closing it if it's gone.
    fn changed(&mut self) {
        self.revision += 1;
        if let Some((id, view)) = &mut self.open {
            match self.sequence.nest_view(*id) {
                Some(fresh) => *view = fresh,
                None => self.open = None,
            }
        }
    }

    /// Changes whenever the sequence does, so observers can tell when to refresh.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn apply(&mut self, edit: impl Edit) -> Result<(), TimelineError> {
        match self.open_nest() {
            Some(nest) => self.execute(Box::new(Snapshot {
                edit: InNest { nest, edit },
                before: None,
            })),
            None => self.execute(Box::new(Snapshot { edit, before: None })),
        }
    }

    /// Applies `edit` to the project's sequence even while a nest is open.
    pub fn apply_to_project(&mut self, edit: impl Edit) -> Result<(), TimelineError> {
        self.execute(Box::new(Snapshot { edit, before: None }))
    }

    pub fn execute(&mut self, mut command: Box<dyn EditCommand>) -> Result<(), TimelineError> {
        command.apply(&mut self.sequence)?;
        self.undo_stack.push(command);
        self.redo_stack.clear();
        self.changed();
        Ok(())
    }

    pub fn undo_name(&self) -> Option<&'static str> {
        self.undo_stack.last().map(|command| command.name())
    }

    pub fn redo_name(&self) -> Option<&'static str> {
        self.redo_stack.last().map(|command| command.name())
    }

    /// The names of the edits in effect, oldest first.
    pub fn done(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.undo_stack.iter().map(|command| command.name())
    }

    /// The names of the edits undone, in the order redo would bring them back.
    pub fn undone(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.redo_stack.iter().rev().map(|command| command.name())
    }

    /// Undoes or redoes until the first `step` edits are in effect, as a history list
    /// jumps to a state. A redo that fails stops there.
    pub fn go_to(&mut self, step: usize) -> Result<(), TimelineError> {
        while self.undo_stack.len() > step && self.undo() {}
        while self.undo_stack.len() < step && self.redo()? {}
        Ok(())
    }

    /// Returns `false` when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some(mut command) = self.undo_stack.pop() else {
            return false;
        };
        command.revert(&mut self.sequence);
        self.redo_stack.push(command);
        self.changed();
        true
    }

    /// Returns `Ok(false)` when there is nothing to redo.
    pub fn redo(&mut self) -> Result<bool, TimelineError> {
        let Some(mut command) = self.redo_stack.pop() else {
            return Ok(false);
        };
        if let Err(error) = command.apply(&mut self.sequence) {
            self.redo_stack.push(command);
            return Err(error);
        }
        self.undo_stack.push(command);
        self.changed();
        Ok(true)
    }
}
