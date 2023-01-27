pub mod methods;

use crate::dbus::MetaWindow;
use clap::ArgEnum;
use regex::Regex;
use std::{
    collections::HashSet,
    ffi::OsStr,
    path::{Path, PathBuf},
    sync::LazyLock,
};
use thiserror::Error;

use crate::session;
pub use methods::Confidence;

static DESKTOP_ENTRY_LOCATIONS: LazyLock<HashSet<PathBuf>> = LazyLock::new(|| {
    let bd = xdg::BaseDirectories::new().unwrap();

    std::iter::once(bd.get_data_home())
        .chain(bd.get_data_dirs())
        .filter_map(|mut p| {
            p.push("applications");

            if p.exists() {
                Some(p)
            } else {
                eprintln!("Ignoring {p:?} reason: directory does not exist");
                None
            }
        })
        .collect()
});

#[derive(ArgEnum, Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Capability {
    ProcFsSearch,
    UseProcFsCommand,
}

#[derive(Debug, Copy, Clone)]
pub struct FindOptions<'r> {
    pub min_wm_class_similarity: Confidence,
    pub min_partial_match_confidence: Confidence,
    pub capabilities: &'r HashSet<Capability>,
}

#[derive(Error, Debug)]
pub enum FindError {
    #[error("io error")]
    IOError(#[from] std::io::Error),

    #[error("could not find a suitable entry")]
    NoSuitableEntryFound,

    #[error("process is zombie")]
    ProcessIsZombie,

    #[error("proc search disabled but could not find alternative")]
    ProcSearchDisabledNoOtherOptionFound,

    #[error("found cmd in proc but not allowed to use")]
    NotAllowedToUseProcCmdNoOtherOptionFound,
}

pub fn find_command(options: FindOptions, meta: &MetaWindow) -> Result<session::Exec, FindError> {
    static DESKTOP_FILES: LazyLock<Vec<PathBuf>> = LazyLock::new(|| {
        DESKTOP_ENTRY_LOCATIONS
            .iter()
            .filter_map(|location| std::fs::read_dir(location).ok())
            .flatten()
            .flatten()
            .map(|direntry| direntry.path())
            .filter(|path| path.extension().map_or(false, |ext| ext == "desktop"))
            .collect()
    });

    try_find_command_any(options, meta, &DESKTOP_FILES.iter())
}

pub fn try_find_command_any<D, P>(
    options: FindOptions,
    meta: &MetaWindow,
    desktop_files: &D,
) -> Result<session::Exec, FindError>
where
    D: Iterator<Item = P> + Clone,
    P: AsRef<Path>,
{
    if !meta.gtk_app_id.is_empty() {
        if let Ok(exec) = methods::try_find_command_by_gtk_app_id(&meta.gtk_app_id) {
            return Ok(exec);
        }
    }

    if !meta.sandboxed_app_id.is_empty() {
        if let Ok(exec) =
            methods::try_find_command_by_sandboxed_app_id(&meta.sandboxed_app_id, DESKTOP_ENTRY_LOCATIONS.iter())
        {
            return Ok(exec);
        }
    }

    match methods::try_find_command_by_wm_class(&meta.window_class, desktop_files.clone()) {
        Ok((exec, confidence)) if confidence >= options.min_wm_class_similarity => return Ok(exec),
        _ => (),
    }

    let maybe_proc_cmdline = if options.capabilities.contains(&Capability::ProcFsSearch) {
        methods::try_find_command_in_proc(meta.pid)
    } else {
        Err(FindError::ProcSearchDisabledNoOtherOptionFound)
    };

    let alt_search_terms = {
        static CHROME_APP_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new("chrome-(?P<website>.+?)__.*?-(?P<profile>.+)").unwrap());

        let mut buf = Vec::new();

        if !meta.window_class.is_empty() {
            buf.push((&meta.window_class).into());
        }

        if let Some(cap) = CHROME_APP_RE.captures(&meta.window_class) {
            buf.extend([cap.name("website"), cap.name("profile")].map(|m| m.unwrap().as_str().into()));
        }

        {
            let proc_binary = maybe_proc_cmdline
                .as_ref()
                .ok()
                .and_then(|cmdline| cmdline.get(0))
                .and_then(|binary| Path::new(binary).file_name())
                .map(OsStr::to_string_lossy);

            if let Some(proc_binary) = proc_binary {
                if meta.window_class.is_empty()
                    || strsim::normalized_levenshtein(&proc_binary, &meta.window_class) > 0.5
                {
                    buf.push(proc_binary);
                }
            }
        }

        buf
    };

    let search_term_result = alt_search_terms
        .into_iter()
        .filter_map(|search_term| methods::try_find_command_by_search_term(&search_term, desktop_files.clone()).ok())
        .reduce(
            |acc @ (_, acc_sim), x @ (_, x_sim)| {
                if x_sim > acc_sim {
                    x
                } else {
                    acc
                }
            },
        );

    match search_term_result {
        Some((exec, confidence)) if confidence >= options.min_partial_match_confidence => return Ok(exec),
        _ => (),
    }

    if options.capabilities.contains(&Capability::UseProcFsCommand) {
        Ok(session::Exec::CmdLine(maybe_proc_cmdline?))
    } else {
        Err(FindError::NotAllowedToUseProcCmdNoOtherOptionFound)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        dbus::{MetaWindow, WindowGeom},
        find_command::{FindError, FindOptions},
        session::Exec,
    };
    use std::{collections::HashSet, path::Path, sync::LazyLock};

    const TESTSET: &str = include_str!("../../testset.list");

    fn get_testset() -> impl Iterator<Item = &'static Path> + Clone {
        static TS: LazyLock<Vec<&'static Path>> = LazyLock::new(|| {
            TESTSET
                .split("\n")
                .filter(|s| !s.is_empty())
                .map(|s| Path::new(s))
                .collect()
        });

        TS.iter().map(|&p| p)
    }

    fn find_dummy(window_class: &str, gtk_app_id: &str, sandboxed_app_id: &str) -> Result<Exec, FindError> {
        super::try_find_command_any(
            FindOptions {
                min_wm_class_similarity: 0.8,
                min_partial_match_confidence: 0.6,
                capabilities: &HashSet::new(),
            },
            &MetaWindow {
                geom: WindowGeom { x: 0, y: 0, width: 0, height: 0, minimized: false },
                pid: 0,
                stable_seq: 0,
                window_class: window_class.to_string(),
                gtk_app_id: gtk_app_id.to_string(),
                sandboxed_app_id: sandboxed_app_id.to_string(),
            },
            &get_testset(),
        )
    }

    #[test]
    fn find_chrome_custom_app() {
        let s = find_dummy("chrome-listen.tidal.com__-Spotify", "", "").expect("finding any");

        assert_eq!(
            s,
            Exec::DesktopFile("/home/liss/.local/share/applications/tidal.desktop".into())
        );
    }

    #[test]
    fn find_flatpak_app() {
        let s = find_dummy("jetbrains-clion", "", "com.jetbrains.CLion").expect("finding clion");

        assert_eq!(
            s,
            Exec::DesktopFile("/var/lib/flatpak/exports/share/applications/com.jetbrains.CLion.desktop".into())
        );

        let s = find_dummy("firefox", "", "org.mozilla.firefox").expect("finding firefox");

        assert_eq!(
            s,
            Exec::DesktopFile("/var/lib/flatpak/exports/share/applications/org.mozilla.firefox.desktop".into())
        );
    }

    #[test]
    fn find_gnome_app() {
        let s = find_dummy("gnome-terminal-server", "org.gnome.Terminal", "").expect("finding gnome terminal");

        assert_eq!(
            s,
            Exec::DesktopFile("/usr/share/applications/org.gnome.Terminal.desktop".into())
        );
    }

    #[test]
    fn find_lutris_app() {
        let s = find_dummy("org.multimc.MultiMC", "", "").expect("finding multimc");

        dbg!(
            strsim::normalized_levenshtein("org.multimc.MultiMC", "net.lutris.multimc-2"),
            strsim::normalized_levenshtein("org.multimc.MultiMC", "org.gimp.GIMP")
        );

        assert_eq!(
            s,
            Exec::DesktopFile("/home/liss/.local/share/applications/net.lutris.multimc-2.desktop".into())
        );

        let s = find_dummy("battle.net.exe", "", "").expect("finding battlenet");

        assert_eq!(
            s,
            Exec::DesktopFile("/home/liss/.local/share/applications/net.lutris.battlenet-7.desktop".into())
        );
    }

    #[test]
    fn sim_test() {
        dbg!(strsim::normalized_levenshtein(
            "chromium-freeworld",
            "chrome-discord.com__app-discord"
        ));
        dbg!(strsim::normalized_levenshtein(
            "chromium-freeworld",
            "chrome-listen.tidal.com__-Spotify"
        ));
        dbg!(strsim::normalized_levenshtein("java", "jetbrains-clion"));
    }
}
