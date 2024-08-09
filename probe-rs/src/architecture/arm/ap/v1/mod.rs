//! Access port v1 specific types and methods.

mod generic_ap;
mod memory_ap;

pub use generic_ap::{ApClass, IDR};
pub(crate) use memory_ap::mock;
pub use memory_ap::{
    AddressIncrement, BaseaddrFormat, DataSize, BASE, BASE2, CFG, CSW, DRW, TAR, TAR2,
};

use crate::architecture::arm::{
    ap::{AccessPort, ApAccess, GenericAp, MemoryAp},
    ApAddress, ApInformation, ApPort, ArmError, DpAddress, MemoryApInformation,
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
        Ok(idr) => {
            let is_valid = u32::from(idr) != 0;

            if !is_valid {
                tracing::debug!("AP {:?} is not valid, IDR = 0", access_port.ap_address().ap);
            }
            is_valid
        }
        Err(e) => {
            tracing::debug!(
                "Error reading IDR register from AP {:?}: {}",
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
        .map(|ap| {
            GenericAp::new(ApAddress {
                dp,
                ap: ApPort::Index(ap),
            })
        })
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
        .map(|ap| {
            GenericAp::new(ApAddress {
                dp,
                ap: ApPort::Index(ap),
            })
        })
        .find(|ap| {
            if let Ok(idr) = debug_port.read_ap_register(*ap) {
                f(idr)
            } else {
                false
            }
        })
}

pub(crate) fn read_ap_information<P>(
    probe: &mut P,
    access_port: GenericAp,
) -> Result<ApInformation, ArmError>
where
    P: ApAccess,
{
    let idr: IDR = probe.read_ap_register(access_port)?;

    if idr.CLASS == ApClass::MemAp {
        let access_port: MemoryAp = access_port.into();

        let base_register: BASE = probe.read_ap_register(access_port)?;

        let mut base_address = if BaseaddrFormat::ADIv5 == base_register.Format {
            let base2: BASE2 = probe.read_ap_register(access_port)?;

            u64::from(base2.BASEADDR) << 32
        } else {
            0
        };
        base_address |= u64::from(base_register.BASEADDR << 12);

        // Save old CSW value. STLink firmare caches it, which breaks things
        // if we change it behind its back.
        let old_csw: CSW = probe.read_ap_register(access_port)?;

        // Read information about HNONSEC support and supported access widths
        let csw = CSW::new(DataSize::U8);

        probe.write_ap_register(access_port, csw)?;
        let csw: CSW = probe.read_ap_register(access_port)?;

        probe.write_ap_register(access_port, old_csw)?;

        let only_32bit_data_size = csw.SIZE != DataSize::U8;

        let supports_hnonsec = csw.HNONSEC == 1;

        tracing::debug!("HNONSEC supported: {}", supports_hnonsec);

        let device_enabled = csw.DeviceEn == 1;

        tracing::debug!("Device enabled: {}", device_enabled);

        let cfg: CFG = probe.read_ap_register(access_port)?;

        let has_large_address_extension = cfg.LA == 1;
        let has_large_data_extension = cfg.LD == 1;

        Ok(ApInformation::MemoryAp(MemoryApInformation {
            address: access_port.ap_address(),
            supports_only_32bit_data_size: only_32bit_data_size,
            debug_base_address: base_address,
            supports_hnonsec,
            has_large_address_extension,
            has_large_data_extension,
            device_enabled,
        }))
    } else {
        Ok(ApInformation::Other {
            address: access_port.ap_address(),
            idr: super::ApIDR::APv1(idr),
        })
    }
}
