use std::ffi::OsString;
use std::io::{self, ErrorKind};
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use winreg::RegKey;
use winreg::enums::{HKEY_CURRENT_USER, KEY_SET_VALUE};

const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const APPROVED: &str = r"Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run";
const NAME: &str = "Tether";
pub(super) const ARG: &str = "--start-in-tray";

fn command(exe: &Path) -> OsString {
    let mut value = OsString::from("\"");
    value.push(exe);
    value.push("\" ");
    value.push(ARG);
    value
}

fn optional<T>(value: io::Result<T>) -> io::Result<Option<T>> {
    match value {
        Ok(value) => Ok(Some(value)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn registered(root: &RegKey, exe: &Path) -> io::Result<bool> {
    let value = optional(
        root.open_subkey(RUN)
            .and_then(|key| key.get_value::<OsString, _>(NAME)),
    )?;
    Ok(value.as_ref() == Some(&command(exe)))
}

#[derive(serde::Serialize)]
pub(super) struct StartupState {
    enabled: bool,
    blocked: bool,
}

pub(super) fn state() -> Result<StartupState, &'static str> {
    let root = RegKey::predef(HKEY_CURRENT_USER);
    let exe = std::env::current_exe().map_err(|_| "Could not locate Tether.exe.")?;
    let enabled =
        registered(&root, &exe).map_err(|_| "Could not read Windows startup settings.")?;
    let mut blocked = false;
    if enabled {
        let approval = optional(
            root.open_subkey(APPROVED)
                .and_then(|key| key.get_raw_value(NAME)),
        )
        .map_err(|_| "Could not read Windows startup settings.")?;
        blocked = approval.is_some_and(|value| !matches!(value.bytes.first(), Some(2 | 6)));
    }
    Ok(StartupState { enabled, blocked })
}

fn write(root: &RegKey, exe: &Path, enabled: bool) -> io::Result<()> {
    if enabled {
        let value = command(exe);
        // Windows limits a Run command line to 260 characters including quotes and arguments.
        if value.encode_wide().count() >= 260 {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "Executable path too long",
            ));
        }
        root.create_subkey_with_flags(RUN, KEY_SET_VALUE)?
            .0
            .set_value(NAME, &value)?;
    } else if registered(root, exe)? {
        optional(
            root.open_subkey_with_flags(RUN, KEY_SET_VALUE)?
                .delete_value(NAME),
        )?;
    }
    Ok(())
}

pub(super) fn set(enabled: bool) -> Result<StartupState, &'static str> {
    let exe = std::env::current_exe().map_err(|_| "Could not locate Tether.exe.")?;
    write(&RegKey::predef(HKEY_CURRENT_USER), &exe, enabled).map_err(
        |_| "Windows could not change the sign-in setting. Check the location of Tether.exe.",
    )?;
    state()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_in_command_quotes_the_entire_executable() {
        let exe = Path::new(r"C:\My apps\Tether.exe");
        assert_eq!(
            command(exe),
            OsString::from(r#""C:\My apps\Tether.exe" --start-in-tray"#)
        );
    }

    #[test]
    fn toggling_only_removes_the_registered_copy() -> io::Result<()> {
        // A disposable registry subtree stands in for HKCU. Windows never
        // processes Run entries below this test root.
        let user = RegKey::predef(HKEY_CURRENT_USER);
        let name = format!(r"Software\TetherStartupTest-{}", std::process::id());
        let (root, disposition) = user.create_subkey(&name)?;
        assert_eq!(disposition, winreg::enums::REG_CREATED_NEW_KEY);
        let result = (|| -> io::Result<()> {
            let exe = Path::new(r"C:\Program Files\Tether\Tether.exe");
            let other = Path::new(r"D:\Tether.exe");
            assert!(!registered(&root, exe)?);
            write(&root, exe, true)?;
            assert!(registered(&root, exe)?);
            write(&root, other, false)?;
            assert!(registered(&root, exe)?);
            write(&root, exe, false)?;
            write(&root, exe, false)?;
            assert!(!registered(&root, exe)?);
            Ok(())
        })();
        drop(root);
        user.delete_subkey_all(name)?;
        result
    }
}
