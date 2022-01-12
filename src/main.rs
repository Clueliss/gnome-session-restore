#![feature(bool_to_option)]

mod dbus;
mod session;

use std::borrow::Cow;

use clap::{AppSettings, Parser, Subcommand};
use dbus::WindowCtlProxy;
use zbus::Connection;

fn valid_sim_value(s: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let x = s.parse::<f32>()?;

    if (0.0..=1.0).contains(&x) {
        Ok(())
    } else {
        Err("expected value in range 0.0..=1.0".into())
    }
}

#[derive(Debug, Subcommand)]
enum SessionAction {
    /// Saves the current gnome session
    Save {
        /// Sets the minimum required similarity between the WM_CLASS
        /// and the process name to allow for the process name to be considered
        /// as an alternative application name.
        #[clap(long, default_value = "0.25", validator = valid_sim_value)]
        min_wm_class_sim: f64,
    },

    /// Restores a gnome session from disk
    Restore {
        /// Removes the session file after restoring
        #[clap(long)]
        rm: bool,

        /// Marks the session file with the current timestamp after restoring
        #[clap(long)]
        mark: bool,
    },
}

#[derive(Debug, Parser)]
#[clap(setting = AppSettings::SubcommandRequired, version, author, about)]
struct Opts {
    /// Manually specify a session file
    #[clap(short, long, default_value = "~/.last_session")]
    file: String,

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

    let conn = if opts.system {
        Connection::new_system().expect("system dbus")
    } else if let Some(addr) = &opts.dbus_address {
        Connection::new_for_address(addr, true).expect("dbus at address")
    } else {
        Connection::new_session().expect("session dbus")
    };

    let shellbus = WindowCtlProxy::new(&conn).unwrap();

    match opts.subcommand {
        SessionAction::Save { min_wm_class_sim } => {
            let path = if opts.file == "-" {
                Cow::Borrowed("/proc/self/fd/1")
            } else {
                shellexpand::tilde(&opts.file)
            };

            session::save(&shellbus, path.as_ref(), min_wm_class_sim).unwrap();
        }
        SessionAction::Restore { rm, mark } => {
            let path = if opts.file == "-" {
                Cow::Borrowed("/proc/self/fd/0")
            } else {
                shellexpand::tilde(&opts.file)
            };

            session::restore(&shellbus, path.as_ref(), rm, mark).unwrap();
        }
    }
}
