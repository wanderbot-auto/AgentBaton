//! Bounded remote-host foundation. This crate is intentionally outside the root workspace.
#![cfg(unix)]
pub mod controller;
pub mod protocol;
pub mod store;
pub mod transport;
pub mod worker;
