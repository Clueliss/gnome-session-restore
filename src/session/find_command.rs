use regex::Regex;
use serde::Deserialize;
use std::{ffi::OsString, fs::File, ops::Deref, path::Path};
use thiserror::Error;

const DESKTOP_ENTRY_LOCATIONS: [&str; 4] = [
    "/usr/share/applications",
    "/usr/local/share/application",
    "~/.local/share/applications",
    "/var/lib/flatpak/exports/share/applications",
];

#[derive(Error, Debug)]
pub enum FindError {
    #[error("io error")]
    IOError(#[from] std::io::Error),

    #[error("could not find a suitable entry")]
    NoSuitableEntryFound,

    #[error("could only find invalid entry")]
    InvalidEntryFound,
}

fn find_main_exec_entry<P: AsRef<Path>>(path: P) -> Result<Vec<String>, FindError> {
    #[derive(Deserialize, Debug)]
    struct MainSection {
        #[serde(rename = "Exec")]
        exec: String,
    }

    #[derive(Deserialize, Debug)]
    struct DesktopEntry {
        #[serde(rename = "Desktop Entry")]
        desktop_entry: MainSection,
    }

    let f = File::open(path)?;
    let de: DesktopEntry = serde_ini::from_read(f).map_err(|_| FindError::InvalidEntryFound)?;

    let cmdline = shell_words::split(&de.desktop_entry.exec)
        .map_err(|_| FindError::InvalidEntryFound)?
        .into_iter()
        .filter(|s| !["%u", "%U", "%f", "%F"].contains(&s.deref()))
        .collect();

    Ok(cmdline)
}

pub fn try_find_command_by_gtk_app_id(gtk_app_id: &str) -> Result<Vec<String>, FindError> {
    let p = Path::new(DESKTOP_ENTRY_LOCATIONS[0])
        .join(format!("{gtk_app_id}.desktop", gtk_app_id = gtk_app_id));

    find_main_exec_entry(p)
}

fn is_rhs_less_complex(x: Option<&str>, y: &str) -> bool {
    match x {
        None => true,
        Some(x) => y.len() < x.len(),
    }
}

pub fn try_find_command_by_window_class(
    w_class: &str,
    alt_w_class: Option<&str>,
) -> Result<Vec<String>, FindError> {
    let re = alt_w_class
        .and_then(|awc| {
            Regex::new(&format!(
                r#"({window_class}|{alt_window_class})(-.*?)*?\.desktop"#,
                window_class = w_class.to_lowercase(),
                alt_window_class = awc.to_lowercase()
            )).ok()
        })
        .or_else(|| {
            Regex::new(&format!(
                r#"{window_class}(-.*?)*?\.desktop"#,
                window_class = w_class.to_lowercase()
            )).ok()
        })
        .unwrap();

    let mut match_filename = None;
    let mut match_location = None;

    for location in DESKTOP_ENTRY_LOCATIONS
        .iter()
        .map(|s| shellexpand::tilde(s))
    {
        let files = match std::fs::read_dir(location.as_ref()) {
            Ok(files) => files,
            Err(_) => continue,
        };

        for file in files {
            let file = file?;
            let filename = file.file_name();
            let filename_str = filename.to_string_lossy().to_ascii_lowercase();

            if re.is_match(&filename_str) {
                let mfm_str = match_filename
                    .as_ref()
                    .map(|f: &OsString| f.to_string_lossy());

                if is_rhs_less_complex(mfm_str.as_deref(), &filename_str) {
                    match_location = Some(location.to_owned());
                    match_filename = Some(filename);
                }
            }
        }
    }

    match match_location.zip(match_filename) {
        Some((location, filename)) => {
            find_main_exec_entry(Path::new(location.as_ref()).join(filename))
        }
        None => Err(FindError::NoSuitableEntryFound),
    }
}

pub fn find_command_in_proc(pid: i32) -> std::io::Result<Vec<String>> {
    let content: Vec<_> = std::fs::read_to_string(format!("/proc/{pid}/cmdline", pid = pid))?
        .split_terminator('\0')
        .map(ToString::to_string)
        .collect();

    if content.len() == 1 && content[0].contains(' ') && which::which(&content[0]).is_err() {
        Ok(content[0].split(' ').map(ToString::to_string).collect())
    } else {
        Ok(content)
    }
}

pub fn find_command(
    pid: i32,
    window_class: &str,
    gtk_app_id: Option<&str>,
) -> Result<Vec<String>, FindError> {
    if let Some(gtk_app_id) = gtk_app_id {
        if let Ok(cmdline) = try_find_command_by_gtk_app_id(gtk_app_id) {
            println!("{} from gtk app id", window_class);
            return Ok(cmdline);
        }
    }

    let proc_cmdline = find_command_in_proc(pid)?;
    let alt_window_class = Path::new(&proc_cmdline[0])
        .file_name()
        .map(|f| f.to_string_lossy());

    if let Ok(cmdline) = try_find_command_by_window_class(window_class, alt_window_class.as_deref()) {
        println!("{} from desktop entry", window_class);
        return Ok(cmdline);
    }

    println!("{} from proc", window_class);
    Ok(proc_cmdline)
}
