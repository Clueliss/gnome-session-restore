#![feature(once_cell)]

mod dbus;
pub mod find_command;
mod session;

use crate::dbus::MetaWindow;
use clap::{ArgEnum, Parser, Subcommand, ValueHint};
use dbus::WindowCtlProxy;
use session::{Capability, Confidence};
use std::{
    collections::HashSet,
    ffi::{OsStr, OsString},
    fmt::Debug,
    fs::File,
    io::{BufReader, BufWriter, Read, Write},
    path::PathBuf,
};
use zbus::Connection;

fn valid_confidence_value(s: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
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

#[derive(ArgEnum, Copy, Clone, PartialEq, Debug)]
enum Policy {
    Allow,
    Deny,
}

#[derive(Debug, Subcommand)]
enum SessionAction {
    /// Saves the current gnome session
    Save {
        /// Set the minimum required (levenshtein) similarity between the WM_CLASS
        /// and the binary name to allow it to be considered
        /// as an alternative application name.
        #[clap(long, default_value_t = 0.8, validator = valid_confidence_value)]
        min_wm_class_similarity: Confidence,

        #[clap(long, default_value_t = 0.6, validator = valid_confidence_value)]
        min_partial_match_confidence: Confidence,

        /// Determine whether gnome-session-restore is allowed to search in /proc/{pid}/cmdline
        /// to obtain information that may be helpful. [hint: specifying deny will also implicitly add --procfs-use-comand-policy deny]
        #[clap(long, arg_enum, default_value_t = Policy::Allow)]
        procfs_search_policy: Policy,

        /// Determine whether gnome-session-restore is allowed to use the command it finds
        /// in /proc/{pid}/commandline as a way to start an application if not desktop file is found.
        #[clap(long, arg_enum, default_value_t = Policy::Deny)]
        procfs_use_command_policy: Policy,
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
    #[clap(short, long, default_value_os_t = default_session_file_path(), forbid_empty_values = true, value_hint = ValueHint::FilePath)]
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
        SessionAction::Save {
            min_wm_class_similarity,
            min_partial_match_confidence,
            procfs_search_policy,
            procfs_use_command_policy,
        } => {
            let writer: Box<dyn Write> = if redirected_to_std_stream {
                Box::new(std::io::stdout())
            } else {
                let f = File::create(&opts.file).unwrap();
                let bw = BufWriter::new(f);

                Box::new(bw)
            };

            let caps = {
                let mut hs = HashSet::new();

                if let Policy::Allow = procfs_search_policy {
                    hs.insert(Capability::ProcFsSearch);
                }

                if let Policy::Allow = procfs_use_command_policy {
                    hs.insert(Capability::UseProcFsCommand);
                }

                hs
            };

            let options = session::FindOptions {
                min_wm_class_similarity,
                min_partial_match_confidence,
                capabilities: &caps,
            };

            let finder = move |mw: &MetaWindow| find_command::find_command(options, mw);

            session::save(&shellbus, writer, finder).unwrap();
        },
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
        },
    }
}
