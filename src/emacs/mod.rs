mod client;

pub use client::{EmacsClient, EmacsClientTrait};

#[cfg(test)]
pub use client::MockEmacsClientTrait;
