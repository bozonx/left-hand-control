//! Schema versions of `config.json` and layout files.
//!
//! The two formats are versioned independently. Within a version only new
//! optional fields and new enum values may appear; any other change bumps
//! that format's version. To introduce version N + 1 of a format:
//!
//! 1. move the current `*Doc` structs into a `vN` submodule next to the
//!    format's reader and add the new ones;
//! 2. bump the constant below;
//! 3. add a `N => …` arm to the reader's version `match` that parses the
//!    old structs and converts them to the domain model (or to the new
//!    structs).
//!
//! The application always writes the current version. Before a file with
//! an older version is first overwritten, the storage keeps a copy of it
//! next to the original (see [`backup_path`]).

/// Version of `config.json` this build reads and writes.
pub const CONFIG_SCHEMA_VERSION: u64 = 1;

/// Version of layout files this build reads and writes.
pub const LAYOUT_SCHEMA_VERSION: u64 = 1;

/// The `schemaVersion` of a parsed document, checked before the rest is
/// read, so a file from a newer build is reported as such rather than as a
/// list of unknown fields. Returns a version in `1..=current`.
pub(crate) fn read_version(
    value: &serde_json::Value,
    what: &str,
    current: u64,
) -> Result<u64, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("{what} must be a JSON object"))?;
    let Some(version) = object.get("schemaVersion") else {
        return Err(format!("{what}: missing \"schemaVersion\" field"));
    };
    match version.as_u64() {
        Some(version) if (1..=current).contains(&version) => Ok(version),
        Some(newer) if newer > current => Err(format!(
            "{what}: schema version {newer} is newer than this application supports ({current})"
        )),
        _ => Err(format!("{what}: unsupported schema version {version}")),
    }
}

/// `schemaVersion` of a file's text, if it is a JSON object with one.
pub(crate) fn version_of(text: &str) -> Option<u64> {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()?
        .get("schemaVersion")?
        .as_u64()
}

/// A strict-read error. A version may gain optional fields and enum
/// values, which an older build reports as unknown, so such errors point at
/// the likely cause.
pub(crate) fn read_error(what: &str, error: &serde_json::Error) -> String {
    let text = error.to_string();
    if text.starts_with("unknown field") || text.starts_with("unknown variant") {
        format!("{what}: {text} (the file may come from a newer version of the application)")
    } else {
        format!("{what}: {text}")
    }
}

/// Where the copy of `path` with schema `version` is kept before the file
/// is upgraded: `config.json` → `config.json.v1.bak`.
pub fn backup_path(path: &std::path::Path, version: u64) -> std::path::PathBuf {
    let mut name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_default();
    name.push(format!(".v{version}.bak"));
    path.with_file_name(name)
}

/// Read an optional field whose absence is allowed but explicit null is not.
pub(crate) fn non_null<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn accepts_every_version_up_to_the_current_one() {
        let read = |value, current| read_version(&value, "file", current);
        assert_eq!(read(json!({"schemaVersion": 1}), 1), Ok(1));
        assert_eq!(read(json!({"schemaVersion": 1}), 3), Ok(1));
        assert_eq!(read(json!({"schemaVersion": 3}), 3), Ok(3));
        assert!(
            read(json!({"schemaVersion": 2}), 1)
                .unwrap_err()
                .contains("newer")
        );
        assert!(
            read(json!({"schemaVersion": 0}), 1)
                .unwrap_err()
                .contains("unsupported")
        );
        assert!(read(json!({"schemaVersion": "1"}), 1).is_err());
        assert!(read(json!({}), 1).unwrap_err().contains("missing"));
        assert!(read(json!([]), 1).unwrap_err().contains("JSON object"));
    }

    #[test]
    fn reads_the_version_of_raw_text() {
        assert_eq!(version_of(r#"{"schemaVersion": 4, "x": 1}"#), Some(4));
        assert_eq!(version_of(r#"{"x": 1}"#), None);
        assert_eq!(version_of("not json"), None);
    }

    #[test]
    fn backup_sits_next_to_the_file() {
        assert_eq!(
            backup_path(Path::new("/c/layouts/Main.json"), 1),
            Path::new("/c/layouts/Main.json.v1.bak")
        );
    }
}
