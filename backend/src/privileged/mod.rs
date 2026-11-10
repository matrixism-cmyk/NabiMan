//! Linux-only, typed local service control and a short list of typed system
//! operations (firewall rules, package upgrades). No arbitrary program or shell protocol.
pub mod client;
mod connect;
mod executor;
pub mod identity;
pub mod policy;
pub mod protocol;
pub mod server;
mod store;
pub mod system_ops;
mod system_server;
pub mod audit;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod identity_tests;

#[cfg(test)]
mod system_tests;
