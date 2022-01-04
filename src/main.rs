mod gdbus;
mod session;

use std::borrow::Cow;

use clap::{AppSettings, Clap};
use gdbus::GnomeShellDBusProxy;
use zbus::Connection;

#[derive(Debug, Clap)]
enum SessionAction {
    Save,
    Restore {
        #[clap(long)]
        rm: bool,

        #[clap(long)]
        mark: bool,
    },
}

#[derive(Debug, Clap)]
#[clap(setting = AppSettings::SubcommandRequired)]
struct Opts {
    #[clap(short, long)]
    file: Option<String>,

    #[clap(long, conflicts_with_all = &["session", "system"])]
    dbus_address: Option<String>,

    #[clap(long, conflicts_with = "system")]
    session: bool,

    #[clap(long, conflicts_with = "session")]
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

    let path = match opts.file {
        Some(f) => Cow::Owned(f),
        None => shellexpand::tilde("~/.last_session"),
    };

    match opts.subcommand {
        SessionAction::Save => session::save_session(&gdb, path.as_ref()),
        SessionAction::Restore { rm, mark } => session::restore_session(&gdb, path.as_ref(), rm, mark),
    }
}
