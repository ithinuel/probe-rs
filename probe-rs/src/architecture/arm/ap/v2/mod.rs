//! Access port v2 specific types and methods.

use crate::define_ap;

define_ap!(
    /// Memory AP
    ///
    /// The memory AP can be used to access a memory-mapped
    /// set of debug resources of the attached system.
    MemoryAp
);
