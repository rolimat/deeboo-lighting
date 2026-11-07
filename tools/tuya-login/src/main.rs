//! Dev CLI: Smart Life QR login, device list (never keys), saves the chosen strip to `.dev/strip.json`.
//!
//! `.dev` is resolved from the workspace root baked in at build time (`CARGO_MANIFEST_DIR/../..`),
//! so run it from the checkout it was built in (`just tuya-login`).

mod secret_file;

use qrcode::{QrCode, render::unicode};
use secrecy::ExposeSecret;
use secret_file::write_secret_file;
use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;
use std::time::{Duration, Instant};
use tuya_cloud::{
    CloudError, QrLogin, UreqTransport, UserCode, list_devices, poll_login, run_and_revoke,
    start_login,
};

const POLL_EVERY: Duration = Duration::from_secs(2);
const POLL_FOR: Duration = Duration::from_secs(180);

/// Only `--name NAME` is accepted. The User Code is never an argument (it would land in shell history
/// and the process list); unknown arguments are not echoed back.
fn parse_args() -> Result<Option<String>, String> {
    let mut it = std::env::args().skip(1);
    match (it.next().as_deref(), it.next(), it.next()) {
        (None, ..) => Ok(None),
        (Some("--name"), Some(n), None) => Ok(Some(n)),
        _ => Err("usage: tuya-login [--name NAME]".to_owned()),
    }
}

/// Cloud-supplied text goes to the terminal: drop control characters (escape sequences).
fn printable(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect()
}

fn read_user_code() -> Result<UserCode, String> {
    eprint!("Smart Life User Code (app > Settings > Account and Security > User Code): ");
    let _ = std::io::stderr().flush();
    let mut s = String::new();
    std::io::stdin()
        .read_line(&mut s)
        .map_err(|_| "could not read stdin".to_owned())?;
    UserCode::parse(&s).map_err(|e| e.to_string())
}

fn show_qr(qr: &QrLogin) -> Result<(), String> {
    let code = QrCode::new(qr.qr_payload.expose_secret().as_bytes())
        .map_err(|_| "could not render QR".to_owned())?;
    let art = code.render::<unicode::Dense1x2>().quiet_zone(true).build();
    eprintln!("{art}\nScan with Smart Life (Me > scan icon) within 3 minutes.");
    Ok(())
}

fn run() -> Result<(), String> {
    let name = parse_args()?;
    let code = read_user_code()?;
    let t = UreqTransport::new();
    let qr = start_login(&t, &code).map_err(|e| e.to_string())?;
    show_qr(&qr)?;
    let deadline = Instant::now() + POLL_FOR;
    // shortcut: Ctrl-C or a panic skips revoke; the tokens die with the process — the app wizard
    // (Plan 4) must revoke on cancel and quit.
    let session = loop {
        if let Some(s) = poll_login(&t, &code, &qr).map_err(|e| e.to_string())? {
            break s;
        }
        if Instant::now() >= deadline {
            return Err("QR not scanned in time".into());
        }
        std::thread::sleep(POLL_EVERY);
    };
    eprintln!("endpoint host: {}", session.endpoint_host());
    let out = run_and_revoke(&t, session, |t, s| {
        let devices = list_devices(t, s)?;
        println!("name | category | strip? | online");
        for d in &devices {
            let strip = if d.is_light_strip() { "strip" } else { "-" };
            println!(
                "{} | {} | {strip} | {}",
                printable(&d.name),
                printable(&d.category),
                if d.online { "online" } else { "offline" }
            );
        }
        let strips: Vec<_> = devices.iter().filter(|d| d.is_light_strip()).collect();
        let chosen = match &name {
            Some(n) => strips.iter().find(|d| &d.name == n),
            None if strips.len() == 1 => strips.first(),
            None => None,
        };
        Ok(chosen.map(|d| {
            serde_json::json!({ "id": d.id, "name": d.name, "local_key": d.local_key.expose_secret() })
        }).ok_or_else(|| strips.iter().map(|d| d.name.clone()).collect::<Vec<_>>()))
    });
    match &out {
        Ok(_) => eprintln!("session revoked"),
        Err(e) => eprintln!(
            "error: {e}\nthe session was revoked if the cloud accepted it; it expires on its own otherwise"
        ),
    }
    let picked = out.map_err(|e: CloudError| e.to_string())?;
    let json = picked.map_err(|names| {
        format!(
            "pick one strip with --name; strips: {}",
            names
                .iter()
                .map(|n| printable(n))
                .collect::<Vec<_>>()
                .join(", ")
        )
    })?;
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.dev/strip.json");
    write_secret_file(&path, json.to_string().as_bytes())
        .map_err(|e| format!("write failed: {}", e.kind()))?;
    eprintln!("saved .dev/strip.json (0600)");
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::printable;

    #[test]
    fn printable_drops_control_characters() {
        assert_eq!(printable("a\x1b[31mb\n\u{7}c"), "a[31mbc");
    }
}
