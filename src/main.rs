#![feature(bool_to_option, once_cell)]

mod dbus;
mod session;

use clap::{Parser, Subcommand};
use dbus::WindowCtlProxy;
use std::{
    ffi::{OsStr, OsString},
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::PathBuf,
};
use zbus::Connection;

fn valid_sim_value(s: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let x = s.parse::<f32>()?;

    if (0.0..=1.0).contains(&x) {
        Ok(())
    } else {
        Err("expected value in range 0.0..=1.0".into())
    }
}

fn default_session_file_path() -> PathBuf {
    xdg::BaseDirectories::with_prefix("gnome-session-restore")
        .unwrap()
        .place_state_file("last-session.json")
        .unwrap()
}

#[derive(Debug, Subcommand)]
enum SessionAction {
    /// Saves the current gnome session
    Save {
        /// Set the minimum required (levenshtein) similarity between the WM_CLASS
        /// and the binary name to allow it to be considered
        /// as an alternative application name.
        #[clap(long, default_value_t = 0.25, validator = valid_sim_value)]
        min_wm_class_sim: f64,
    },

    /// Restores a gnome session from disk
    Restore {
        /// Remove the session file after restoring
        /// [hint: ignored when reading from stdin]
        #[clap(long)]
        rm: bool,

        /// Rename the file to the given name after restoring
        /// [hint: ignored when reading from stdin]
        #[clap(long)]
        rename: Option<OsString>,
    },
}

#[derive(Debug, Parser)]
#[clap(version, author, about, subcommand_required = true)]
struct Opts {
    /// Manually specify a session file [hint: use `-` for std(in|out) redirection]
    #[clap(short, long, default_value_os_t = default_session_file_path(), forbid_empty_values = true)]
    file: PathBuf,

    /// Connect to the specified D-Bus address
    #[clap(long, conflicts_with_all = &["session", "system"])]
    dbus_address: Option<String>,

    /// Connect to the session D-Bus [default]
    #[clap(long, conflicts_with = "system")]
    session: bool,

    /// Connect to the system D-Bus
    #[clap(long, conflicts_with = "session")]
    system: bool,

    #[clap(subcommand)]
    subcommand: SessionAction,
}

fn main() {
    let opts = Opts::parse();
    let redirected_to_std_stream = opts.file == OsStr::new("-");

    let conn = if opts.system {
        Connection::new_system().expect("system dbus")
    } else if let Some(addr) = &opts.dbus_address {
        Connection::new_for_address(addr, true).expect("dbus at address")
    } else {
        Connection::new_session().expect("session dbus")
    };

    let shellbus = WindowCtlProxy::new(&conn).expect("service at destination");

    match opts.subcommand {
        SessionAction::Save { min_wm_class_sim } => {
            let writer: Box<dyn Write> = if redirected_to_std_stream {
                Box::new(std::io::stdout())
            } else {
                let f = File::create(&opts.file).unwrap();
                let bw = BufWriter::new(f);

                Box::new(bw)
            };

            session::save(&shellbus, writer, min_wm_class_sim).unwrap();
        }
        SessionAction::Restore { rm, rename } => {
            let reader: Box<dyn Read> = if redirected_to_std_stream {
                Box::new(std::io::stdin())
            } else {
                let f = File::open(&opts.file).unwrap();
                let br = BufReader::new(f);

                Box::new(br)
            };

            session::restore(&shellbus, reader).unwrap();

            if redirected_to_std_stream {
                eprintln!("ignoring `--rm` and `--rename` because input file was stdin");
            } else if let Some(new_name) = rename {
                let new_file = opts.file.with_file_name(new_name);
                std::fs::rename(&opts.file, &new_file).unwrap();

                if rm {
                    std::fs::remove_file(new_file).unwrap();
                }
            } else if rm {
                std::fs::remove_file(&opts.file).unwrap();
            }
        }
    }
}
