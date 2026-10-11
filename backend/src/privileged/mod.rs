//! Linux-only, typed local service control. No arbitrary program or shell protocol.
pub mod client;
mod connect;
mod executor;
pub mod identity;
pub mod policy;
pub mod protocol;
pub mod server;
mod store;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod identity_tests;
