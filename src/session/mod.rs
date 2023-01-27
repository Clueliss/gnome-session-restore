use crate::dbus::{MetaWindow, WindowCtlProxy};
use gio::{prelude::AppInfoExt, AppLaunchContext};
use serde::{ser::SerializeSeq, Deserialize, Serialize, Serializer};
use std::{
    ffi::OsString,
    io::{Read, Write},
    path::PathBuf,
    process::Command,
    time::Duration,
};
use thiserror::Error;

pub use crate::find_command::{Capability, Confidence, FindOptions};

fn utf8_ser<S: Serializer>(x: &[OsString], s: S) -> Result<S::Ok, S::Error> {
    let mut seq = s.serialize_seq(Some(x.len()))?;

    let itr = x.iter().map(|osstr| osstr.to_str().unwrap());

    for item in itr {
        seq.serialize_element(item)?;
    }

    seq.end()
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
pub enum Exec {
    CmdLine(#[serde(serialize_with = "utf8_ser")] Vec<OsString>),
    DesktopFile(PathBuf),
}

#[derive(Serialize, Deserialize, Debug)]
struct SessionApplication {
    #[serde(flatten)]
    window: MetaWindow,
    exec: Exec,
}

#[derive(Serialize, Deserialize, Debug)]
struct Session {
    applications: Vec<SessionApplication>,
    num_monitors: u32,
}

fn dedup_applications(sess: &mut Vec<SessionApplication>) {
    sess.sort_by(|app1, app2| app1.window.window_class.cmp(&app2.window.window_class));
    sess.dedup_by(|app1, app2| app1.window.window_class == app2.window.window_class);
}

#[derive(Debug, Error)]
pub enum SaveError {
    #[error("dbus error {0}")]
    DBus(#[from] zbus::Error),

    #[error("serialization error {0}")]
    Serialization(#[from] serde_json::Error),
}

pub type RestoreError = serde_json::Error;

pub fn save<W: Write, F, E>(conn: &WindowCtlProxy, writer: W, find: F) -> Result<(), SaveError>
where
    F: Fn(&MetaWindow) -> Result<Exec, E>,
    E: std::error::Error,
{
    let num_monitors = conn.get_num_monitors()?;

    let res = conn.list_windows()?;

    let v: Vec<_> = res
        .into_iter()
        .filter(|w| w.window_class != "Gnome-shell")
        .filter_map(|w| {
            let wm_class = w.window_class.clone();
            let gtk_app_id = w.gtk_app_id.clone();
            let sandboxed_app_id = w.sandboxed_app_id.clone();
            let pid = w.pid;

            find(&w)
                .map(|exec| SessionApplication { window: w, exec })
                .map_err(|e| eprintln!("unable to find command for {{ wm_class: {:?}, gtk_app_id: {:?}, sandboxed_app_id: {:?}, pid: {:?} }}: {e}", wm_class, gtk_app_id, sandboxed_app_id, pid))
                .ok()
        })
        .collect();

    let session = Session { applications: v, num_monitors };

    serde_json::to_writer(writer, &session)?;

    Ok(())
}

pub fn restore<R: Read>(conn: &WindowCtlProxy, rdr: R) -> Result<(), RestoreError> {
    let deduped_sess = {
        let mut sess: Session = serde_json::from_reader(rdr)?;
        dedup_applications(&mut sess.applications);
        sess
    };

    for app in &deduped_sess.applications {
        match &app.exec {
            Exec::CmdLine(cmdline) => {
                let res = Command::new(&cmdline[0]).args(&cmdline[1..]).spawn();

                if let Err(e) = res {
                    eprintln!("Error spawning process '{cmdline:?}': {e:?}");
                }
            },
            Exec::DesktopFile(path) => match gio::DesktopAppInfo::from_filename(path) {
                Some(x) => {
                    if let Err(e) = x.launch_uris::<AppLaunchContext>(&[], None) {
                        eprintln!("Error spawning process '{path:?}': {e:?}");
                    }
                },
                None => eprintln!("Error spawning process '{path:?}': could not get desktop app info"),
            },
        }
    }

    std::thread::sleep(Duration::from_secs(1));

    let cur_num_monitors = conn.get_num_monitors();

    if matches!(cur_num_monitors, Ok(n) if n == deduped_sess.num_monitors) {
        for app in &deduped_sess.applications {
            if !app.window.window_class.is_empty() {
                if let Err(e) = conn.set_window_geom_by_class(&app.window.window_class, app.window.geom) {
                    eprintln!("Error moving window '{class}': {e:?}", class = app.window.window_class,);
                }
            }
        }
    }

    Ok(())
}
