# ADR 0005 — Windows private storage

**Status:** Accepted by the owner on 2026-11-19; native verification required

**Scope:** R-018, `tether-config`, Windows 11 x64

## Decision

Use the existing Microsoft `windows` crate for a private Windows-only storage
module. Rust's stable filesystem APIs supply file locking and replacement, but
they do not expose protected Windows DACL creation and full handle-based link
inspection. The module will translate only the Win32 calls needed to establish
and verify the existing storage contract. The settings and identity APIs, data
formats, process-lifetime lock and atomic commit flow remain shared.

## Boundaries

- Resolve the user application-data root through the desktop's trusted OS
  integration. Check canonical containment and local volume ACL support before
  accessing application files. Use bounded path and security descriptor buffers.
- Read the current user's SID from the process token. Create application
  directories and new files with a protected DACL for that user, SYSTEM and
  local Administrators. Apply it at creation using `SECURITY_ATTRIBUTES`, before
  writing bytes. For existing entries, inspect ownership and ACLs via the open
  handle and reject insecure descriptors. Handle grants made before an ACL
  change can survive it, so tightening an existing broad ACL alone is insufficient.
- Open entries with reparse-point inspection and check their type, handle-based
  volume/file identity and regular-file link count before use. Hold the required
  handles across validation and I/O. Validate source and destination constraints
  before replacement. Fail closed when the filesystem cannot report these facts.
- Isolate handle and allocated-descriptor lifetime management inside the module.
  Each native call with an unsafe precondition documents its ownership, pointer,
  size and lifetime invariant. Rust callers use `File` and closed storage errors.
  The running agent uses the ordinary unelevated desktop user.

## Evidence required before enabling Windows persistence

Run tests on Windows 11 using temporary directories and synthetic records.
Verify exact DACL principals and inheritance protection, rejection of broad or
foreign ACLs and links, exclusive lifetime locking, versioned settings and
identity replacement, interrupted writes, corruption recovery, reset and
upgrade. Verify Windows replacement/durability behavior explicitly and record
what was measured. Review the native boundary and require every previously
failing Windows storage test to pass before closing R-018.

Microsoft documents handle identity and DACL operations in
[GetFileInformationByHandle](https://learn.microsoft.com/windows/win32/api/fileapi/nf-fileapi-getfileinformationbyhandle),
[GetSecurityInfo](https://learn.microsoft.com/windows/win32/api/aclapi/nf-aclapi-getsecurityinfo)
and [CreateDirectoryW](https://learn.microsoft.com/windows/win32/api/fileapi/nf-fileapi-createdirectoryw).
