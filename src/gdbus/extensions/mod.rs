pub mod extension_info;

use super::{Error, Result};
use extension_info::ExtensionInfo;
use zbus::{Connection, Proxy};

const DBUS_DEST: &str = "org.gnome.Shell.Extensions";
const DBUS_PATH: &str = "/org/gnome/Shell/Extensions";

pub struct GnomeShellExtensionsDBusProxy<'a> {
    dbus_conn: Proxy<'a>,
}

impl<'a> GnomeShellExtensionsDBusProxy<'a> {
    pub fn new(connection: &'a Connection) -> Result<Self> {
        Ok(GnomeShellExtensionsDBusProxy {
            dbus_conn: Proxy::new(connection, DBUS_DEST, DBUS_PATH, DBUS_DEST)?,
        })
    }
}

impl<'a> GnomeShellExtensionsDBusProxy<'a> {
    pub fn enable_extension(&self, uuid: &str) -> Result<()> {
        let success: bool = self.dbus_conn.call("EnableExtension", &uuid)?;

        if success {
            Ok(())
        } else {
            Err(Error::Shell("extension does not exist".to_owned()))
        }
    }

    pub fn disable_extension(&self, uuid: &str) -> Result<()> {
        let success: bool = self.dbus_conn.call("DisableExtension", &uuid)?;

        if success {
            Ok(())
        } else {
            Err(Error::Shell("extension does not exist".to_owned()))
        }
    }

    pub fn get_extension_info(&self, uuid: &str) -> Result<ExtensionInfo> {
        let info: zbus::Result<ExtensionInfo> = self.dbus_conn.call("GetExtensionInfo", &uuid);

        match info {
            Ok(info) => Ok(info),
            Err(zbus::Error::Message(zbus::MessageError::Variant(zvariant::Error::Message(
                msg,
            )))) if msg.starts_with("missing field") => {
                Err(Error::Shell("extension does not exist".to_owned()))
            }
            Err(e) => Err(Error::DBus(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::extension_info::ExtensionInfo;
    use super::GnomeShellExtensionsDBusProxy;
    use std::collections::HashMap;
    use zbus::Connection;
    use zvariant::Type;

    #[test]
    fn introspect() {
        let conn = Connection::new_session().unwrap();
        let ext = GnomeShellExtensionsDBusProxy::new(&conn).unwrap();

        println!("{}", ext.dbus_conn.introspect().unwrap());
    }

    #[test]
    fn ext_info() {
        let conn = Connection::new_session().unwrap();
        let ext = GnomeShellExtensionsDBusProxy::new(&conn).unwrap();

        println!(
            "{:#?}",
            ext.get_extension_info("fullscreen-avoider@noobsai.github.com")
                .unwrap()
        );
    }
}
