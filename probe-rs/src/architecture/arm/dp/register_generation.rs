//! Helper macros to implement an access port
#[macro_export]
/// Defines a new debug port register for typed access.
macro_rules! define_dp_register {
    (
        $type:ident,
        $version:ident,
        $address:expr,
        $name:expr
    ) => {
        impl TryFrom<u32> for $type {
            type Error = RegisterParseError;

            fn try_from(raw: u32) -> Result<Self, Self::Error> {
                Ok($type(raw))
            }
        }

        impl From<$type> for u32 {
            fn from(raw: $type) -> Self {
                raw.0
            }
        }

        impl DpRegister for $type {
            const VERSION: DebugPortVersion = DebugPortVersion::$version;
        }

        impl Register for $type {
            const ADDRESS: u16 = $address;
            const NAME: &'static str = $name;
        }
    };
}
