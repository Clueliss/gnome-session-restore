mod find_command;

use std::{
    io::{Read, Write},
    process::Command,
    time::Duration,
};

use gio::{prelude::AppInfoExt, AppLaunchContext};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{
    dbus::{MetaWindow, WindowCtlProxy},
    session::find_command::Exec,
};

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

pub fn save<W: Write>(conn: &WindowCtlProxy, writer: W, min_wm_class_sim: f64) -> Result<(), SaveError> {
    let num_monitors = conn.get_num_monitors()?;

    let res = conn.list_windows()?;

    let v: Vec<_> = res
        .into_iter()
        .filter_map(|w| {
            let app_id = w.gtk_app_id.clone();

            find_command::find_command(&w, min_wm_class_sim)
                .map(|exec| SessionApplication { window: w, exec })
                .map_err(|e| eprintln!("unable to find command for {:?}: {:?}", app_id, e))
                .ok()
        })
        .collect();

    let session = Session {
        applications: v,
        num_monitors,
    };

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
            }
            Exec::DesktopFile(path) => match gio::DesktopAppInfo::from_filename(path) {
                Some(x) => {
                    if let Err(e) = x.launch_uris::<AppLaunchContext>(&[], None) {
                        eprintln!("Error spawning process '{path:?}': {e:?}");
                    }
                }
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
