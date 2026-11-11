//! Integrates local application settings.
use std::{collections::BTreeMap, fs, path::PathBuf};

use serde_json::Value;

use crate::{config, Error, Result};

const NO_PROXY_KEY: &str = "http.noProxy";
/// Pre-takeover values of every key takeover touches (`null` = key was absent).
const BACKUP_FILE: &str = "cursor-settings-original.json";
const KEYS: [&str; 5] = [
    "http.proxy",
    "http.proxyKerberosServicePrincipal",
    "http.proxySupport",
    "cursor.general.disableHttp2",
    "http.experimental.systemCertificatesV2",
];

fn path() -> Result<PathBuf> {
    let home = dirs::home_dir()
        .ok_or_else(|| Error::Config("cannot resolve user home directory".into()))?;
    match std::env::consts::OS {
        "macos" => Ok(home.join("Library/Application Support/Cursor/User/settings.json")),
        "windows" => Ok(std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("AppData/Roaming"))
            .join("Cursor/User/settings.json")),
        "linux" => Ok(std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".config"))
            .join("Cursor/User/settings.json")),
        platform => Err(Error::Config(format!(
            "Cursor settings are unsupported on {platform}"
        ))),
    }
}

fn read() -> Result<BTreeMap<String, Value>> {
    let path = path()?;
    let data = match fs::read_to_string(path) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(error.into()),
    };
    if data.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    json5::from_str(&data)
        .map_err(|error| Error::Config(format!("parse Cursor settings JSONC: {error}")))
}

fn write(settings: &BTreeMap<String, Value>) -> Result<()> {
    let path = path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_vec_pretty(settings)?;
    let temp = path.with_extension("json.tmp");
    fs::write(&temp, [data.as_slice(), b"\n"].concat())?;
    fs::rename(temp, path)?;
    Ok(())
}

fn backup_path() -> Result<PathBuf> {
    Ok(config::managed_data_dir()?.join(BACKUP_FILE))
}

/// Snapshots the keys takeover overwrites, once per takeover: while the managed settings are
/// already in place the existing snapshot is kept.
fn backup(settings: &BTreeMap<String, Value>) -> Result<()> {
    if is_managed(settings) && backup_path()?.exists() {
        return Ok(());
    }
    let original: BTreeMap<&str, Option<&Value>> = std::iter::once(NO_PROXY_KEY)
        .chain(KEYS)
        .map(|key| (key, settings.get(key).filter(|_| !is_managed(settings))))
        .collect();
    fs::write(backup_path()?, serde_json::to_vec_pretty(&original)?)?;
    Ok(())
}

pub fn write_proxy_settings(proxy_url: &str) -> Result<()> {
    let mut settings = read()?;
    backup(&settings)?;
    settings.remove(NO_PROXY_KEY);
    settings.insert(KEYS[0].into(), Value::String(proxy_url.into()));
    settings.insert(KEYS[1].into(), Value::String(proxy_url.into()));
    settings.insert(KEYS[2].into(), Value::String("on".into()));
    settings.insert(KEYS[3].into(), Value::Bool(true));
    settings.insert(KEYS[4].into(), Value::Bool(true));
    write(&settings)
}

/// Puts Cursor's settings back to their pre-takeover values. Does nothing unless the
/// takeover settings are still in place, so later manual edits are never overwritten.
pub fn restore() -> Result<()> {
    let mut settings = read()?;
    let backup_path = backup_path()?;
    if is_managed(&settings) {
        let original: BTreeMap<String, Option<Value>> = match fs::read(&backup_path) {
            Ok(data) => serde_json::from_slice(&data)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(error.into()),
        };
        for key in std::iter::once(NO_PROXY_KEY).chain(KEYS) {
            match original.get(key).cloned().flatten() {
                Some(value) => settings.insert(key.into(), value),
                None => settings.remove(key),
            };
        }
        write(&settings)?;
    }
    match fs::remove_file(backup_path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

pub fn settings_match(proxy_url: &str) -> Result<bool> {
    let settings = read()?;
    Ok(
        settings.get(KEYS[0]) == Some(&Value::String(proxy_url.into()))
            && settings.get(KEYS[1]) == Some(&Value::String(proxy_url.into()))
            && settings.get(KEYS[2]) == Some(&Value::String("on".into()))
            && settings.get(KEYS[3]) == Some(&Value::Bool(true))
            && settings.get(KEYS[4]) == Some(&Value::Bool(true)),
    )
}

fn is_managed(settings: &BTreeMap<String, Value>) -> bool {
    let managed_signature = settings.get(KEYS[2]) == Some(&Value::String("on".into()))
        && settings.get(KEYS[3]) == Some(&Value::Bool(true))
        && settings.get(KEYS[4]) == Some(&Value::Bool(true));
    let loopback = settings
        .get(KEYS[0])
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<reqwest::Url>().ok())
        .and_then(|url| url.host_str().map(str::to_owned))
        .is_some_and(|host| matches!(host.as_str(), "127.0.0.1" | "localhost" | "::1"));
    managed_signature && loopback
}
