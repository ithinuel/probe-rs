//! Sequences for the nRF91.

use std::sync::Arc;

use super::nrf::Nrf;
use crate::architecture::arm::ap::AccessPort;
use crate::architecture::arm::sequences::ArmDebugSequence;
use crate::architecture::arm::ArmError;
use crate::architecture::arm::ArmProbe;
use crate::architecture::arm::{
    communication_interface::Initialized, ApAddress, ArmCommunicationInterface, DapAccess,
};

/// The sequence handle for the nRF9160.
#[derive(Debug)]
pub struct Nrf9160(());

impl Nrf9160 {
    /// Create a new sequence handle for the nRF9160.
    pub fn create() -> Arc<dyn ArmDebugSequence> {
        Arc::new(Self(()))
    }
}

impl Nrf for Nrf9160 {
    fn core_aps(&self, memory: &mut dyn ArmProbe) -> Vec<(ApAddress, ApAddress)> {
        let ap_address = memory.ap().ap_address();
        vec![(
            ApAddress::apv1_with_dp(ap_address.dp, 0),
            ApAddress::apv1_with_dp(ap_address.dp, 4),
        )]
    }

    fn is_core_unlocked(
        &self,
        arm_interface: &mut ArmCommunicationInterface<Initialized>,
        _ahb_ap_address: ApAddress,
        ctrl_ap_address: ApAddress,
    ) -> Result<bool, ArmError> {
        let approtect_status = arm_interface.read_raw_ap_register(ctrl_ap_address, 0x00C)?;
        Ok(approtect_status != 0)
    }

    fn has_network_core(&self) -> bool {
        false
    }
}
