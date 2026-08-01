//! Dependency-free cooperative cancellation.
//!
//! A [`CancellationToken`] is a cheap handle that callers cancel from their
//! own control flow. Token-aware operations poll it at documented structural
//! checkpoints (frame/page boundaries, chunk boundaries, strip/tile loops)
//! and stop with [`ImageError::Cancelled`] without publishing partial state.
//! The crate remains single-threaded by design, so the token uses `Rc<Cell>`
//! and adds no synchronization overhead on native or WASM targets.

use std::cell::Cell;
use std::rc::Rc;

/// Cooperative cancellation handle for token-aware operations.
///
/// Clones share the same cancellation state. The token is neither `Send` nor
/// `Sync`, matching the crate's single-threaded execution model.
#[derive(Clone, Default)]
pub struct CancellationToken {
    cancelled: Rc<Cell<bool>>,
    #[cfg(coverage)]
    cancel_after: Rc<Cell<Option<usize>>>,
}

impl CancellationToken {
    /// Create an uncancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Cancel the token; every clone observes the cancellation.
    pub fn cancel(&self) {
        self.cancelled.set(true);
    }

    /// Return whether the token has been cancelled.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        #[cfg(coverage)]
        {
            if self.cancelled.get() {
                return true;
            }
            match self.cancel_after.get() {
                Some(0) => {
                    self.cancelled.set(true);
                    true
                }
                Some(remaining) => {
                    self.cancel_after.set(Some(remaining.saturating_sub(1)));
                    false
                }
                None => false,
            }
        }
        #[cfg(not(coverage))]
        {
            self.cancelled.get()
        }
    }

    /// Coverage-only hook: automatically cancel after `checks` more polls.
    ///
    /// This lets the coverage drills deterministically hit each structural
    /// checkpoint inside a single call. It is compiled out of production
    /// builds and has no effect on the public contract.
    #[cfg(coverage)]
    pub(crate) fn cancel_after(&self, checks: usize) {
        self.cancel_after.set(Some(checks));
    }
}
