//! Shared typed count entry used by read-only redundancy censuses.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RedundancyCount<T> {
    pub(super) key: T,
    pub(super) sites: u64,
}

impl<T: Copy> RedundancyCount<T> {
    pub const fn key(self) -> T {
        self.key
    }

    pub const fn sites(self) -> u64 {
        self.sites
    }
}

impl<T> RedundancyCount<T> {
    pub(super) const fn new(key: T, sites: u64) -> Self {
        Self { key, sites }
    }
}
