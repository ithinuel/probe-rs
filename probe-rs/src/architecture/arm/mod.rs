//! All the interface bits for ARM.

pub mod ap;
pub(crate) mod communication_interface;
pub mod component;
pub(crate) mod core;
pub mod dp;
pub mod memory;
pub mod sequences;
pub mod swo;
mod traits;

pub use self::core::{armv6m, armv7a, armv7m, armv8a, armv8m, Dump};
use self::{
    ap::{AccessPort, AccessPortError, MemoryAp},
    armv7a::Armv7aError,
    armv8a::Armv8aError,
    communication_interface::{Initialized, RegisterParseError, SwdSequence},
    dp::DebugPortError,
    memory::romtable::RomTableError,
    sequences::ArmDebugSequenceError,
};
use crate::{probe::DebugProbeError, CoreStatus};
pub use communication_interface::{
    ApInformation, ArmChipInfo, ArmCommunicationInterface, ArmProbeInterface, DapError,
    MemoryApInformation, Register,
};
pub use swo::{SwoAccess, SwoConfig, SwoMode, SwoReader};
pub use traits::*;

/// ArmProbe trait
// TODO: write better doc
pub trait ArmProbe: SwdSequence {
    /// Reads a 8 bit word from `address`.
    fn read_8(&mut self, address: u64, data: &mut [u8]) -> Result<(), ArmError>;

    /// Reads a 16 bit word from `address`.
    fn read_16(&mut self, address: u64, data: &mut [u16]) -> Result<(), ArmError>;

    /// Reads a 32 bit word from `address`.
    fn read_32(&mut self, address: u64, data: &mut [u32]) -> Result<(), ArmError>;

    /// Reads a 64 bit words from `address`.
    fn read_64(&mut self, address: u64, data: &mut [u64]) -> Result<(), ArmError>;

    /// Reads a 64 bit word from `address`.
    fn read_word_64(&mut self, address: u64) -> Result<u64, ArmError> {
        let mut buff = [0];
        self.read_64(address, &mut buff)?;

        Ok(buff[0])
    }

    /// Reads a 32 bit word from `address`.
    fn read_word_32(&mut self, address: u64) -> Result<u32, ArmError> {
        let mut buff = [0];
        self.read_32(address, &mut buff)?;

        Ok(buff[0])
    }

    /// Reads a 16 bit word from `address`.
    fn read_word_16(&mut self, address: u64) -> Result<u16, ArmError> {
        let mut buff = [0];
        self.read_16(address, &mut buff)?;

        Ok(buff[0])
    }

    /// Reads an 8 bit word from `address`.
    fn read_word_8(&mut self, address: u64) -> Result<u8, ArmError> {
        let mut buff = [0];
        self.read_8(address, &mut buff)?;

        Ok(buff[0])
    }

    /// Read a block of 8bit words at `address`. May use 32 bit memory access,
    /// so should only be used if reading memory locations that don't have side
    /// effects. Generally faster than [`MemoryInterface::read_8`].
    fn read(&mut self, address: u64, data: &mut [u8]) -> Result<(), ArmError> {
        let len = data.len();
        if address % 4 == 0 && len % 4 == 0 {
            let mut buffer = vec![0u32; len / 4];
            self.read_32(address, &mut buffer)?;
            for (bytes, value) in data.chunks_exact_mut(4).zip(buffer.iter()) {
                bytes.copy_from_slice(&u32::to_le_bytes(*value));
            }
        } else {
            let start_extra_count = (address % 4) as usize;
            let mut buffer = vec![0u32; (start_extra_count + len + 3) / 4];
            let read_address = address - start_extra_count as u64;
            self.read_32(read_address, &mut buffer)?;
            for (bytes, value) in data
                .chunks_exact_mut(4)
                .zip(buffer[start_extra_count..start_extra_count + len].iter())
            {
                bytes.copy_from_slice(&u32::to_le_bytes(*value));
            }
        }
        Ok(())
    }

    /// Writes 8 bit words to `address`.
    fn write_8(&mut self, address: u64, data: &[u8]) -> Result<(), ArmError>;

    /// Writes 16 bit words to `address`.
    fn write_16(&mut self, address: u64, data: &[u16]) -> Result<(), ArmError>;

    /// Writes 32 bit words to `address`.
    fn write_32(&mut self, address: u64, data: &[u32]) -> Result<(), ArmError>;

    /// Writes 63 bit words to `address`.
    fn write_64(&mut self, address: u64, data: &[u64]) -> Result<(), ArmError>;

    /// Writes a 64 bit word to `address`.
    fn write_word_64(&mut self, address: u64, data: u64) -> Result<(), ArmError> {
        self.write_64(address, &[data])
    }

    /// Writes a 32 bit word to `address`.
    fn write_word_32(&mut self, address: u64, data: u32) -> Result<(), ArmError> {
        self.write_32(address, &[data])
    }

    /// Writes a 16 bit word to `address`.
    fn write_word_16(&mut self, address: u64, data: u16) -> Result<(), ArmError> {
        self.write_16(address, &[data])
    }

    /// Writes a 8 bit word to `address`.
    fn write_word_8(&mut self, address: u64, data: u8) -> Result<(), ArmError> {
        self.write_8(address, &[data])
    }

    /// Write a block of 8bit words to `address`. May use 32 bit memory access,
    /// so it should only be used if writing memory locations that don't have side
    /// effects. Generally faster than [`MemoryInterface::write_8`].
    fn write(&mut self, mut address: u64, mut data: &[u8]) -> Result<(), ArmError> {
        let len = data.len();
        // Number of unaligned bytes at the start
        let start_extra_count = ((4 - (address % 4) as usize) % 4).min(len);
        // Extra bytes to be written at the end
        let end_extra_count = (len - start_extra_count) % 4;
        // Number of bytes between start and end (i.e. number of bytes transmitted as 32 bit words)
        let inbetween_count = len - start_extra_count - end_extra_count;

        assert!(start_extra_count < 4);
        assert!(end_extra_count < 4);
        assert!(inbetween_count % 4 == 0);

        // If we do not have 32 bit aligned access we first check that we can do 8 bit aligned access on this platform.
        // If we cannot we throw an error.
        // If we can we write the first n < 4 bytes up until the word aligned address that comes next.
        if address % 4 != 0 || len % 4 != 0 {
            // If we do not support 8 bit transfers we have to bail because we can only do 32 bit word aligned transers.
            if !self.supports_8bit_transfers()? {
                return Err(ArmError::alignment_error(address, 4));
            }

            // We first do an 8 bit write of the first < 4 bytes up until the 4 byte aligned boundary.
            self.write_8(address, &data[..start_extra_count])?;

            address += start_extra_count as u64;
            data = &data[start_extra_count..];
        }

        // Make sure we don't try to do an empty but potentially unaligned write
        if inbetween_count > 0 {
            // We do a 32 bit write of the remaining bytes that are 4 byte aligned.
            let mut buffer = vec![0u32; inbetween_count / 4];
            for (bytes, value) in data.chunks_exact(4).zip(buffer.iter_mut()) {
                *value = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
            }
            self.write_32(address, &buffer)?;

            address += inbetween_count as u64;
            data = &data[inbetween_count..];
        }

        // We write the remaining bytes that we did not write yet which is always n < 4.
        if end_extra_count > 0 {
            self.write_8(address, &data[..end_extra_count])?;
        }

        Ok(())
    }

    /// Completes all operations
    ///
    /// Some implementation may cache write operations, this method insures this cache is flushed
    /// and all pending operations actually applied.
    fn flush(&mut self) -> Result<(), ArmError>;

    /// Returns true if the ArmProbe supports native 64bits accesses.
    fn supports_native_64bit_access(&mut self) -> bool;

    /// Returns Ok(true) if the ArmProbe supports native 64bits accesses.
    fn supports_8bit_transfers(&self) -> Result<bool, ArmError>;

    /// Returns the underlying [`ApAddress`].
    fn ap(&mut self) -> MemoryAp;

    /// Returns a mutable reference to the internal communication interface.
    fn get_arm_communication_interface(
        &mut self,
    ) -> Result<&mut ArmCommunicationInterface<Initialized>, DebugProbeError>;

    /// Inform the probe of the [`CoreStatus`] of the chip/core attached to
    /// the probe.
    //
    // NOTE: this function should be infallible as it is usually only
    // a visual indication.
    fn update_core_status(&mut self, state: CoreStatus) {
        self.get_arm_communication_interface()
            .map(|iface| iface.core_status_notification(state))
            .ok();
    }
}

/// ARM-specific errors
#[derive(Debug, thiserror::Error)]
#[error("An ARM specific error occurred.")]
pub enum ArmError {
    /// The operation requires a specific architecture.
    #[error("The operation requires one of the following architectures: {0:?}")]
    ArchitectureRequired(&'static [&'static str]),
    /// A timeout occurred during an operation
    #[error("Timeout occurred during operation.")]
    Timeout,
    /// The address is too large for the 32 bit address space.
    #[error("Address is not in 32 bit address space.")]
    AddressOutOf32BitAddressSpace,
    /// The current target device is not an ARM device.
    #[error("Target device is not an ARM device.")]
    NoArmTarget,
    /// Error using a specific AP.
    #[error("Error using access port")]
    AccessPort {
        /// Address of the access port
        address: ApAddress,
        /// Source of the error.
        source: AccessPortError,
    },
    /// An error occurred while using a debug port.
    #[error("Error using a debug port.")]
    DebugPort(#[from] DebugPortError),
    /// The core has to be halted for the operation, but was not.
    #[error("The core needs to be halted for this operation but was not.")]
    CoreNotHalted,
    /// Performing certain operations (e.g device unlock or Chip-Erase) can leave the device in a state
    /// that requires a probe re-attach to resolve.
    #[error("Probe and device internal state mismatch. A probe re-attach is required")]
    ReAttachRequired,
    /// An operation was not performed because the required permissions were not given.
    ///
    /// This can for example happen when the core is locked and needs to be erased to be unlocked.
    /// Then the correct permission needs to be given to automatically unlock the core to prevent accidental erases.
    #[error("An operation could not be performed because it lacked the permission to do so: {0}")]
    MissingPermissions(String),

    /// An error occurred in the communication with an access port or debug port.
    #[error("An error occurred in the communication with an access port or debug port.")]
    Dap(#[from] DapError),

    /// The debug probe encountered an error.
    #[error("The debug probe encountered an error.")]
    Probe(#[from] DebugProbeError),

    /// The given register address to perform an access on was not memory aligned.
    /// Make sure it is aligned to the size of the access (`address & access_size == 0`).
    #[error("Failed to access address 0x{address:08x} as it is not aligned to the requirement of {alignment} bytes for this platform and API call.")]
    MemoryNotAligned {
        /// The address of the register.
        address: u64,
        /// The required alignment in bytes (address increments).
        alignment: usize,
    },
    /// A region outside of the AP address space was accessed.
    #[error("Out of bounds access")]
    OutOfBounds,
    /// The requested memory transfer width is not supported on the current core.
    #[error("{0} bit is not a supported memory transfer width on the current core")]
    UnsupportedTransferWidth(usize),

    /// The AP with the specified address does not exist.
    #[error("The AP with address {0:?} does not exist.")]
    ApDoesNotExist(ApAddress),

    /// The AP has the wrong type for the operation.
    WrongApType,

    /// It is not possible to create a breakpoint a the given address.
    #[error("Unable to create a breakpoint at address {0:#010X}. Hardware breakpoints are only supported at addresses < 0x2000'0000.")]
    UnsupportedBreakpointAddress(u32),

    /// ARMv8a specific error occurred.
    Armv8a(#[from] Armv8aError),

    /// ARMv7a specific error occurred.
    Armv7a(#[from] Armv7aError),

    /// Error occurred in a debug sequence.
    DebugSequence(#[from] ArmDebugSequenceError),

    /// Tracing has not been configured.
    TracingUnconfigured,

    /// Error parsing a register.
    RegisterParse(#[from] RegisterParseError),

    /// Error reading ROM table.
    RomTable(#[source] RomTableError),

    /// Failed to erase chip
    ChipEraseFailed,

    /// The operation requires a specific extension.
    #[error("The operation requires the following extension(s): {0:?}")]
    ExtensionRequired(&'static [&'static str]),

    /// Any other error occurred.
    Other(#[from] anyhow::Error),
}

impl ArmError {
    /// Constructs [`ArmError::MemoryNotAligned`] from the address and the required alignment.
    pub fn from_access_port(err: AccessPortError, ap: impl AccessPort) -> Self {
        ArmError::AccessPort {
            address: ap.ap_address(),
            source: err,
        }
    }

    /// Constructs a [`ArmError::MemoryNotAligned`] from the address and the required alignment.
    pub fn alignment_error(address: u64, alignment: usize) -> Self {
        ArmError::MemoryNotAligned { address, alignment }
    }
}

impl From<RomTableError> for ArmError {
    fn from(value: RomTableError) -> Self {
        match value {
            RomTableError::Memory(err) => *err,
            other => ArmError::RomTable(other),
        }
    }
}

/// Check if the address is a valid 32 bit address. This functions
/// is ARM specific for ease of use, so that a specific error code can be returned.
pub fn valid_32bit_arm_address(address: u64) -> Result<u32, ArmError> {
    address
        .try_into()
        .map_err(|_| ArmError::AddressOutOf32BitAddressSpace)
}
