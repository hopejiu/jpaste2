//! Clipboard monitoring: the capture → toast pipeline (`capture`) and the
//! global keyboard hook for queue-paste mode (`hook`).
//!
//! Both concern the "what happened on the clipboard" event pipeline, so they
//! live together here rather than as top-level modules.

pub mod capture;
pub mod hook;
