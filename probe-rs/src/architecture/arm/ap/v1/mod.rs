//! Access port v1 specific types and methods.

pub(crate) mod generic_ap;
pub(crate) mod memory_ap;

pub use generic_ap::IDR;
pub use memory_ap::{
    AddressIncrement, BaseaddrFormat, DataSize, BASE, BASE2, CFG, CSW, DRW, TAR, TAR2,
};

use crate::architecture::arm::{
    ap::{
        v1::generic_ap::ApClass, v1::generic_ap::ApType, AccessPort, ApAccess, GenericAp, MemoryAp,
    },
    ApAddress, DpAddress,
};

/// Determine if an AP exists with the given AP number.
///
/// The test is performed by reading the IDR register, and checking if the register is non-zero.
///
/// Can fail silently under the hood testing an ap that doesn't exist and would require cleanup.
pub fn access_port_is_valid<AP>(debug_port: &mut AP, access_port: GenericAp) -> bool
where
    AP: ApAccess,
{
    let idr_result: Result<IDR, _> = debug_port.read_ap_register(access_port);

    match idr_result {
        Ok(idr) => u32::from(idr) != 0,
        Err(e) => {
            tracing::debug!(
                "Error reading IDR register from AP {}: {}",
                access_port.ap_address().ap,
                e
            );
            false
        }
    }
}

/// Return a Vec of all valid access ports found that the target connected to the debug_probe.
/// Can fail silently under the hood testing an ap that doesn't exist and would require cleanup.
#[tracing::instrument(skip(debug_port))]
pub(crate) fn valid_access_ports<AP>(debug_port: &mut AP, dp: DpAddress) -> Vec<GenericAp>
where
    AP: ApAccess,
{
    (0..=255)
        .map(|ap| GenericAp::new(ApAddress { dp, ap }))
        .take_while(|port| access_port_is_valid(debug_port, *port))
        .collect()
}

/// Tries to find the first AP with the given idr value, returns `None` if there isn't any
pub fn get_ap_by_idr<AP, P>(debug_port: &mut AP, dp: DpAddress, f: P) -> Option<GenericAp>
where
    AP: ApAccess,
    P: Fn(IDR) -> bool,
{
    (0..=255)
        .map(|ap| GenericAp::new(ApAddress { dp, ap }))
        .find(|ap| {
            if let Ok(idr) = debug_port.read_ap_register(*ap) {
                f(idr)
            } else {
                false
            }
        })
}
