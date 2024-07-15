#![deny(warnings)]

use std::{
    fs,
    process::{Child, Command, Stdio},
    sync::{atomic::AtomicBool, Arc},
    thread::sleep,
    time::Duration,
};

use clap::Parser;

#[derive(Parser)]
struct Cli {
    port: String,
    baudrate: u32,
    #[clap(short, long, action)]
    debug: bool,
    #[clap(short, long)]
    socat_port: Option<String>,
}

fn run_socat(port_in: &str, port_out: &str) -> Result<Child, String> {
    let child = Command::new("socat")
        .arg(format!("PTY,link={},raw,echo=0", port_in))
        .arg(format!("PTY,link={},raw,echo=0", port_out))
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|err| format!("Cannot spawn socat process: {}", err))?;
    sleep(Duration::from_millis(10));

    Ok(child)
}

fn main() -> Result<(), String> {
    let cli = Cli::parse();

    let port_out = cli.port;
    let baudrate = cli.baudrate;
    let debug = cli.debug;
    let exit = Arc::new(AtomicBool::new(false));
    let exit2 = exit.clone();

    if debug {
        println!("Port {} selected", port_out);
        println!("Baudrate {} selected", baudrate);
    }

    let socat_proc = if let Some(port_in) = cli.socat_port {
        Some((run_socat(&port_in, &port_out)?, port_in))
    } else {
        None
    };

    ctrlc::set_handler(move || {
        exit2.store(true, std::sync::atomic::Ordering::SeqCst);
    })
    .map_err(|err| format!("Cannot set CTRL+C handler: {}", err))?;

    let mut s = serialport::new(port_out.clone(), baudrate)
        .open()
        .map_err(|err| {
            format!(
                "Cannot open serial port {} with baudrate {} bps: {}",
                &port_out, baudrate, err
            )
        })?;
    let mut output = [0u8];

    'main: loop {
        if exit.load(std::sync::atomic::Ordering::SeqCst) {
            break 'main;
        }

        while s.read(&mut output).is_err() {
            if exit.load(std::sync::atomic::Ordering::SeqCst) {
                break 'main;
            }
        }

        if debug {
            println!("Read: {:?}", &output);
        }
        s.write_all(&output)
            .map_err(|err| format!("Cannot write a byte on serial: {}", err))?;
        s.flush()
            .map_err(|err| format!("Cannot flush serial data: {}", err))?;
    }

    if let Some((mut socat_proc, port_in)) = socat_proc {
        socat_proc
            .kill()
            .map_err(|err| format!("Cannot kill socat process: {}", err))?;
        fs::remove_file(&port_in).map_err(|err| format!("Cannot remove {}: {}", port_in, err))?;
        fs::remove_file(&port_out).map_err(|err| format!("Cannot remove {}: {}", port_out, err))?;
        if debug {
            println!("Kill socat succesfully");
        }
    }

    if debug {
        println!("See you later ^^");
    }

    Ok(())
}