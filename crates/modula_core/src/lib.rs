//! The engine's shared foundations: its domain error, paths, the repository
//! set, and small helpers. Depends only on `modula-db` and `modula-platform`,
//! so every layer above can take these without a cycle.

pub mod error;
pub mod paths;
pub mod repositories;
pub mod slug;
pub mod validation;
