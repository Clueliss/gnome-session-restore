mod gdbus;
mod session;

use clap::{crate_authors, crate_description, crate_version, AppSettings, Parser, Subcommand};
use gdbus::{
    extensions::{extension_info::ExtensionState, GnomeShellExtensionsDBusProxy},
    GnomeShellDBusProxy,
};
use include_js::JSStr;
use serde::Deserialize;
use std::fs::File;
use zbus::Connection;

const UNSAFE_MODE_ENABLER_UUID: &str = "unsafe-mode-enabler@Clueliss.github.com";

#[derive(Debug, Subcommand)]
enum SessionAction {
    #[clap(about = "Saves the current gnome session")]
    Save,
    #[clap(about = "Restores a gnome session from disk")]
    Restore {
        #[clap(long, about = "Removes the session file after restoring")]
        rm: bool,

        #[clap(
            long,
            about = "Marks the session file with the current timestamp after restoring"
        )]
        mark: bool,
    },
}

#[derive(Debug, Parser)]
#[clap(setting = AppSettings::SubcommandRequired, version = crate_version!(), author = crate_authors!(), about = crate_description!())]
struct Opts {
    #[clap(
        short,
        long,
        about = "Manually specify a session file",
        default_value = "~/.last_session"
    )]
    file: String,

    #[clap(long, conflicts_with_all = &["session", "system"], about = "Connect to the specified D-Bus address")]
    dbus_address: Option<String>,

    #[clap(
        long,
        conflicts_with = "system",
        about = "Connect to the session D-Bus [default]"
    )]
    session: bool,

    #[clap(
        long,
        conflicts_with = "session",
        about = "Connect to the system D-Bus"
    )]
    system: bool,

    #[clap(
        long,
        about = "overrides the use_unsafe_mode_enabler option in ~/.config/gnome-session-restore.conf"
    )]
    use_unsafe_mode_enabler_override: Option<bool>,

    #[clap(subcommand)]
    subcommand: SessionAction,
}

#[derive(Debug, Default, Deserialize)]
struct Config {
    use_unsafe_mode_enabler: bool,
}

fn main() {
    let opts = Opts::parse();
    let config = {
        if let Ok(f) =
            File::open(shellexpand::tilde("~/.config/gnome-session-restore.conf").as_ref())
        {
            serde_json::from_reader(f).expect("invalid syntax in config file")
        } else {
            Config::default()
        }
    };

    let use_unsafe_mode_enabler = opts
        .use_unsafe_mode_enabler_override
        .unwrap_or(config.use_unsafe_mode_enabler);

    let conn = if opts.system {
        Connection::new_system().expect("could not connect to system dbus")
    } else if let Some(addr) = &opts.dbus_address {
        Connection::new_for_address(addr, true).expect("could not connect to dbus")
    } else {
        Connection::new_session().expect("could not connect to session dbus")
    };

    let shellbus = GnomeShellDBusProxy::new(&conn).expect("failed to create proxy");

    let (extbus, prev_ext_state) = if use_unsafe_mode_enabler {
        let extbus =
            GnomeShellExtensionsDBusProxy::new(&conn).expect("failed to create extension proxy");

        let prev_state = extbus.get_extension_info(UNSAFE_MODE_ENABLER_UUID).unwrap();

        (Some(extbus), prev_state.state)
    } else {
        let _: bool = shellbus.eval(unsafe { JSStr::new_unchecked("true") })
            .expect("gnome-shell could not complete a simple Eval, this probably means you need to use unsafe-mode-enabler or enable unsafe mode yourself");

        (None, ExtensionState::Uninstalled)
    };

    let path = shellexpand::tilde(&opts.file);

    match (&extbus, prev_ext_state) {
        (Some(_), ExtensionState::Enabled) => (),
        (Some(extb), _) => extb
            .enable_extension(UNSAFE_MODE_ENABLER_UUID)
            .expect("unable to enable unsafe-mode-enabler"),
        _ => (),
    }

    let res = match opts.subcommand {
        SessionAction::Save => session::save_session(&shellbus, path.as_ref()),
        SessionAction::Restore { rm, mark } => {
            session::restore_session(&shellbus, path.as_ref(), rm, mark)
        }
    };

    if let (Some(extb), ExtensionState::Disabled) = (&extbus, prev_ext_state) {
        if let Err(e) = extb.disable_extension(UNSAFE_MODE_ENABLER_UUID) {
            eprintln!("Error: unable to disable unsafe-mode-enabler: {:?}", e);
        }
    }

    res.expect("could not save/restore");
}
