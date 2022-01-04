use serde::de::{Deserialize, Deserializer, Error, Unexpected, Visitor};
use std::fmt::Formatter;
use zbus::export::zvariant::Signature;
use zvariant::Type;
use zvariant_derive::{DeserializeDict, TypeDict};

fn f64_try_into_i32(v: f64) -> Result<i32, ()> {
    if v.trunc() == v && v >= f64::from(i32::MIN) && v <= f64::from(i32::MAX) {
        Ok(v as i32)
    } else {
        Err(())
    }
}

#[derive(Debug, Copy, Clone)]
pub enum ExtensionState {
    Enabled,
    Disabled,
    Error,
    OutOfDate,
    Downloading,
    Initialized,
    Uninstalled,
}

#[derive(Debug, Copy, Clone)]
pub enum ExtensionType {
    System,
    PerUser,
}

#[derive(Debug, DeserializeDict, TypeDict)]
pub struct ExtensionInfo {
    pub uuid: String,
    pub name: String,
    pub description: String,
    pub url: String,
    pub version: f64,
    pub state: ExtensionState,
    pub path: String,
    pub error: String,

    #[zvariant(rename = "extension-id")]
    pub extension_id: String,

    #[zvariant(rename = "gettext-domain")]
    pub gettext_domain: String,

    #[zvariant(rename = "type")]
    pub exttype: ExtensionType,

    #[zvariant(rename = "hasPrefs")]
    pub has_prefs: bool,

    #[zvariant(rename = "hasUpdate")]
    pub has_update: bool,

    #[zvariant(rename = "canChange")]
    pub can_change: bool,
}

impl Type for ExtensionState {
    fn signature() -> Signature<'static> {
        f64::signature()
    }
}

impl<'de> Deserialize<'de> for ExtensionState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ExtStateVisitor;

        impl Visitor<'_> for ExtStateVisitor {
            type Value = ExtensionState;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                write!(formatter, "f64 representing a gnome shell extension state")
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                use ExtensionState::*;

                match f64_try_into_i32(v) {
                    Ok(1) => Ok(Enabled),
                    Ok(2) => Ok(Disabled),
                    Ok(3) => Ok(Error),
                    Ok(4) => Ok(OutOfDate),
                    Ok(5) => Ok(Downloading),
                    Ok(6) => Ok(Initialized),
                    Ok(99) => Ok(Uninstalled),
                    _ => Err(serde::de::Error::invalid_value(Unexpected::Float(v), &self)),
                }
            }
        }

        deserializer.deserialize_f64(ExtStateVisitor)
    }
}

impl Type for ExtensionType {
    fn signature() -> Signature<'static> {
        f64::signature()
    }
}

impl<'de> Deserialize<'de> for ExtensionType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ExtTypeVisitor;

        impl Visitor<'_> for ExtTypeVisitor {
            type Value = ExtensionType;

            fn expecting(&self, formatter: &mut Formatter) -> std::fmt::Result {
                write!(formatter, "f64 representing a gnome shell extension type")
            }

            fn visit_f64<E>(self, v: f64) -> Result<Self::Value, E>
            where
                E: Error,
            {
                use ExtensionType::*;

                match f64_try_into_i32(v) {
                    Ok(1) => Ok(System),
                    Ok(2) => Ok(PerUser),
                    _ => Err(serde::de::Error::invalid_value(Unexpected::Float(v), &self)),
                }
            }
        }

        deserializer.deserialize_f64(ExtTypeVisitor)
    }
}
