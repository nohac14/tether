//! Windows storage boundary approved by ADR 0005.
#![allow(unsafe_code)]

use super::{
    QUARANTINE_DIRECTORY_NAME, StorageError, StorageFileKind, StorageIoOperation, StorageProblem,
    document_error, io_error,
};
use std::ffi::{OsString, c_void};
use std::fs::{self, File, OpenOptions};
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::{MetadataExt, OpenOptionsExt};
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
use std::path::{Component, Path, PathBuf, Prefix};
use std::ptr;
use windows::Win32::Foundation::{
    ERROR_SHARING_VIOLATION, ERROR_SUCCESS, GENERIC_READ, GENERIC_WRITE, HANDLE, HLOCAL, LocalFree,
};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo,
    SDDL_REVISION_1, SE_FILE_OBJECT,
};
use windows::Win32::Security::{
    ACL, ACL_SIZE_INFORMATION, AclSizeInformation, DACL_SECURITY_INFORMATION, EqualSid,
    GetAclInformation, GetSecurityDescriptorControl, GetSecurityDescriptorDacl,
    GetSecurityDescriptorOwner, GetTokenInformation, IsValidAcl, OWNER_SECURITY_INFORMATION,
    PSECURITY_DESCRIPTOR, PSID, SE_DACL_PROTECTED, SECURITY_ATTRIBUTES, TOKEN_QUERY, TOKEN_USER,
    TokenUser,
};
use windows::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, CREATE_NEW, CreateDirectoryW, CreateFileW,
    FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_REPARSE_POINT, FILE_CREATION_DISPOSITION,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_NAME_NORMALIZED,
    FILE_READ_ATTRIBUTES, FILE_SHARE_MODE, FILE_SHARE_READ, FILE_SHARE_WRITE, FILE_TYPE_DISK,
    GetDriveTypeW, GetFileInformationByHandle, GetFileType, GetFinalPathNameByHandleW,
    GetVolumeInformationByHandleW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, MoveFileExW,
    OPEN_ALWAYS, READ_CONTROL,
};
use windows::Win32::System::SystemServices::FILE_PERSISTENT_ACLS;
use windows::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};
use windows::core::{BOOL, PCWSTR, PWSTR};

const MAX_PATH_UNITS: usize = 32_767;
const MAX_SID_STRING_UNITS: usize = 184;
const MAX_ACL_BYTES: u32 = 1_024;
// GetDriveTypeW's result for local fixed media.
const DRIVE_FIXED: u32 = 3;

pub(super) struct RootGuards {
    _quarantine: File,
    _root: File,
}

pub(super) fn prepare_root(root: &Path) -> Result<(PathBuf, RootGuards), StorageError> {
    if !root.is_absolute() || !root.components().all(permitted_component) {
        return Err(document_error(StorageProblem::UnsafeEntryKind));
    }
    let parent = root
        .parent()
        .ok_or_else(|| document_error(StorageProblem::UnsafeEntryKind))?;
    let name = root
        .file_name()
        .ok_or_else(|| document_error(StorageProblem::UnsafeEntryKind))?;
    let parent_handle = inspect_path(parent).map_err(|error| {
        io_error(
            StorageFileKind::Root,
            StorageIoOperation::InspectEntry,
            error,
        )
    })?;
    inspect(&parent_handle, true)?;
    verify_directory_ancestors(parent)?;
    let parent = final_path(&parent_handle).map_err(|error| {
        io_error(
            StorageFileKind::Root,
            StorageIoOperation::InspectEntry,
            error,
        )
    })?;
    check_local_acl_volume(&parent, &parent_handle)?;

    let root = parent.join(name);
    let root_guard = ensure_directory(&root, StorageFileKind::Root)?;
    let canonical = final_path(&root_guard).map_err(|error| {
        io_error(
            StorageFileKind::Root,
            StorageIoOperation::InspectEntry,
            error,
        )
    })?;
    if canonical.parent() != Some(parent.as_path()) {
        return Err(document_error(StorageProblem::UnsafeEntryKind));
    }
    let quarantine_path = canonical.join(QUARANTINE_DIRECTORY_NAME);
    let quarantine_guard = ensure_directory(&quarantine_path, StorageFileKind::Quarantine)?;
    Ok((
        canonical,
        RootGuards {
            _quarantine: quarantine_guard,
            _root: root_guard,
        },
    ))
}

fn verify_directory_ancestors(path: &Path) -> Result<(), StorageError> {
    for ancestor in path.ancestors() {
        let entry = fs::symlink_metadata(ancestor).map_err(|error| {
            io_error(
                StorageFileKind::Root,
                StorageIoOperation::InspectEntry,
                error,
            )
        })?;
        if !entry.is_dir() || entry.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
            return Err(document_error(StorageProblem::UnsafeEntryKind));
        }
    }
    Ok(())
}

fn permitted_component(component: Component<'_>) -> bool {
    match component {
        Component::Prefix(prefix) => {
            matches!(prefix.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
        }
        Component::RootDir => true,
        Component::Normal(name) => !name
            .encode_wide()
            .any(|unit| unit == u16::from(b':') || unit == 0),
        Component::CurDir | Component::ParentDir => false,
    }
}

fn ensure_directory(path: &Path, kind: StorageFileKind) -> Result<File, StorageError> {
    let descriptor = private_descriptor(true)
        .map_err(|_| StorageError::PlatformPermissionEnforcementUnavailable)?;
    let security = attributes(&descriptor);
    let name = wide_path(path)
        .map_err(|error| io_error(kind, StorageIoOperation::CreateDirectory, error))?;
    // SAFETY: name is NUL-terminated, and security points to a live self-relative
    // descriptor owned by this call. CreateDirectoryW borrows both arguments.
    let created = unsafe { CreateDirectoryW(PCWSTR(name.as_ptr()), Some(&raw const security)) };
    if let Err(error) = created.map_err(|error| win_error(error.code().0))
        && error.kind() != io::ErrorKind::AlreadyExists
    {
        return Err(io_error(kind, StorageIoOperation::CreateDirectory, error));
    }
    let directory = inspect_path(path)
        .map_err(|error| io_error(kind, StorageIoOperation::InspectEntry, error))?;
    inspect(&directory, true)?;
    verify_security(&directory, &descriptor)?;
    Ok(directory)
}

fn check_local_acl_volume(path: &Path, file: &File) -> Result<(), StorageError> {
    let Some(Component::Prefix(prefix)) = path.components().next() else {
        return Err(document_error(StorageProblem::UnsafeEntryKind));
    };
    let (Prefix::Disk(letter) | Prefix::VerbatimDisk(letter)) = prefix.kind() else {
        return Err(document_error(StorageProblem::UnsafeEntryKind));
    };
    let drive = [u16::from(letter), u16::from(b':'), u16::from(b'\\'), 0];
    // SAFETY: drive is a NUL-terminated root path.
    if unsafe { GetDriveTypeW(PCWSTR(drive.as_ptr())) } != DRIVE_FIXED {
        return Err(StorageError::PlatformPermissionEnforcementUnavailable);
    }
    let mut flags = 0;
    // SAFETY: file stays open, and flags is writable for the duration of the call.
    unsafe {
        GetVolumeInformationByHandleW(handle(file), None, None, None, Some(&raw mut flags), None)
    }
    .map_err(|_| StorageError::PlatformPermissionEnforcementUnavailable)?;
    if flags & FILE_PERSISTENT_ACLS == 0 {
        return Err(StorageError::PlatformPermissionEnforcementUnavailable);
    }
    Ok(())
}

pub(super) fn open_lock(path: &Path) -> Result<File, StorageError> {
    // The held handle denies all sharing. Inspect it in place, since opening
    // a second data handle would contradict the lifetime lock contract.
    let file = create_file(path, OPEN_ALWAYS, FILE_SHARE_MODE(0)).map_err(|error| {
        if error.raw_os_error() == i32::try_from(ERROR_SHARING_VIOLATION.0).ok() {
            return StorageError::LockHeld;
        }
        io_error(
            StorageFileKind::ProcessLock,
            StorageIoOperation::OpenLock,
            error,
        )
    })?;
    inspect(&file, false)?;
    verify_file_security(&file)?;
    Ok(file)
}

pub(super) fn create_temporary(path: &Path) -> io::Result<File> {
    create_file(path, CREATE_NEW, FILE_SHARE_READ | FILE_SHARE_WRITE)
}

fn create_file(
    path: &Path,
    disposition: FILE_CREATION_DISPOSITION,
    sharing: FILE_SHARE_MODE,
) -> io::Result<File> {
    let descriptor = private_descriptor(false)?;
    let security = attributes(&descriptor);
    let name = wide_path(path)?;
    // SAFETY: path and descriptor stay alive for the native call. A successful
    // CreateFileW returns a new handle which is transferred to File exactly once.
    let file = unsafe {
        CreateFileW(
            PCWSTR(name.as_ptr()),
            GENERIC_READ.0 | GENERIC_WRITE.0,
            sharing,
            Some(&raw const security),
            disposition,
            FILE_FLAG_OPEN_REPARSE_POINT,
            None,
        )
    }
    .map_err(|error| win_error(error.code().0))?;
    // SAFETY: the successful native call transferred ownership of this handle.
    Ok(unsafe { File::from_raw_handle(file.0) })
}

pub(super) fn open_read(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE).0)
        .open(path)
}

fn inspect_path(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .access_mode(READ_CONTROL.0 | FILE_READ_ATTRIBUTES.0)
        .custom_flags((FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT).0)
        .share_mode((FILE_SHARE_READ | FILE_SHARE_WRITE).0)
        .open(path)
}

pub(super) fn validate_file(path: &Path, kind: StorageFileKind) -> Result<(), StorageError> {
    let file = inspect_path(path)
        .map_err(|error| io_error(kind, StorageIoOperation::InspectEntry, error))?;
    inspect(&file, false)?;
    verify_file_security(&file)
}

pub(super) fn validate_opened(
    path: &Path,
    file: &File,
    kind: StorageFileKind,
) -> Result<(), StorageError> {
    let current = inspect_path(path)
        .map_err(|error| io_error(kind, StorageIoOperation::InspectEntry, error))?;
    if inspect(file, false)? != inspect(&current, false)? {
        return Err(document_error(StorageProblem::UnsafeEntryKind));
    }
    Ok(())
}

pub(super) fn verify_file_security(file: &File) -> Result<(), StorageError> {
    let expected = private_descriptor(false)
        .map_err(|_| StorageError::PlatformPermissionEnforcementUnavailable)?;
    verify_security(file, &expected)
}

pub(super) fn confirm_directory(path: &Path) -> io::Result<()> {
    let directory = inspect_path(path)?;
    let expected = private_descriptor(true)?;
    inspect(&directory, true)
        .and_then(|_| verify_security(&directory, &expected))
        .map_err(|_| io::Error::from(io::ErrorKind::PermissionDenied))
}

pub(super) fn move_file(from: &Path, to: &Path, replace: bool) -> io::Result<()> {
    let from = wide_path(from)?;
    let to = wide_path(to)?;
    let mut flags = MOVEFILE_WRITE_THROUGH;
    if replace {
        flags |= MOVEFILE_REPLACE_EXISTING;
    }
    // SAFETY: both paths are live, NUL-terminated names within the protected
    // storage roots verified by the caller. MoveFileExW borrows both buffers.
    unsafe { MoveFileExW(PCWSTR(from.as_ptr()), PCWSTR(to.as_ptr()), flags) }
        .map_err(|error| win_error(error.code().0))
}

fn inspect(file: &File, directory: bool) -> Result<(u32, u32, u32), StorageError> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    // SAFETY: file stays open while Win32 queries its kind and writes into info.
    unsafe {
        if GetFileType(handle(file)) != FILE_TYPE_DISK {
            return Err(document_error(StorageProblem::UnsafeEntryKind));
        }
        GetFileInformationByHandle(handle(file), &raw mut info)
    }
    .map_err(|_| document_error(StorageProblem::UnsafeEntryKind))?;

    let attributes = info.dwFileAttributes;
    if attributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
        || (attributes & FILE_ATTRIBUTE_DIRECTORY.0 != 0) != directory
    {
        return Err(document_error(StorageProblem::UnsafeEntryKind));
    }
    if !directory && info.nNumberOfLinks != 1 {
        return Err(document_error(StorageProblem::HardLinkDetected));
    }
    Ok((
        info.dwVolumeSerialNumber,
        info.nFileIndexHigh,
        info.nFileIndexLow,
    ))
}

fn handle(file: &File) -> HANDLE {
    HANDLE(file.as_raw_handle())
}

fn final_path(file: &File) -> io::Result<PathBuf> {
    let mut path = vec![0_u16; MAX_PATH_UNITS + 1];
    // SAFETY: file is live and Win32 receives a writable buffer with its size.
    let length =
        unsafe { GetFinalPathNameByHandleW(handle(file), &mut path, FILE_NAME_NORMALIZED) };
    let length =
        usize::try_from(length).map_err(|_| io::Error::from(io::ErrorKind::InvalidData))?;
    if length == 0 || length > MAX_PATH_UNITS {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }
    Ok(PathBuf::from(OsString::from_wide(&path[..length])))
}

fn wide_path(path: &Path) -> io::Result<Vec<u16>> {
    let mut units = path
        .as_os_str()
        .encode_wide()
        .take(MAX_PATH_UNITS + 1)
        .collect::<Vec<_>>();
    if units.len() > MAX_PATH_UNITS || units.contains(&0) {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    units.push(0);
    Ok(units)
}

struct LocalAllocation(*mut c_void);

impl Drop for LocalAllocation {
    fn drop(&mut self) {
        // SAFETY: this wrapper takes sole ownership of one LocalAlloc value
        // returned by Win32. A null fallback needs no deallocation.
        if !self.0.is_null() {
            let _unused = unsafe { LocalFree(Some(HLOCAL(self.0))) };
        }
    }
}

fn attributes(descriptor: &LocalAllocation) -> SECURITY_ATTRIBUTES {
    SECURITY_ATTRIBUTES {
        nLength: u32::try_from(size_of::<SECURITY_ATTRIBUTES>())
            .expect("fixed Win32 structure size"),
        lpSecurityDescriptor: descriptor.0,
        bInheritHandle: BOOL(0),
    }
}

fn private_descriptor(directory: bool) -> io::Result<LocalAllocation> {
    let user = current_user_sid()?;
    let inheritance = if directory { "OICI" } else { "" };
    let sddl = format!(
        "O:{user}D:P(A;{inheritance};FA;;;{user})(A;{inheritance};FA;;;SY)(A;{inheritance};FA;;;BA)"
    );
    descriptor_from_sddl(&sddl)
}

fn descriptor_from_sddl(sddl: &str) -> io::Result<LocalAllocation> {
    let text = sddl.encode_utf16().chain([0]).collect::<Vec<_>>();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: text is NUL-terminated and descriptor is a writable out pointer.
    // The returned self-relative descriptor uses LocalFree ownership.
    unsafe {
        ConvertStringSecurityDescriptorToSecurityDescriptorW(
            PCWSTR(text.as_ptr()),
            SDDL_REVISION_1,
            &raw mut descriptor,
            None,
        )
    }
    .map_err(|error| win_error(error.code().0))?;
    Ok(LocalAllocation(descriptor.0))
}

fn current_user_sid() -> io::Result<String> {
    let mut token = HANDLE::default();
    // SAFETY: the pseudo process handle is valid for this process. On success
    // the output is a new token handle with only query access.
    unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &raw mut token) }
        .map_err(|error| win_error(error.code().0))?;
    // SAFETY: token was returned owned by OpenProcessToken and is transferred once.
    let token = unsafe { OwnedHandle::from_raw_handle(token.0) };

    // Pointer-aligned storage for TOKEN_USER and its bounded SID data.
    let mut buffer = [0_usize; 32];
    let capacity = u32::try_from(size_of_val(&buffer)).expect("fixed token buffer size");
    let mut written = 0;
    // SAFETY: token stays open and buffer is suitably aligned and writable for
    // the passed capacity. On success TokenUser returns a complete TOKEN_USER.
    unsafe {
        GetTokenInformation(
            HANDLE(token.as_raw_handle()),
            TokenUser,
            Some(buffer.as_mut_ptr().cast()),
            capacity,
            &raw mut written,
        )
    }
    .map_err(|error| win_error(error.code().0))?;
    let minimum = u32::try_from(size_of::<TOKEN_USER>()).expect("fixed Win32 structure size");
    if !(minimum..=capacity).contains(&written) {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }
    // SAFETY: GetTokenInformation succeeded for TOKEN_USER. Its Sid points to
    // valid SID bytes in buffer, which is kept live across the conversion.
    let user = unsafe { (*buffer.as_ptr().cast::<TOKEN_USER>()).User.Sid };
    let mut encoded = PWSTR::null();
    // SAFETY: user refers to the TokenUser SID and encoded is a valid out pointer.
    unsafe { ConvertSidToStringSidW(user, &raw mut encoded) }
        .map_err(|error| win_error(error.code().0))?;
    let allocation = LocalAllocation(encoded.0.cast());
    let mut value = Vec::new();
    for index in 0..MAX_SID_STRING_UNITS {
        // SAFETY: a successful conversion returns an allocated NUL-terminated
        // SID. The loop stops at NUL, never reading past that terminator.
        let unit = unsafe { allocation.0.cast::<u16>().add(index).read() };
        if unit == 0 {
            return String::from_utf16(&value)
                .map_err(|_| io::Error::from(io::ErrorKind::InvalidData));
        }
        value.push(unit);
    }
    Err(io::Error::from(io::ErrorKind::InvalidData))
}

fn verify_security(file: &File, expected: &LocalAllocation) -> Result<(), StorageError> {
    let error = StorageError::PlatformPermissionEnforcementUnavailable;
    let mut actual = PSECURITY_DESCRIPTOR::default();
    // SAFETY: file is open with READ_CONTROL and the out pointer is writable.
    // On success actual owns a complete self-relative descriptor.
    if unsafe {
        GetSecurityInfo(
            handle(file),
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            None,
            None,
            None,
            None,
            Some(&raw mut actual),
        )
    } != ERROR_SUCCESS
    {
        return Err(error);
    }
    let actual = LocalAllocation(actual.0);
    let (owner, acl) = owned_acl(&actual).ok_or(error)?;
    let (expected_owner, expected_acl) = owned_acl(expected).ok_or(error)?;
    // SAFETY: both SIDs were returned from live, validated Windows descriptors.
    if unsafe { EqualSid(owner, expected_owner) }.is_err() || acl != expected_acl {
        return Err(error);
    }
    Ok(())
}

fn owned_acl(descriptor: &LocalAllocation) -> Option<(PSID, &[u8])> {
    let descriptor = PSECURITY_DESCRIPTOR(descriptor.0);
    let mut owner = PSID::default();
    let mut defaulted = BOOL(0);
    let mut control = 0;
    let mut revision = 0;
    let mut present = BOOL(0);
    let mut acl: *mut ACL = ptr::null_mut();
    // SAFETY: callers pass a live self-relative descriptor produced by Windows.
    // The out pointers are writable and the returned owner and ACL are borrowed
    // from that descriptor's allocation.
    unsafe {
        GetSecurityDescriptorOwner(descriptor, &raw mut owner, &raw mut defaulted).ok()?;
        GetSecurityDescriptorControl(descriptor, &raw mut control, &raw mut revision).ok()?;
        GetSecurityDescriptorDacl(
            descriptor,
            &raw mut present,
            &raw mut acl,
            &raw mut defaulted,
        )
        .ok()?;
    }
    if !present.as_bool()
        || owner.0.is_null()
        || acl.is_null()
        || control & SE_DACL_PROTECTED.0 == 0
    {
        return None;
    }
    let mut size = ACL_SIZE_INFORMATION::default();
    // SAFETY: acl is a present non-null borrowed ACL from the live descriptor.
    // Validate it before asking Windows for its complete initialized ACE length.
    unsafe {
        if !IsValidAcl(acl).as_bool() {
            return None;
        }
        GetAclInformation(
            acl,
            (&raw mut size).cast(),
            u32::try_from(size_of::<ACL_SIZE_INFORMATION>()).ok()?,
            AclSizeInformation,
        )
        .ok()?;
    }
    if size.AceCount != 3 || size.AclBytesInUse > MAX_ACL_BYTES {
        return None;
    }
    let length = usize::try_from(size.AclBytesInUse).ok()?;
    let ace_bytes = length.checked_sub(size_of::<ACL>())?;
    // SAFETY: IsValidAcl and GetAclInformation verified the number of bytes in
    // use. The returned slice covers its ACE records and borrows descriptor.
    let entries =
        unsafe { std::slice::from_raw_parts(acl.cast::<u8>().add(size_of::<ACL>()), ace_bytes) };
    Some((owner, entries))
}

fn win_error(code: i32) -> io::Error {
    if (code >> 16) & 0x1fff == 7 {
        io::Error::from_raw_os_error(code & 0xffff)
    } else {
        io::Error::from(io::ErrorKind::Other)
    }
}

#[cfg(test)]
mod tests;
