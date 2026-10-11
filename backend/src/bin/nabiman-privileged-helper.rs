use std::path::PathBuf;
fn main() -> std::io::Result<()> {
    let mut policy = PathBuf::from("/etc/nabiman/privileged-services.json");
    let mut socket = PathBuf::from("/run/nabiman-helper/control.sock");
    let mut ledger = PathBuf::from("/var/lib/nabiman-helper/operations.db");
    let mut pin_api = None;
    let mut args = std::env::args().skip(1);
    while let Some(option) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| std::io::Error::other("Expected option value"))?;
        match option.as_str() {
            "--policy" => policy = PathBuf::from(value),
            "--socket" => socket = PathBuf::from(value),
            "--ledger" => ledger = PathBuf::from(value),
            "--pin-api" => pin_api = Some(value.parse::<u32>().map_err(std::io::Error::other)?),
            _ => return Err(std::io::Error::other("Unknown helper option")),
        }
    }
    if let Some(pid) = pin_api {
        let approved = nabiman_server::privileged::policy::Policy::load(&policy)?;
        return nabiman_server::privileged::identity::publish(&socket, pid, approved.api_uid);
    }
    nabiman_server::privileged::server::run(&policy, &socket, &ledger)
}
