mod dbus;
mod session;

use clap::{crate_authors, crate_description, crate_version, AppSettings, Parser, Subcommand};
use dbus::WindowCtlProxy;
use zbus::Connection;

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

    let path = shellexpand::tilde(&opts.file);

    match opts.subcommand {
        SessionAction::Save => session::save(&shellbus, path.as_ref()).unwrap(),
        SessionAction::Restore { rm, mark } => {
            session::restore(&shellbus, path.as_ref(), rm, mark).unwrap();
        }
    }
}
