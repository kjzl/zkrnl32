//! The `zkrnl32` prelude.
//!
//! This module provides the core types and traits that are universally useful
//! across the kernel. It is intended to be glob-imported (`use crate::prelude::*;`)
//! in most other modules.

// Example of the Rust for Linux (RfL) vertical formatting rule using trailing comments:
// use core::marker::PhantomData; //
// use core::ptr::NonNull; //

/// A placeholder for the prelude module initialization.
pub struct PreludeInit;
