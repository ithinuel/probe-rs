//! Memory access port

pub(crate) mod mock;
pub mod registers;

mod amba_ahb3;
mod amba_apb2_apb3;
mod amba_apb4_apb5;

mod amba_ahb5;
mod amba_ahb5_hprot;

mod amba_axi3_axi4;
mod amba_axi5;

pub use registers::DataSize;
use registers::{AddressIncrement, BASE, BASE2, DRW, TAR, TAR2};

use super::{
    v1::{AccessPortType, ApRegAccess},
    ApAccessT, ApRegAddressT, ApRegisterAccessT, RegisterT,
};
use crate::architecture::arm::{ArmError, DapAccess, FullyQualifiedApAddress};

/// Implements all default registers of a memory AP to the given type.
///
/// Invoke in the form `attached_regs_to_mem_ap!(mod_name => ApName)` where:
/// - `mod_name` is a module name in which the impl an the required use will be expanded to.
/// - `ApName` a type name that must be available in the current scope to which the registers will
///   be attached.
#[macro_export]
macro_rules! attached_regs_to_mem_ap {
    ($mod_name:ident => $name:ident) => {
        mod $mod_name {
            use super::$name;
            use $crate::architecture::arm::ap::{
                memory::registers::{
                    BASE, BASE2, BD0, BD1, BD2, BD3, CFG, CSW, DRW, MBT, TAR, TAR2,
                },
                v1::{ApRegAccess, RegAddr as RegAddrV1},
                v2::RegAddr as RegAddrV2,
                ApRegisterAccessT,
            };
            impl ApRegAccess<CFG> for $name {}
            impl ApRegAccess<CSW> for $name {}
            impl ApRegAccess<BASE> for $name {}
            impl ApRegAccess<BASE2> for $name {}
            impl ApRegAccess<TAR> for $name {}
            impl ApRegAccess<TAR2> for $name {}
            impl ApRegAccess<BD2> for $name {}
            impl ApRegAccess<BD3> for $name {}
            impl ApRegAccess<DRW> for $name {}
            impl ApRegAccess<MBT> for $name {}
            impl ApRegAccess<BD1> for $name {}
            impl ApRegAccess<BD0> for $name {}

            impl ApRegisterAccessT<CFG, RegAddrV1> for $name {}
            impl ApRegisterAccessT<CSW, RegAddrV1> for $name {}
            impl ApRegisterAccessT<BASE, RegAddrV1> for $name {}
            impl ApRegisterAccessT<BASE2, RegAddrV1> for $name {}
            impl ApRegisterAccessT<TAR, RegAddrV1> for $name {}
            impl ApRegisterAccessT<TAR2, RegAddrV1> for $name {}
            impl ApRegisterAccessT<BD2, RegAddrV1> for $name {}
            impl ApRegisterAccessT<BD3, RegAddrV1> for $name {}
            impl ApRegisterAccessT<DRW, RegAddrV1> for $name {}
            impl ApRegisterAccessT<MBT, RegAddrV1> for $name {}
            impl ApRegisterAccessT<BD1, RegAddrV1> for $name {}
            impl ApRegisterAccessT<BD0, RegAddrV1> for $name {}

            impl ApRegisterAccessT<CFG, RegAddrV2> for $name {}
            impl ApRegisterAccessT<CSW, RegAddrV2> for $name {}
            impl ApRegisterAccessT<BASE, RegAddrV2> for $name {}
            impl ApRegisterAccessT<BASE2, RegAddrV2> for $name {}
            impl ApRegisterAccessT<TAR, RegAddrV2> for $name {}
            impl ApRegisterAccessT<TAR2, RegAddrV2> for $name {}
            impl ApRegisterAccessT<BD2, RegAddrV2> for $name {}
            impl ApRegisterAccessT<BD3, RegAddrV2> for $name {}
            impl ApRegisterAccessT<DRW, RegAddrV2> for $name {}
            impl ApRegisterAccessT<MBT, RegAddrV2> for $name {}
            impl ApRegisterAccessT<BD1, RegAddrV2> for $name {}
            impl ApRegisterAccessT<BD0, RegAddrV2> for $name {}
        }
    };
}

macro_rules! memory_aps {
    ($($variant:ident => $type:path),*) => {
        #[derive(Debug)]
        pub enum MemoryAp {
            $($variant($type)),*
        }

        $(impl From<$type> for MemoryAp {
            fn from(value: $type) -> Self {
                Self::$variant(value)
            }
        })*

        impl MemoryAp {
            pub fn new<I>(
                interface: &mut I,
                address: &FullyQualifiedApAddress,
            ) -> Result<Self, ArmError> where I: DapAccess {
                let idr: super::IDR = interface.read_raw_ap_register(
                    address,
                    u16::from(<super::IDR as super::RegisterT<super::v1::RegAddr>>::ADDRESS) as u8
                )?.try_into()?;
                tracing::debug!("reading IDR: {:x?}", idr);
                use $crate::architecture::arm::ap::ApType;
                Ok(match idr.TYPE {
                    ApType::JtagComAp => return Err(ArmError::WrongApType),
                    $(ApType::$variant => <$type>::new(interface, address.clone())?.into(),)*
                })
            }
        }
    }
}

memory_aps! {
    AmbaAhb3 => amba_ahb3::AmbaAhb3,
    AmbaAhb5 => amba_ahb5::AmbaAhb5,
    AmbaAhb5Hprot => amba_ahb5_hprot::AmbaAhb5Hprot,
    AmbaApb2Apb3 => amba_apb2_apb3::AmbaApb2Apb3,
    AmbaApb4Apb5 => amba_apb4_apb5::AmbaApb4Apb5,
    AmbaAxi3Axi4 => amba_axi3_axi4::AmbaAxi3Axi4,
    AmbaAxi5 => amba_axi5::AmbaAxi5
}

impl ApRegAccess<super::IDR> for MemoryAp {}
attached_regs_to_mem_ap!(memory_ap_regs => MemoryAp);

macro_rules! mem_ap_forward {
    (bounds: $b:tt) => {
        amba_ahb3::AmbaAhb3: $b,
        amba_ahb5::AmbaAhb5: $b,
        amba_ahb5_hprot::AmbaAhb5Hprot: $b,
        amba_apb2_apb3::AmbaApb2Apb3: $b,
        amba_apb4_apb5::AmbaApb4Apb5: $b,
        amba_axi3_axi4::AmbaAxi3Axi4: $b,
        amba_axi5::AmbaAxi5: $b
    };
    ($me:ident, $trait:path, $name:ident($($arg:ident),*)) => {
        match $me {
            MemoryAp::AmbaApb2Apb3(ap) => { use $trait as T; T::$name(ap, $($arg),*) },
            MemoryAp::AmbaApb4Apb5(ap) => { use $trait as T; T::$name(ap, $($arg),*) },
            MemoryAp::AmbaAhb3(m) => { use $trait as T; T::$name(m, $($arg),*) },
            MemoryAp::AmbaAhb5(m) => { use $trait as T; T::$name(m, $($arg),*) },
            MemoryAp::AmbaAhb5Hprot(m) => { use $trait as T; T::$name(m, $($arg),*) },
            MemoryAp::AmbaAxi3Axi4(m) => { use $trait as T; T::$name(m, $($arg),*) },
            MemoryAp::AmbaAxi5(m) => { use $trait as T; T::$name(m, $($arg),*) },
        }
    };
    ($me:ident, $name:ident($($arg:ident),*)) => {
        match $me {
            MemoryAp::AmbaApb2Apb3(ap) => ap.$name($($arg),*),
            MemoryAp::AmbaApb4Apb5(ap) => ap.$name($($arg),*),
            MemoryAp::AmbaAhb3(m) => m.$name($($arg),*),
            MemoryAp::AmbaAhb5(m) => m.$name($($arg),*),
            MemoryAp::AmbaAhb5Hprot(m) => m.$name($($arg),*),
            MemoryAp::AmbaAxi3Axi4(m) => m.$name($($arg),*),
            MemoryAp::AmbaAxi5(m) => m.$name($($arg),*),
        }
    };
}
/// Trait for getting this memory AP’s base component.
///
/// This component may be a debug component or a ROM table listing the components available in this
/// MemoryAP.
pub(crate) trait MemoryApBaseT<A: ApRegAddressT> {
    /// The address of the base component.
    fn base_address<I: ApAccessT<A>>(&self, interface: &mut I) -> Result<u64, ArmError>;
}

fn base_address<A, T, I>(me: &T, interface: &mut I) -> Result<u64, ArmError>
where
    T: ApRegisterAccessT<BASE, A> + ApRegisterAccessT<BASE2, A>,
    BASE: RegisterT<A>,
    BASE2: RegisterT<A>,
    A: ApRegAddressT,
    I: ApAccessT<A>,
{
    let base_register: BASE = interface.read_register(me)?;
    if u32::from(base_register) == 0xFFFF_FFFF {
        todo!("Legacy format; No debug entries.");
    }

    let address_hi = match base_register.Format {
        registers::BaseAddrFormat::Legacy => 0,
        registers::BaseAddrFormat::ADIv5 if !base_register.present => {
            todo!("ADIv5/ADIv6 Base Register format; No debug entry present.")
        }
        registers::BaseAddrFormat::ADIv5 => {
            let base2: BASE2 = interface.read_register(me)?;

            u64::from(base2.BASEADDR) << 32
        }
    };
    let address_lo = u64::from(base_register.BASEADDR << 12);

    Ok(address_hi | address_lo)
}

/// A trait to implement on memory APs and inform on their extensions.
pub(crate) trait MemApExtensionsT {
    /// Does this Memory AP supports large address extension (64bits)?
    fn has_large_address_extension(&self) -> bool;
    /// Does this Memory Ap supports large data extension?
    fn has_large_data_extension(&self) -> bool;
    /// Does this Memory AP supports packed transfers?
    fn supports_packed_transfers(&self) -> bool;
}

pub(crate) trait MemoryApDataSizeAndIncrementT<A: ApRegAddressT> {
    /// Attempts to set the requested data size.
    ///
    /// The operation may fail if the requested data size is not supported by the Memory Access
    /// Port.
    fn try_set_datasize_and_incr<I: ApAccessT<A>>(
        &mut self,
        interface: &mut I,
        data_size: DataSize,
        increment: AddressIncrement,
    ) -> Result<(), ArmError>;
}
impl<A: ApRegAddressT> MemoryApDataSizeAndIncrementT<A> for MemoryAp {
    fn try_set_datasize_and_incr<I: ApAccessT<A>>(
        &mut self,
        interface: &mut I,
        data_size: DataSize,
        increment: AddressIncrement,
    ) -> Result<(), ArmError> {
        mem_ap_forward!(
            self,
            try_set_datasize_and_incr(interface, data_size, increment)
        )
    }
}

pub(crate) trait MemoryApT<A: ApRegAddressT>
where
    A: ApRegAddressT,
    TAR: RegisterT<A>,
    TAR2: RegisterT<A>,
    DRW: RegisterT<A>,
    Self: MemApExtensionsT
        + ApRegisterAccessT<TAR, A>
        + ApRegisterAccessT<TAR2, A>
        + ApRegisterAccessT<DRW, A>,
{
    fn set_target_address<I: ApAccessT<A>>(
        &mut self,
        interface: &mut I,
        address: u64,
    ) -> Result<(), ArmError> {
        let address_lower = address as u32;
        let address_upper = (address >> 32) as u32;

        if self.has_large_address_extension() {
            interface.write_register(
                self,
                TAR2 {
                    address: address_upper,
                },
            )?;
        } else if address_upper != 0 {
            return Err(ArmError::OutOfBounds);
        }

        interface.write_register(
            self,
            TAR {
                address: address_lower,
            },
        )?;

        Ok(())
    }

    /// Read multiple 32 bit values from the DRW register on the given AP.
    fn read_data<I: ApAccessT<A>>(
        &mut self,
        interface: &mut I,
        values: &mut [u32],
    ) -> Result<(), ArmError> {
        for value in values.iter_mut() {
            *value = interface.read_register::<DRW, _>(self)?.data;
        }
        Ok(())
    }

    /// Write multiple 32 bit values to the DRW register on the given AP.
    fn write_data<I: ApAccessT<A>>(
        &mut self,
        interface: &mut I,
        values: &[u32],
    ) -> Result<(), ArmError> {
        for data in values.iter().cloned() {
            interface.write_register(self, DRW { data })?;
        }
        Ok(())
    }
}

// =========================================== old traits =========================================

impl AccessPortType for MemoryAp {
    fn ap_address(&self) -> &crate::architecture::arm::FullyQualifiedApAddress {
        mem_ap_forward!(self, ap_address())
    }
}

impl super::v1::MemoryApType for MemoryAp {
    type CSW = registers::CSW;

    fn has_large_address_extension(&self) -> bool {
        mem_ap_forward!(self, super::v1::MemoryApType, has_large_address_extension())
    }

    fn has_large_data_extension(&self) -> bool {
        mem_ap_forward!(self, super::v1::MemoryApType, has_large_data_extension())
    }

    fn supports_only_32bit_data_size(&self) -> bool {
        mem_ap_forward!(self, supports_only_32bit_data_size())
    }

    fn try_set_datasize<I: super::v1::ApAccess>(
        &mut self,
        interface: &mut I,
        data_size: DataSize,
    ) -> Result<(), ArmError> {
        mem_ap_forward!(self, try_set_datasize(interface, data_size))
    }

    fn status<I: super::v1::ApAccess>(&mut self, interface: &mut I) -> Result<Self::CSW, ArmError> {
        mem_ap_forward!(self, generic_status(interface))
    }
}
