//! Access port v2 specific types and methods.
use enum_primitive_derive::Primitive;
use num_traits::{FromPrimitive, ToPrimitive};

use crate::{
    architecture::arm::{
        ap::{AccessPort, ApAccess, GenericAp, MemoryAp},
        communication_interface::{Initialized, RegisterParseError, SwdSequence},
        ApAddress, ApInformation, ApPort, ArmCommunicationInterface, ArmError, ArmProbe, DapAccess,
        DpAddress, MemoryApInformation,
    },
    define_ap_register,
    probe::DebugProbeError,
};

use super::ApIDR;

pub(crate) mod mock;

/// Describes the class of an access port defined in the [`ARM Debug Interface v6.0`](https://developer.arm.com/documentation/ihi0074/d/?lang=en) specification.
#[derive(Debug, Primitive, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApClass {
    /// This describes a custom AP that is vendor defined and not defined by ARM.
    #[default]
    Undefined = 0b0000,
    /// The standard ARM MEM-AP defined  in the [`ARM Debug Interface v6.0`](https://developer.arm.com/documentation/ihi0074/d/?lang=en) specification.
    MemAp = 0b1000,
}

/// The type of AP defined in the [`ARM Debug Interface v6.0`](https://developer.arm.com/documentation/ihi0074/d/?lang=en) specification.
/// The different types correspond to the different access/memory buses of ARM cores.
#[allow(non_camel_case_types)]
#[derive(Debug, Primitive, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApType {
    /// This is the most basic AP that is included in most MCUs and uses SWD or JTAG as an access bus.
    #[default]
    JtagComAp = 0x0,
    /// A AMBA based AHB3 AP (see E1.5).
    AmbaAhb3 = 0x1,
    /// A AMBA based APB2 and APB3 AP (see E1.8).
    AmbaApb2Apb3 = 0x2,
    /// A AMBA based AXI3 and AXI4 (with optional ACE-Lite) AP (see E1.2 and E1.3).
    AmbaAxi3Axi4 = 0x4,
    /// A AMBA based AHB5 AP (see E1.6).
    AmbaAhb5 = 0x5,
    /// A AMBA based APB4 and APB5 AP (see E1.9)
    AmbaApb4Apb5 = 0x6,
    /// A AMBA based AXI5 AP (see E1.4).
    AmbaAxi5 = 0x7,
    /// A AMBA based protected AHB5 AP (see E1.7).
    AmbaAhb5Hprot = 0x8,
}

define_ap_register!(
    type: GenericAp,
    /// Identification Register
    ///
    /// The identification register is used to identify
    /// an AP.
    ///
    /// It has to be present on every AP.
    name: IDR,
    address: 0xDFC,
    fields: [
        /// The revision of this access point.
        REVISION: u8,
        /// The JEP106 code of the designer of this access point.
        DESIGNER: jep106::JEP106Code,
        /// The class of this access point.
        CLASS: ApClass,
        #[doc(hidden)]
        _RES0: u8,
        /// The variant of this access port.
        VARIANT: u8,
        /// The type of this access port.
        TYPE: ApType,
    ],
    from: value => Ok(IDR {
        REVISION: ((value >> 28) & 0x0F) as u8,
        DESIGNER: {
            let designer = (value >> 17) & 0x7FF;
            let cc = (designer >> 7) as u8;
            let id = (designer & 0x7f) as u8;

            jep106::JEP106Code::new(cc, id)
        },
        CLASS: ApClass::from_u8(((value >> 13) & 0x0F) as u8).ok_or_else(|| RegisterParseError::new("IDR", value))?,
        _RES0: 0,
        VARIANT: ((value >> 4) & 0x0F) as u8,
        TYPE: ApType::from_u8((value & 0x0F) as u8).ok_or_else(|| RegisterParseError::new("IDR", value))?
    }),
    to: value => (u32::from(value.REVISION) << 28)
        | (((u32::from(value.DESIGNER.cc) << 7) | u32::from(value.DESIGNER.id)) << 17)
        | (value.CLASS.to_u32().unwrap() << 13)
        | (u32::from(value.VARIANT) << 4)
        | (value.TYPE.to_u32().unwrap())
);

/// The format of the BASE register (see C2.6.2).
#[derive(Debug, Primitive, Clone, Copy, PartialEq, Eq, Default)]
pub enum BaseaddrFormat {
    /// The legacy format of very old cores. Very little cores use this.
    #[default]
    Legacy = 0,
    /// The format all newer MCUs use.
    ADIv6 = 1,
}

/// Whether a debug entry for this MEM-AP is present (see C2.6.2)
#[derive(Debug, Primitive, Clone, Copy, PartialEq, Eq, Default)]
pub enum DebugEntryState {
    /// Debug entry is not present
    #[default]
    NotPresent = 0,
    /// Debug entry is present
    Present = 1,
}

define_ap_register!(
    type: MemoryAp,
    /// Base register
    name: BASE,
    address: 0xDF8,
    fields: [
        /// The base address of this access point.
        BASEADDR: u32,
        /// The base address format of this access point.
        Format: BaseaddrFormat,
        /// Does this access point exists?
        /// This field can be used to detect access points by iterating over all possible ones until one is found which has `exists == false`.
        present: DebugEntryState,
    ],
    from: value => Ok(BASE {
        BASEADDR: (value & 0xFFFF_F000) >> 12,
        Format: match ((value >> 1) & 0x01) as u8 {
            0 => BaseaddrFormat::Legacy,
            1 => BaseaddrFormat::ADIv6,
            _ => panic!("This is a bug. Please report it."),
        },
        present: match (value & 0x01) as u8 {
            0 => DebugEntryState::NotPresent,
            1 => DebugEntryState::Present,
            _ => panic!("This is a bug. Please report it."),
        },
    }),
   to: value =>
        (value.BASEADDR << 12)
        | (value.Format.to_u32().unwrap() << 1)
        | value.present.to_u32().unwrap()
);

define_ap_register!(
    type: MemoryAp,
    /// Base register
    name: BASE2,
    address: 0xDF0,
    fields: [
        /// The second part of the base address of this access point if required.
        BASEADDR: u32
    ],
    from: value => Ok(BASE2 { BASEADDR: value }),
    to: value => value.BASEADDR
);

/// The increment to the TAR that is performed after each DRW read or write.
///
/// This can be used to avoid successive TAR transfers for writes of consecutive addresses.
/// This will effectively save half the bandwidth!
///
/// Can be configured in the CSW.
#[derive(Debug, Primitive, Clone, Copy, PartialEq, Eq, Default)]
pub enum AddressIncrement {
    /// No increments are happening after the DRW access. TAR always stays the same.
    /// Always supported.
    Off = 0b00,
    /// Increments the TAR by the size of the access after each DRW access.
    /// Always supported.
    #[default]
    Single = 0b01,
    /// Enables packed access to the DRW (see C2.6.15).
    /// Only available if sub-word access is supported by the core.
    Packed = 0b10,
}

/// The unit of data that is transferred in one transfer via the DRW commands.
///
/// This can be configured with the CSW command.
///
/// ALL MCUs support `U32`. All other transfer sizes are optionally implemented.
#[derive(Debug, Primitive, Clone, Copy, PartialEq, Eq, Default)]
pub enum DataSize {
    /// 1 byte transfers are supported.
    U8 = 0b000,
    /// 2 byte transfers are supported.
    U16 = 0b001,
    /// 4 byte transfers are supported.
    #[default]
    U32 = 0b010,
    /// 8 byte transfers are supported.
    U64 = 0b011,
    /// 16 byte transfers are supported.
    U128 = 0b100,
    /// 32 byte transfers are supported.
    U256 = 0b101,
}

impl DataSize {
    /// Create a new `DataSize` from a number of bytes.
    /// Defaults to 4 bytes if the given number of bytes is not available. See [`DataSize`] for available data sizes.
    pub fn from_bytes(bytes: u8) -> Self {
        if bytes == 1 {
            DataSize::U8
        } else if bytes == 2 {
            DataSize::U16
        } else if bytes == 4 {
            DataSize::U32
        } else if bytes == 8 {
            DataSize::U64
        } else if bytes == 16 {
            DataSize::U128
        } else if bytes == 32 {
            DataSize::U256
        } else {
            DataSize::U32
        }
    }
}

define_ap_register!(
    type: MemoryAp,
    /// Control and Status Word register
    ///
    /// The control and status word register (CSW) is used
    /// to configure memory access through the memory AP.
    #[derive(Default)]
    name: CSW,
    address: 0xD00,
    fields: [
        /// Is debug software access enabled.
        DbgSwEnable: u8,           // 1 bit
        /// Prot, implementation defined.
        PROT: u8,                  // 7 bits
        /// Secure Debug Enabled.
        ///
        /// This field is optional and read-only. If not implemented, the bit is `RES0`.
        /// if CSW.DEVICEEN is `0b0`, SDEVICEEN is ignored and the effective value of SDEVICEEN is
        /// `0b1`.
        ///
        /// This bit is equivalent to ADIv5’s SPIDEN field in the same position.
        SDeviceEn: u8,             // 1 bit
        /// Real and Root access status
        ///
        /// This field is read only.
        /// When CFG.RME == 0b1, the defined values of this field are:
        /// - `0b00` Realm and Root accesses are disabled
        /// - `0b01` Realm access is enabled. Root access is disabled.
        /// - `0b11` Realm access is enabled. Root access is enabled.
        ///
        /// Otherwise, this field is `RES0`.
        RMEEN: u8,                 // 2 bits
        /// Error prevent future memory accesses.
        ///
        /// CFG.ERR indicates if this field is implemented.
        ERRSTOP: u8,               // 1 bit
        /// Error are not passed upstream.
        ///
        /// CFG.ERR indicates if this field is implemented.
        ERRNPASS: u8,              // 1 bit
        /// `true` if memory tagging access is enabled.
        MTE: u8,                   // 1 bit
        /// Memory tagging type. Implementation defined.
        Type: u8,                  // 3 bits
        /// Mode of operation. Is set to `0b0000` normally.
        ///
        /// - `0b0000`: Basic mode
        /// - `0b0001`: Barrier support enabled
        Mode: u8,                  // 4 bits
        /// A transfer is in progress.
        /// Can be used to poll whether an aborted transaction has completed.
        /// Read only.
        TrinProg: u8,              // 1 bit
        /// `1` if transactions can be issued through this access port at the moment.
        /// Read only.
        DeviceEn: u8,              // 1 bit
        /// The address increment on DRW access.
        AddrInc: AddressIncrement, // 2 bits
        /// The access size of this memory AP.
        SIZE: DataSize,            // 3 bits
    ],
    from: value => Ok(CSW {
        DbgSwEnable: ((value >> 31) & 0x01) as u8,
        PROT: ((value >> 24) & 0x7F) as u8,
        SDeviceEn: ((value >> 23) & 0x01) as u8,
        RMEEN: ((value >> 21) & 0b11) as u8,
        ERRSTOP: ( (value >> 17) & 0b1 ) as u8,
        ERRNPASS: ( (value >> 16) & 0b1) as u8,
        MTE: ((value >> 15) & 0b1) as u8,
        Type: ((value >> 12) & 0x07) as u8,
        Mode: ((value >> 8) & 0x0F) as u8,
        TrinProg: ((value >> 7) & 0x01) as u8,
        DeviceEn: ((value >> 6) & 0x01) as u8,
        AddrInc: AddressIncrement::from_u8(((value >> 4) & 0x03) as u8).ok_or_else(|| RegisterParseError::new("CSW", value))?,
        SIZE: DataSize::from_u8((value & 0x07) as u8).ok_or_else(|| RegisterParseError::new("CSW", value))?,
    }),
    to: value => (u32::from(value.DbgSwEnable) << 31)
    | (u32::from(value.PROT       ) << 24)
    | (u32::from(value.SDeviceEn  ) << 23)
    | (u32::from(value.RMEEN      ) << 21)
    | (u32::from(value.ERRSTOP    ) << 17)
    | (u32::from(value.ERRNPASS   ) << 16)
    | (u32::from(value.MTE        ) << 15)
    | (u32::from(value.Type       ) << 12)
    | (u32::from(value.Mode       ) <<  8)
    | (u32::from(value.TrinProg   ) <<  7)
    | (u32::from(value.DeviceEn   ) <<  6)
    | (u32::from(value.AddrInc as u8) <<  4)
    // unwrap() is safe!
    | value.SIZE.to_u32().unwrap()
);

impl CSW {
    /// Creates a new CSW content with default values and a configurable [`DataSize`].
    /// See in code documentation for more info.
    ///
    /// The CSW Register is set for an AMBA AHB Access, according to
    /// the ARM Debug Interface Architecture Specification.
    ///
    /// The PROT bits are set as follows:
    ///
    /// ```text
    /// HNONSEC[30]          = 1  - Should be One, if not supported.
    /// MasterType, bit [29] = 1  - Access as default AHB Master
    /// HPROT[4]             = 0  - Non-allocating access
    /// ```
    ///
    /// The CACHE bits are set for the following AHB access:
    ///
    /// ```text
    /// HPROT[0] == 1   - data           access
    /// HPROT[1] == 1   - privileged     access
    /// HPROT[2] == 0   - non-bufferable access
    /// HPROT[3] == 1   - cacheable      access
    /// ```
    ///
    /// Setting cacheable indicates the request must not bypass the cache,
    /// to ensure we observe the same state as the CPU core. On cores without
    /// cache the bit is RAZ/WI.
    pub fn new(data_size: DataSize) -> Self {
        CSW {
            DbgSwEnable: 0b1,
            AddrInc: AddressIncrement::Single,
            SIZE: data_size,
            ..Default::default()
        }
    }
}

define_ap_register!(
    type: MemoryAp,
    /// Data Read/Write register
    ///
    /// The data read/write register (DRW) can be used to read
    /// or write from the memory attached to the memory access point.
    ///
    /// A write to the *DRW* register is translated to a memory write
    /// to the address specified in the TAR register.
    ///
    /// A read from the *DRW* register is translated to a memory read
    name: DRW,
    address: 0xD0C,
    fields: [
        /// The data held in the DRW corresponding to the address held in TAR.
        data: u32,
    ],
    from: value => Ok(DRW { data: value }),
    to: value => value.data
);

define_ap_register!(
    type: MemoryAp,
    /// Transfer Address Register
    ///
    /// The transfer address register (TAR) holds the memory
    /// address which will be accessed through a read or
    /// write of the DRW register.
    name: TAR,
    address: 0xD04,
    fields: [
        /// The register address to be used for the next access to DRW.
        address: u32,
    ],
    from: value => Ok(TAR { address: value }),
    to: value => value.address
);

define_ap_register!(
    type: MemoryAp,
    /// Transfer Address Register - upper word
    ///
    /// The transfer address register (TAR) holds the memory
    /// address which will be accessed through a read or
    /// write of the DRW register.
    name: TAR2,
    address: 0xD08,
    fields: [
        /// The upper 32-bits of the register address to be used for the next access to DRW.
        address: u32,
    ],
    from: value => Ok(TAR2 { address: value }),
    to: value => value.address
);

define_ap_register!(
    type: MemoryAp,
    /// Configuration register
    ///
    /// The configuration register (CFG) is used to determine
    /// which extensions are included in the memory AP.
    name: CFG,
    address: 0xDF4,
    fields: [
        /// TAR incrementer size.
        TARINC: u8,
        /// Identifies the type of error handling that is implemented.
        ERR:u8,
        /// Indicates the size of the DAR0-DAR255 register space. This field can have one of the
        /// following values:
        /// - `0b0000`: DAR0-DAR255 are not implemented
        /// - `0b1010`: DAR0-DAR255, which occupy a register space of 1KB, are implemented.
        DARSIZE: u8,
        /// Real Management Extension.
        RME: u8,
        /// Specifies whether this access port includes the large data extension (access larger than 32 bits).
        LD: u8,
        /// Specifies whether this access port includes the large address extension (64 bit addressing).
        LA: u8,
        /// Specifies whether this architecture uses big endian. Must always be zero for modern chips as the ADI v5.2 deprecates big endian.
        BE: u8,
    ],
    from: value => Ok(CFG {
        TARINC: ((value >> 16) & 0xF) as u8,
        ERR: ((value >> 8) & 0x7) as u8,
        DARSIZE: ((value >> 4) & 0xF) as u8,
        RME: ((value >> 3) & 0b1) as u8,
        LD: ((value >> 2) & 0b1) as u8,
        LA: ((value >> 1) & 0b1) as u8,
        BE: (value & 0b1) as u8,
    }),
    to: value => (u32::from(value.TARINC) << 16)
        | (u32::from(value.ERR) << 8)
        | (u32::from(value.DARSIZE) << 4)
        | (u32::from(value.RME) << 3)
        | (u32::from(value.LD) << 2)
        | (u32::from(value.LA) << 1)
        | u32::from(value.BE)
);

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

        let mut base_address = if BaseaddrFormat::ADIv6 == base_register.Format {
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

        //let supports_hnonsec = csw.HNONSEC == 1;

        //tracing::debug!("HNONSEC supported: {}", supports_hnonsec);

        let device_enabled = csw.DeviceEn == 1;

        tracing::debug!("Device enabled: {}", device_enabled);

        let cfg: CFG = probe.read_ap_register(access_port)?;

        let has_large_address_extension = cfg.LA == 1;
        let has_large_data_extension = cfg.LD == 1;

        Ok(ApInformation::MemoryAp(MemoryApInformation {
            address: access_port.ap_address(),
            supports_only_32bit_data_size: only_32bit_data_size,
            debug_base_address: base_address,
            supports_hnonsec: false,
            has_large_address_extension,
            has_large_data_extension,
            device_enabled,
        }))
    } else {
        Ok(ApInformation::Other {
            address: access_port.ap_address(),
            idr: ApIDR::APv2(idr),
        })
    }
}

/// This provides a partial ArmProbe implementation viable only for the purpose of reading the root
/// ROMTable.
pub struct AccessToRootRomtable<'interface> {
    interface: &'interface mut ArmCommunicationInterface<Initialized>,
    dp: DpAddress,
    base_addr: u64,
}
impl<'interface> AccessToRootRomtable<'interface> {
    pub fn new(
        interface: &'interface mut ArmCommunicationInterface<Initialized>,
        dp: DpAddress,
        base_addr: u64,
    ) -> Self {
        Self {
            interface,
            dp,
            base_addr,
        }
    }
}
impl<'interface> SwdSequence for AccessToRootRomtable<'interface> {
    fn swj_sequence(&mut self, _bit_len: u8, _bits: u64) -> Result<(), DebugProbeError> {
        unimplemented!("This is a bug please report it.")
    }

    fn swj_pins(
        &mut self,
        _pin_out: u32,
        _pin_select: u32,
        _pin_wait: u32,
    ) -> Result<u32, DebugProbeError> {
        unimplemented!("This is a bug please report it.")
    }
}
impl<'interface> ArmProbe for AccessToRootRomtable<'interface> {
    fn read_8(&mut self, _address: u64, _data: &mut [u8]) -> Result<(), ArmError> {
        unimplemented!("This is a bug please report it.")
    }

    fn read_16(&mut self, _address: u64, _data: &mut [u16]) -> Result<(), ArmError> {
        unimplemented!("This is a bug please report it.")
    }

    fn read_32(&mut self, address: u64, data: &mut [u32]) -> Result<(), ArmError> {
        for (i, word) in data.iter_mut().enumerate() {
            let addr = self.base_addr + address + 4 * (i as u64);

            *word = self.interface.read_raw_ap_register(
                ApAddress {
                    dp: self.dp,
                    ap: ApPort::Address(addr & !0xFFF),
                },
                (addr & 0xFFF) as u16,
            )?;
        }
        Ok(())
    }

    fn read_64(&mut self, _address: u64, _data: &mut [u64]) -> Result<(), ArmError> {
        unimplemented!("This is a bug please report it.")
    }

    fn write_8(&mut self, _address: u64, _data: &[u8]) -> Result<(), ArmError> {
        unimplemented!("This is a bug please report it.")
    }

    fn write_16(&mut self, _address: u64, _data: &[u16]) -> Result<(), ArmError> {
        unimplemented!("This is a bug please report it.")
    }

    fn write_32(&mut self, _address: u64, _data: &[u32]) -> Result<(), ArmError> {
        unimplemented!("This is a bug please report it.")
    }

    fn write_64(&mut self, _address: u64, _data: &[u64]) -> Result<(), ArmError> {
        unimplemented!("This is a bug please report it.")
    }

    fn flush(&mut self) -> Result<(), ArmError> {
        unimplemented!("This is a bug please report it.")
    }

    fn supports_native_64bit_access(&mut self) -> bool {
        unimplemented!("This is a bug please report it.")
    }

    fn supports_8bit_transfers(&self) -> Result<bool, ArmError> {
        unimplemented!("This is a bug please report it.")
    }

    fn ap(&mut self) -> MemoryAp {
        MemoryAp {
            address: ApAddress {
                dp: self.dp,
                ap: ApPort::Address(0),
            },
        }
    }

    fn get_arm_communication_interface(
        &mut self,
    ) -> Result<&mut ArmCommunicationInterface<Initialized>, DebugProbeError> {
        unimplemented!("This is a bug please report it.")
    }
}
