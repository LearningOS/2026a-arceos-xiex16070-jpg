//! Collection types.
//!
//! This module provides the collection types of the `alloc` crate, plus a
//! [`HashMap`] implementation that works without the standard library.

pub use alloc::collections::*;

mod hashmap;

pub use self::hashmap::{HashMap, DefaultHashBuilder};
