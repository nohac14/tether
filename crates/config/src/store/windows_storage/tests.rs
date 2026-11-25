use super::*;
use crate::SettingsStore;
use std::process::Command;
use tether_test_support::PrivateTempDir;

#[test]
fn rejects_inherited_directory_permissions() {
    let test = PrivateTempDir::new().unwrap();
    let root = test.path().join("config");
    fs::create_dir(&root).unwrap();
    assert_eq!(
        SettingsStore::open(&root).map(|_| ()),
        Err(StorageError::PlatformPermissionEnforcementUnavailable)
    );
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
}

#[test]
fn rejects_an_extra_principal_even_in_a_protected_acl() {
    let test = PrivateTempDir::new().unwrap();
    let root = test.path().join("config");
    let user = current_user_sid().unwrap();
    let descriptor = descriptor_from_sddl(&format!(
        "O:{user}D:P(A;OICI;FA;;;{user})(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;;FR;;;WD)"
    ))
    .unwrap();
    let security = attributes(&descriptor);
    let name = wide_path(&root).unwrap();
    // SAFETY: these buffers are live for the call, and the operation only
    // creates this test's empty temporary directory.
    unsafe { CreateDirectoryW(PCWSTR(name.as_ptr()), Some(&raw const security)) }.unwrap();
    assert_eq!(
        SettingsStore::open(&root).map(|_| ()),
        Err(StorageError::PlatformPermissionEnforcementUnavailable)
    );
    assert_eq!(fs::read_dir(root).unwrap().count(), 0);
}

#[test]
fn settings_with_inherited_permissions_are_refused_before_processing() {
    let test = PrivateTempDir::new().unwrap();
    let root = test.path().join("config");
    let (_, guards) = prepare_root(&root).unwrap();
    drop(guards);
    let path = root.join("settings.json");
    let fixture = b"unauthorized synthetic file";
    fs::write(&path, fixture).unwrap();
    assert_eq!(
        SettingsStore::open(&root).map(|_| ()),
        Err(StorageError::PlatformPermissionEnforcementUnavailable)
    );
    assert_eq!(fs::read(path).unwrap(), fixture);
    assert!(!root.join("identity.json").exists());
}

#[test]
fn settings_hard_link_is_rejected_without_changing_either_entry() {
    let test = PrivateTempDir::new().unwrap();
    let root = test.path().join("config");
    let store = SettingsStore::open(&root).unwrap();
    drop(store);
    let original = root.join("settings.json");
    let alias = test.path().join("alias.json");
    let original_bytes = fs::read(&original).unwrap();
    fs::hard_link(&original, &alias).unwrap();
    assert!(matches!(
        SettingsStore::open(&root),
        Err(StorageError::Document {
            problem: StorageProblem::HardLinkDetected,
            ..
        })
    ));
    assert_eq!(fs::read(original).unwrap(), original_bytes);
    assert_eq!(fs::read(alias).unwrap(), original_bytes);
}

#[test]
fn rejects_junctions_as_root_or_an_ancestor() {
    let test = PrivateTempDir::new().unwrap();
    let target = test.path().join("target");
    let junction = test.path().join("junction");
    fs::create_dir(&target).unwrap();
    fs::create_dir(target.join("nested")).unwrap();
    let made = Command::new("cmd.exe")
        .args(["/D", "/C", "mklink", "/J", "junction", "target"])
        .current_dir(test.path())
        .output()
        .unwrap();
    assert!(made.status.success(), "create temporary NTFS junction");
    for path in [&junction, &junction.join("nested").join("config")] {
        assert!(matches!(
            SettingsStore::open(path),
            Err(StorageError::Document {
                problem: StorageProblem::UnsafeEntryKind,
                ..
            })
        ));
    }
    fs::remove_dir(junction).unwrap();
    assert_eq!(fs::read_dir(target.join("nested")).unwrap().count(), 0);
}

#[test]
fn root_guard_blocks_rename_until_store_drops() {
    let test = PrivateTempDir::new().unwrap();
    let root = test.path().join("config");
    let moved = test.path().join("moved");
    let (store, _) = SettingsStore::open(&root).unwrap();
    assert!(fs::rename(&root, &moved).is_err());
    drop(store);
    fs::rename(root, moved).unwrap();
}
