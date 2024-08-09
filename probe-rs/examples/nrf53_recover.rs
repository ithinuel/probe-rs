use anyhow::Result;
use probe_rs::{
    architecture::arm::{ApAddress, ApPort, DpAddress},
    probe::list::Lister,
};

fn main() -> Result<()> {
    pretty_env_logger::init();

    let lister = Lister::new();

    // Get a list of all available debug probes.
    let probes = lister.list_all();

    // Use the first probe found.
    let mut probe = probes[0].open()?;

    probe.attach_to_unspecified()?;
    let mut iface = probe
        .try_into_arm_interface()
        .unwrap()
        .initialize_unspecified(DpAddress::Default)
        .unwrap();

    // This is an example on how to do a "recover" operation (erase+unlock a locked chip)
    // on an nRF5340 target.

    const APP_MEM: ApAddress = ApAddress {
        ap: ApPort::Index(0),
        dp: DpAddress::Default,
    };
    const NET_MEM: ApAddress = ApAddress {
        ap: ApPort::Index(1),
        dp: DpAddress::Default,
    };
    const APP_CTRL: ApAddress = ApAddress {
        ap: ApPort::Index(2),
        dp: DpAddress::Default,
    };
    const NET_CTRL: ApAddress = ApAddress {
        ap: ApPort::Index(3),
        dp: DpAddress::Default,
    };

    const ERASEALL: u16 = 0x04;
    const ERASEALLSTATUS: u16 = 0x08;
    const IDR: u16 = 0xFC;

    for &ap in &[APP_MEM, NET_MEM, APP_CTRL, NET_CTRL] {
        println!("IDR {:?} {:x}", ap, iface.read_raw_ap_register(ap, IDR)?);
    }

    for &ap in &[APP_CTRL, NET_CTRL] {
        // Start erase
        iface.write_raw_ap_register(ap, ERASEALL, 1)?;
        // Wait for erase done
        while iface.read_raw_ap_register(ap, ERASEALLSTATUS)? != 0 {}
    }

    Ok(())
}
