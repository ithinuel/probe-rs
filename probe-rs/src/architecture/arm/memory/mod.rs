//! Types and functions for interacting with target memory.

pub(crate) mod adi_v5_memory_interface;
pub(crate) mod adi_v6_memory_interface;
pub(crate) mod romtable;

use super::ap::AccessPortError;
pub use romtable::{Component, ComponentClass, ComponentId, CoresightComponent, PeripheralType};
