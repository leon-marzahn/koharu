#[cfg(not(target_os = "macos"))]
use keyring_core::Entry;
use secrecy::{SecretBox, zeroize::Zeroize};
use serde::{Deserialize, Serialize, Serializer};
#[cfg(not(target_os = "macos"))]
use std::sync::LazyLock;

pub use secrecy::{ExposeSecret, SerializableSecret};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct SecretString(SecretBox<SecretValue>);

impl Default for SecretString {
    fn default() -> Self {
        Self::from(String::new())
    }
}

impl From<String> for SecretString {
    fn from(value: String) -> Self {
        Self(SecretBox::new(Box::new(SecretValue(value))))
    }
}

impl From<&str> for SecretString {
    fn from(value: &str) -> Self {
        Self::from(value.to_owned())
    }
}

impl ExposeSecret<str> for SecretString {
    fn expose_secret(&self) -> &str {
        &self.0.expose_secret().0
    }
}

#[derive(Clone, Deserialize)]
#[serde(transparent)]
struct SecretValue(String);

impl Zeroize for SecretValue {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

impl secrecy::CloneableSecret for SecretValue {}

impl Serialize for SecretValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str("[REDACTED]")
    }
}

impl SerializableSecret for SecretValue {}

const SERVICE: &str = "koharu";

#[cfg(not(target_os = "macos"))]
static CREDENTIAL_STORE: LazyLock<Result<(), String>> = LazyLock::new(|| {
    #[cfg(target_os = "linux")]
    {
        linux_keyutils_keyring_store::Store::new()
            .map(|store| keyring_core::set_default_store(store))
            .map_err(|error| error.to_string())
    }
    #[cfg(not(target_os = "linux"))]
    Ok(())
});

#[cfg(not(target_os = "macos"))]
/// Load a Koharu secret by key, returning `None` when no credential exists.
pub fn get(key: &str) -> anyhow::Result<Option<SecretString>> {
    let entry = entry(key)?;
    match entry.get_password() {
        Ok(value) => Ok(Some(SecretString::from(value))),
        Err(keyring_core::Error::NoEntry) => Ok(None),
        Err(error) => Err(error.into()),
    }
}

#[cfg(not(target_os = "macos"))]
/// Store a Koharu secret by key.
pub fn set(key: &str, secret: &SecretString) -> anyhow::Result<()> {
    entry(key)?.set_password(secret.expose_secret())?;
    Ok(())
}

#[cfg(not(target_os = "macos"))]
/// Delete a Koharu secret by key. Missing credentials are treated as success.
pub fn delete(key: &str) -> anyhow::Result<()> {
    match entry(key)?.delete_credential() {
        Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(not(target_os = "macos"))]
fn entry(key: &str) -> anyhow::Result<Entry> {
    match &*CREDENTIAL_STORE {
        Ok(()) => {}
        Err(error) => anyhow::bail!("failed to initialize Linux Keyutils: {error}"),
    }
    #[cfg(target_os = "linux")]
    let entry = Entry::new(SERVICE, key)?;
    #[cfg(not(target_os = "linux"))]
    let entry = keyring::Entry::new(SERVICE, key)?.inner;
    Ok(entry)
}

// ponytail: macOS goes through /usr/bin/security (it is on the item's `apple-tool:` partition), so
// rebuilding the app never re-prompts for the keychain. Upgrade: a Developer ID and `keyring`.
#[cfg(target_os = "macos")]
pub use macos::{delete, get, set};

#[cfg(target_os = "macos")]
mod macos {
    use super::{ExposeSecret, SERVICE, SecretString};
    use anyhow::{Context, bail};
    use std::io::Write;
    use std::process::{Command, Output, Stdio};

    const SECURITY: &str = "/usr/bin/security";
    const ITEM_NOT_FOUND: i32 = 44; // errSecItemNotFound

    fn failed(what: &str, out: &Output) -> anyhow::Error {
        anyhow::anyhow!(
            "security {what} failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        )
    }

    pub fn get(key: &str) -> anyhow::Result<Option<SecretString>> {
        // -g prints `password: "text"`, or `password: 0xHEX  "text"` for non-ASCII, on stderr.
        // (-w would hex-encode too, but then a plain all-hex key could not be told apart.)
        let out = Command::new(SECURITY)
            .args(["find-generic-password", "-s", SERVICE, "-a", key, "-g"])
            .output()
            .context("failed to run security")?;
        match out.status.code() {
            Some(0) => {}
            Some(ITEM_NOT_FOUND) => return Ok(None),
            _ => return Err(failed("find-generic-password", &out)),
        }
        let stderr = String::from_utf8_lossy(&out.stderr);
        let line = stderr
            .lines()
            .find_map(|line| line.strip_prefix("password: "))
            .context("security printed no password")?;
        if let Some(hex) = line.strip_prefix("0x") {
            let hex = hex.split_whitespace().next().unwrap_or_default();
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(hex.get(i..i + 2).unwrap_or(""), 16))
                .collect::<Result<Vec<u8>, _>>()
                .context("security printed invalid hex")?;
            return Ok(Some(SecretString::from(String::from_utf8(bytes)?)));
        }
        let quoted = line
            .strip_prefix('"')
            .and_then(|rest| rest.strip_suffix('"'));
        Ok(Some(SecretString::from(
            quoted.context("unexpected password format")?,
        )))
    }

    pub fn set(key: &str, secret: &SecretString) -> anyhow::Result<()> {
        let value = secret.expose_secret();
        if value.contains(['\n', '\r', '\0']) {
            bail!("a secret must be a single line");
        }
        // `security -i` reads the command from stdin, so the secret never shows in the process list.
        // -A lets any app read the item; -U updates it in place.
        let mut child = Command::new(SECURITY)
            .arg("-i")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .context("failed to run security")?;
        let mut stdin = child.stdin.take().context("no stdin")?;
        writeln!(
            stdin,
            "add-generic-password -U -A -s {SERVICE} -a {} -w {}",
            quote(key),
            quote(value)
        )?;
        drop(stdin);
        let out = child.wait_with_output()?;
        // Interactive mode exits 0 even when the command fails, so stderr is the signal.
        if !out.status.success() || !out.stderr.is_empty() {
            return Err(failed("add-generic-password", &out));
        }
        Ok(())
    }

    pub fn delete(key: &str) -> anyhow::Result<()> {
        let out = Command::new(SECURITY)
            .args(["delete-generic-password", "-s", SERVICE, "-a", key])
            .output()
            .context("failed to run security")?;
        match out.status.code() {
            Some(0) | Some(ITEM_NOT_FOUND) => Ok(()),
            _ => Err(failed("delete-generic-password", &out)),
        }
    }

    fn quote(text: &str) -> String {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn round_trips_awkward_secrets_without_a_prompt() {
            let key = format!("koharu-secrets-test-{}", std::process::id());
            let value = r#"a "b" \c d'e é $x `y`"#;
            delete(&key).unwrap();
            assert!(get(&key).unwrap().is_none());
            set(&key, &SecretString::from("first")).unwrap();
            set(&key, &SecretString::from(value)).unwrap();
            assert_eq!(get(&key).unwrap().unwrap().expose_secret(), value);
            delete(&key).unwrap();
            assert!(get(&key).unwrap().is_none());
            assert!(set(&key, &SecretString::from("a\nb")).is_err());
        }
    }
}
