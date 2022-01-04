mod gdbus;
mod session;

use clap::{AppSettings, Clap, crate_authors, crate_version, crate_description};
use gdbus::GnomeShellDBusProxy;
use zbus::Connection;

#[derive(Debug, Clap)]
enum SessionAction {
    #[clap(about = "Saves the current gnome session")]
    Save,
    #[clap(about = "Restores a gnome session from disk")]
    Restore {
        #[clap(long, about = "Removes the session file after restoring")]
        rm: bool,

        #[clap(long, about = "Marks the session file with the current timestamp after restoring")]
        mark: bool,
    },
}

#[derive(Debug, Clap)]
#[clap(setting = AppSettings::SubcommandRequired, version = crate_version!(), author = crate_authors!(), about = crate_description!())]
struct Opts {
    #[clap(short, long, about = "Manually specify a session file", default_value = "~/.last_session")]
    file: String,

    #[clap(long, conflicts_with_all = &["session", "system"], about = "Connect to the specified D-Bus address")]
    dbus_address: Option<String>,

    #[clap(long, conflicts_with = "system", about = "Connect to the session D-Bus [default]")]
    session: bool,

    #[clap(long, conflicts_with = "session", about = "Connect to the system D-Bus")]
    system: bool,

    #[clap(subcommand)]
    subcommand: SessionAction,
}

fn main() {
    let opts = Opts::parse();

    let conn = if opts.system {
        Connection::new_system().expect("could not connect to system dbus")
    } else if let Some(addr) = &opts.dbus_address {
        Connection::new_for_address(addr, true).expect("could not connect to dbus")
    } else {
        Connection::new_session().expect("could not connect to session dbus")
    };

    let gdb = GnomeShellDBusProxy::new(&conn).expect("failed to create proxy");

    let path = shellexpand::tilde(&opts.file);

    match opts.subcommand {
        SessionAction::Save => session::save_session(&gdb, path.as_ref()),
        SessionAction::Restore { rm, mark } => session::restore_session(&gdb, path.as_ref(), rm, mark),
    }
}
