//! Types and functions for interacting with access ports.

use super::{
    communication_interface::RegisterParseError, ApAddress, ArmError, DapAccess, Register,
};
use crate::architecture::arm::dp::DebugPortError;
use crate::probe::DebugProbeError;

pub mod register_generation;

pub mod v1;
pub mod v2;

crate::define_ap!(
    /// A generic access port which implements just the register every access port has to implement
    /// to be compliant with the ADI 5.2 specification.
    GenericAp
);

crate::define_ap!(
    /// Memory AP
    ///
    /// The memory AP can be used to access a memory-mapped
    /// set of debug resources of the attached system.
    MemoryAp
);

impl MemoryAp {
    /// The base address of this AP which is used to then access all relative control registers.
    pub fn base_address<A>(&self, interface: &mut A) -> Result<u64, ArmError>
    where
        A: ApAccess,
    {
        let base_register: v1::BASE = interface.read_ap_register(*self)?;

        let mut base_address = if v1::memory_ap::BaseAddrFormat::ADIv5 == base_register.Format {
            let base2: v1::BASE2 = interface.read_ap_register(*self)?;

            u64::from(base2.BASEADDR) << 32
        } else {
            0
        };
        base_address |= u64::from(base_register.BASEADDR << 12);

        Ok(base_address)
    }
}

impl From<GenericAp> for MemoryAp {
    fn from(other: GenericAp) -> Self {
        MemoryAp {
            address: other.ap_address(),
        }
    }
}

/// Some error during AP handling occurred.
#[derive(Debug, thiserror::Error)]
pub enum AccessPortError {
    /// An error occurred when trying to read a register.
    #[error("Failed to read register {name} at address 0x{address:08x}")]
    RegisterRead {
        /// The address of the register.
        address: u16,
        /// The name if the register.
        name: &'static str,
        /// The underlying root error of this access error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// An error occurred when trying to write a register.
    #[error("Failed to write register {name} at address 0x{address:08x}")]
    RegisterWrite {
        /// The address of the register.
        address: u16,
        /// The name if the register.
        name: &'static str,
        /// The underlying root error of this access error.
        #[source]
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    /// Some error with the operation of the APs DP occurred.
    #[error("Error while communicating with debug port")]
    DebugPort(#[from] DebugPortError),
    /// An error occurred when trying to flush batched writes of to the AP.
    #[error("Failed to flush batched writes")]
    Flush(#[from] DebugProbeError),

    /// Error while parsing a register
    #[error("Error parsing a register")]
    RegisterParse(#[from] RegisterParseError),
}

impl AccessPortError {
    /// Constructs a [`AccessPortError::RegisterRead`] from just the source error and the register type.
    pub fn register_read_error<R: Register, E: std::error::Error + Send + Sync + 'static>(
        source: E,
    ) -> Self {
        AccessPortError::RegisterRead {
            address: R::ADDRESS,
            name: R::NAME,
            source: Box::new(source),
        }
    }

    /// Constructs a [`AccessPortError::RegisterWrite`] from just the source error and the register type.
    pub fn register_write_error<R: Register, E: std::error::Error + Send + Sync + 'static>(
        source: E,
    ) -> Self {
        AccessPortError::RegisterWrite {
            address: R::ADDRESS,
            name: R::NAME,
            source: Box::new(source),
        }
    }
}

/// A trait to be implemented by access port register types.
///
/// Use the [`define_ap_register!`] macro to implement this.
pub trait ApRegister<PORT: AccessPort>: Register + Sized {}

/// A trait to be implemented on access port types.
///
/// Use the [`define_ap!`] macro to implement this.
pub trait AccessPort: Clone {
    /// Returns the address of the access port.
    fn ap_address(&self) -> ApAddress;
}

/// A trait to be implemented by access port drivers to implement access port operations.
pub trait ApAccess {
    /// Read a register of the access port.
    fn read_ap_register<PORT, R>(&mut self, port: PORT) -> Result<R, ArmError>
    where
        PORT: AccessPort,
        R: ApRegister<PORT>;

    /// Read a register of the access port using a block transfer.
    /// This can be used to read multiple values from the same register.
    fn read_ap_register_repeated<PORT, R>(
        &mut self,
        port: impl Into<PORT> + Clone,
        register: R,
        values: &mut [u32],
    ) -> Result<(), ArmError>
    where
        PORT: AccessPort,
        R: ApRegister<PORT>;

    /// Write a register of the access port.
    fn write_ap_register<PORT, R>(
        &mut self,
        port: impl Into<PORT>,
        register: R,
    ) -> Result<(), ArmError>
    where
        PORT: AccessPort,
        R: ApRegister<PORT>;

    /// Write a register of the access port using a block transfer.
    /// This can be used to write multiple values to the same register.
    fn write_ap_register_repeated<PORT, R>(
        &mut self,
        port: impl Into<PORT> + Clone,
        register: R,
        values: &[u32],
    ) -> Result<(), ArmError>
    where
        PORT: AccessPort,
        R: ApRegister<PORT>;
}

impl<T: DapAccess> ApAccess for T {
    #[tracing::instrument(skip(self, port), fields(ap = port.ap_address().ap, register = R::NAME, value))]
    fn read_ap_register<PORT, R>(&mut self, port: PORT) -> Result<R, ArmError>
    where
        PORT: AccessPort,
        R: ApRegister<PORT>,
    {
        let raw_value = self.read_raw_ap_register(port.ap_address(), R::ADDRESS)?;

        tracing::Span::current().record("value", raw_value);

        tracing::debug!("Register read succesful");

        Ok(raw_value.try_into()?)
    }

    fn write_ap_register<PORT, R>(
        &mut self,
        port: impl Into<PORT>,
        register: R,
    ) -> Result<(), ArmError>
    where
        PORT: AccessPort,
        R: ApRegister<PORT>,
    {
        tracing::debug!("Writing register {}, value={:x?}", R::NAME, register);
        self.write_raw_ap_register(port.into().ap_address(), R::ADDRESS, register.into())
    }

    fn write_ap_register_repeated<PORT, R>(
        &mut self,
        port: impl Into<PORT>,
        _register: R,
        values: &[u32],
    ) -> Result<(), ArmError>
    where
        PORT: AccessPort,
        R: ApRegister<PORT>,
    {
        tracing::debug!(
            "Writing register {}, block with len={} words",
            R::NAME,
            values.len(),
        );
        self.write_raw_ap_register_repeated(port.into().ap_address(), R::ADDRESS, values)
    }

    fn read_ap_register_repeated<PORT, R>(
        &mut self,
        port: impl Into<PORT>,
        _register: R,
        values: &mut [u32],
    ) -> Result<(), ArmError>
    where
        PORT: AccessPort,
        R: ApRegister<PORT>,
    {
        tracing::debug!(
            "Reading register {}, block with len={} words",
            R::NAME,
            values.len(),
        );

        self.read_raw_ap_register_repeated(port.into().ap_address(), R::ADDRESS, values)
    }
}
