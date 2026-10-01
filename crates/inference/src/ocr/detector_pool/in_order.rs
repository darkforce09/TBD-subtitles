//! Screening results put back in the order of their sequence numbers.
//!
//! **Role:** hold the results that finish ahead of an earlier job until that job's result
//! arrives, and release them in sequence order.
//!
//! **Position:** for the detection scan, which applies results strictly in sample order while the
//! pool's sessions finish in any order.
//!
//! **Signals and state:** the next sequence number to release and the results waiting for it.
//!
//! **Invariants:** results come out in consecutive sequence order, each exactly once; a result
//! for a number already released, or one already waiting, is refused.

use std::collections::BTreeMap;

use crate::ocr::OcrError;
use crate::ocr::pool::ScreenResult;

/// A reorder buffer over screening results numbered from `first` up, one number per job.
#[derive(Debug)]
pub struct InOrder {
    next: u64,
    waiting: BTreeMap<u64, ScreenResult>,
}

impl InOrder {
    pub fn new(first: u64) -> InOrder {
        InOrder {
            next: first,
            waiting: BTreeMap::new(),
        }
    }

    /// Hold `result` until every earlier number has been released.
    pub fn push(&mut self, result: ScreenResult) -> Result<(), OcrError> {
        if result.seq < self.next || self.waiting.contains_key(&result.seq) {
            return Err(format!("screening result {} arrived twice", result.seq).into());
        }
        self.waiting.insert(result.seq, result);
        Ok(())
    }

    /// The next result in sequence order, once it has arrived.
    pub fn pop(&mut self) -> Option<ScreenResult> {
        let result = self.waiting.remove(&self.next)?;
        self.next += 1;
        Some(result)
    }

    /// Results that arrived ahead of the next number.
    pub fn waiting(&self) -> usize {
        self.waiting.len()
    }
}

#[cfg(test)]
#[path = "tests/in_order.rs"]
mod tests;
