//! u64 fuel lease; decrement per conformance step.

pub struct FuelLease {
    remaining: u64,
}

impl FuelLease {
    pub fn new(amount: u64) -> Self {
        Self { remaining: amount }
    }

    /// Charge one step. Returns false when exhausted (and pins at 0).
    pub fn charge(&mut self, units: u64) -> bool {
        if self.remaining < units {
            self.remaining = 0;
            return false;
        }
        self.remaining -= units;
        true
    }

    pub fn remaining(&self) -> u64 {
        self.remaining
    }
}
