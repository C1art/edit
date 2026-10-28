use std::fmt::Debug;

use crate::{Sequence, TimelineError};

/// A reversible timeline mutation. `apply` must leave the sequence unchanged when it
/// fails, and `revert` is only called after a successful `apply`.
pub trait EditCommand: Debug + Send {
    fn apply(&mut self, sequence: &mut Sequence) -> Result<(), TimelineError>;
    fn revert(&mut self, sequence: &mut Sequence);
}

/// Owns the sequence so that every mutation goes through the undo stack.
#[derive(Debug)]
pub struct Timeline {
    sequence: Sequence,
    undo_stack: Vec<Box<dyn EditCommand>>,
    redo_stack: Vec<Box<dyn EditCommand>>,
}

impl Timeline {
    pub fn new(sequence: Sequence) -> Self {
        Self {
            sequence,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    pub fn sequence(&self) -> &Sequence {
        &self.sequence
    }

    pub fn execute(&mut self, mut command: Box<dyn EditCommand>) -> Result<(), TimelineError> {
        command.apply(&mut self.sequence)?;
        self.undo_stack.push(command);
        self.redo_stack.clear();
        Ok(())
    }

    /// Returns `false` when there is nothing to undo.
    pub fn undo(&mut self) -> bool {
        let Some(mut command) = self.undo_stack.pop() else {
            return false;
        };
        command.revert(&mut self.sequence);
        self.redo_stack.push(command);
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
        Ok(true)
    }
}
