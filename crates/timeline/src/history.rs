use std::fmt::Debug;

use crate::validate;
use crate::{Sequence, TimelineError};

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

/// Owns the sequence so that every mutation goes through the undo stack.
#[derive(Debug)]
pub struct Timeline {
    sequence: Sequence,
    undo_stack: Vec<Box<dyn EditCommand>>,
    redo_stack: Vec<Box<dyn EditCommand>>,
    revision: u64,
}

impl Timeline {
    pub fn new(sequence: Sequence) -> Self {
        Self {
            sequence,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            revision: 0,
        }
    }

    pub fn sequence(&self) -> &Sequence {
        &self.sequence
    }

    /// Changes whenever the sequence does, so observers can tell when to refresh.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn apply(&mut self, edit: impl Edit) -> Result<(), TimelineError> {
        self.execute(Box::new(Snapshot { edit, before: None }))
    }

    pub fn execute(&mut self, mut command: Box<dyn EditCommand>) -> Result<(), TimelineError> {
        command.apply(&mut self.sequence)?;
        self.undo_stack.push(command);
        self.redo_stack.clear();
        self.revision += 1;
        Ok(())
    }

    pub fn undo_name(&self) -> Option<&'static str> {
        self.undo_stack.last().map(|command| command.name())
    }

    pub fn redo_name(&self) -> Option<&'static str> {
        self.redo_stack.last().map(|command| command.name())
    }

    /// Returns `false` when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some(mut command) = self.undo_stack.pop() else {
            return false;
        };
        command.revert(&mut self.sequence);
        self.redo_stack.push(command);
        self.revision += 1;
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
        self.revision += 1;
        Ok(true)
    }
}
