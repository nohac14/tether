# Provisioning Tether validation environments

**Status:** Setup guide; no environment provisioned or native test passed by this document
**Last checked:** 2026-11-19 for Windows; 2026-10-16 otherwise
**Authority:** `SPEC.md`, D-02/D-05, `environment-inventory.md`, and `../implementation-plan.md`

## Choose the machines before downloading images

The initial release targets Windows 11 x86-64 and named Linux x86-64 systems.
An Apple Silicon Mac can efficiently virtualize ARM guests. That is useful
additional engineering evidence, but an ARM guest does not validate the
approved x86-64 target. A VM also cannot establish physical mixed-DPI monitor,
pointer acceleration, or hardware support claims by itself.

Use this order to minimize purchases and duplicate setup:

1. Inventory an existing Intel/AMD Windows 11 computer, any other Intel/AMD
   computer capable of hosting desktop VMs, an Android phone, and the Mac's RAM
   and free disk space. The owner now reports a Windows 11 AMD PC in addition
   to the Mac (16 GiB RAM, about 60 GiB free). Confirm the PC's architecture,
   resources, and developer tools before preparing it. Do not repartition a
   machine to start testing.
2. If an Intel/AMD Windows 11 computer is available, use its normal interactive
   desktop for Windows and run separate Linux desktop VMs there. This directly
   exercises the intended CPU architecture.
3. If only the Mac is available, start with one ARM Linux VM to establish portal
   feasibility. Obtain an x86-64 target later for the named support gate.
   UTM can emulate x86-64 as a different, slower option; record that emulation
   explicitly and do not equate it with native hardware validation.
4. Use a physical Android phone on the same private Wi-Fi as the Mac for BV-001.
   An Android emulator cannot satisfy that gate.

VirtualBox 7.1 on Apple Silicon supports ARM Linux guests and does not run x86
guests. Its ARM graphics and guest integration also have limitations. Verify
the chosen hypervisor/version before creating a compositor test environment:
[Oracle host/guest combinations](https://docs.oracle.com/en/virtualization/virtualbox/7.1/user/Introduction.html),
[Oracle known limitations](https://docs.oracle.com/en/virtualization/virtualbox/7.1/user/KnownIssues.html).

Windows ARM on a Mac is optional additional evidence. It does not replace the
Windows 11 x86-64 gate. Microsoft documents
[Windows ARM on Apple Silicon and licensing](https://support.microsoft.com/en-us/windows/experience/platform-variants/options-for-using-windows-11-with-mac-computers-with-apple-m1-m2-and-m3-chips).
Do not buy a Windows license or a paid hypervisor until the chosen route and
existing hardware have been checked.

## Linux desktop VM setup

For the currently available 16 GiB Apple Silicon Mac with roughly 60 GiB free,
start with a single ARM64 GNOME VM: 4 GiB RAM, 2 vCPUs, and a dynamically
allocated 32 GiB disk. Keep approximately 20 GiB of host space free for swap,
builds, and updates; monitor actual guest/host usage before each build. These
are a limited spike configuration, not full-release build minimums. If the guest
outgrows its disk, add storage before proceeding. Run no second VM concurrently.
Use external storage or another host before retaining additional full desktop
images and snapshots.

The Windows 11 x64 candidate is available for native Windows preparation. Assess
its RAM and free disk space before deciding whether it can also host separate
Linux x86-64 desktop VMs. The Mac ARM64 VM route below is optional. Linux guest
provisioning and physical Android validation remain pending.

On a larger host, resource planning may start at 4–8 GiB RAM, 2–4 vCPUs, and a
dynamically allocated 40–60 GiB disk. Those are estimates, not platform minimums,
and do not supersede the smaller single-VM allocation above for this Mac.
A sparse disk can eventually consume its full capacity. Do not allocate more
cores/RAM than the host can spare; avoid retaining multiple images/snapshots
without additional storage.

The candidate configurations are:

| Purpose | Candidate image | Required observed session |
| --- | --- | --- |
| GNOME portal/EI | Maintained Fedora Workstation, currently Fedora 44 | GNOME Wayland |
| KDE portal/EI | Maintained Fedora KDE Plasma Desktop, currently Fedora 44 | Plasma Wayland |
| X11 XTEST/XKB | A maintained distribution with an actual Xorg desktop, such as Ubuntu 24.04 LTS with its Xorg login option or an Xfce/Xorg installation | `XDG_SESSION_TYPE=x11`, not an XWayland app inside Wayland |

Download from the official
[Fedora Workstation](https://fedoraproject.org/workstation/download/),
[Fedora KDE](https://fedoraproject.org/kde/download/), or
[Ubuntu 24.04](https://releases.ubuntu.com/24.04/) page. Choose `aarch64`/ARM64
for virtualization on Apple Silicon, or `x86_64`/AMD64 on Intel/AMD. Verify the
vendor's signed checksum and image hash. Record the installed release and
updates; a download recommendation is not a support declaration.

For UTM on the Mac:

1. Obtain UTM from its [official site](https://mac.getutm.app/). Installation and
   VM storage locations need owner approval before the agent writes outside
   its workspace. macOS may require the owner to approve opening the app.
2. Create a new VM, choose **Virtualize → Linux**, select the matching ARM64 ISO,
   and set the resources above. UTM's [Fedora guide](https://docs.getutm.app/guides/fedora/)
   shows the wizard; its older example release is not the release to download.
3. Use a new disposable VM disk. Install the desktop into that disk, create a
   dedicated test user, update from official distribution repositories, reboot,
   and log into the graphical desktop.
4. Keep guest clipboard sharing, drag-and-drop, and broad home-directory shares
   off. Use a separate guest checkout and transfer source through SSH. Do not
   mount the host's credentials or writable working repository into the guest.
5. Keep GNOME and KDE as separate installations to avoid ambiguous portal
   backend selection. Shut one down before running the other if RAM is limited.
6. Take a powered-off baseline snapshot or backup if the hypervisor supports it.
   Record the hypervisor version; saved states can have upgrade restrictions.

Confirm the session from a terminal opened inside its graphical desktop:

```sh
uname -m
cat /etc/os-release
printf '%s\n' "$XDG_SESSION_TYPE" "$XDG_CURRENT_DESKTOP"
```

Record installed compositor and portal package versions. GNOME/KDE names alone
do not prove that RemoteDesktop portal v2, ConnectToEIS, or EI text works. The
actual spike must still test grant/deny, restore, capabilities, device lifecycle,
and cleanup. For X11, inspect XTEST/XKB extensions only after confirming a real
Xorg session. Do not use `xhost +`, global keymap changes, `/dev/uinput`, or root
input injection to make a test work.

## Give the agent access without sharing passwords

SSH is sufficient to transfer a checkout, install approved build dependencies,
compile, run inert tests, and collect sanitized version/capability evidence.
It does not by itself supply a logged-in GUI session or permission to inject.

1. Enable the guest's OpenSSH server using its official package manager and a
   dedicated test account. Keep administrative package installation separate
   from the unelevated application tests; do not grant passwordless root access.
2. Configure guest access using host-only networking or NAT with an SSH forward
   bound to the Mac's `127.0.0.1` (for example host port 2222 to guest port 22).
   Do not forward SSH from the public internet. An actual LAN test later needs
   its own approved interface/topology; the SSH route is administration only.
3. Create a dedicated SSH key outside the repository, authorize only its public
   key for the test account, and verify the guest host-key fingerprint from the
   VM console before the first connection. Keep strict host-key verification.
   Do not send private keys, passwords, or signing identities in chat.
4. Supply only the connection target, test username, verified host fingerprint,
   local key path or SSH alias, guest checkout path, and the scope of permission
   to prepare/test that guest. Machine-specific endpoints and keys remain local
   and uncommitted.
5. Keep the guest graphical desktop logged in. Portal approval must occur in
   that user's real session. The agent must confirm the correct session bus and
   graphical context; it must not bypass portal policy from an SSH shell.
6. Before any live injection, review the exact opt-in command, limited test
   sequence, timeout, cleanup/abort controls, and selected disposable window.
   Default tests remain inert. The current repository command is
   `cargo xtask manual-test-guard <scope>`; that command only checks consent and
   target variables and does not run an injector.

If the current agent sandbox cannot reach the guest or its local SSH key,
approve the specific connection operation or run the supplied commands yourself
and return the sanitized output. There is no need to grant the agent general
administrator or unrestricted account access.

## Windows 11 x86-64 setup

Use an existing Intel/AMD Windows 11 machine or a Windows 11 x86-64 VM on an
Intel/AMD host. Keep the desktop interactive and run the app unelevated.
The [Windows checkpoint](2026-11-19-windows-foundation.md) records the current
PC and completed checks. Windows persistence is gated by R-018's DACL and
reparse-point implementation, review, and tests.

For the repository's MSVC target, use the pinned Rust toolchain and Microsoft's
standalone Visual Studio C++ Build Tools. The Build Tools provide the x64
linker and libraries. Visual Studio Code is optional and can be replaced by any
editor. Select **Desktop development with C++**, the x64/x86 MSVC component, and
the Windows 11 SDK in the Microsoft installer. Use the x64 developer shell and
record the installed linker and SDK versions in the test evidence.
[Microsoft command-line setup](https://learn.microsoft.com/en-us/cpp/build/building-on-the-command-line)

Run the locked checks from the repository after provisioning. These tests use
an inert sink and can run without an interactive input target.

```powershell
rustc -vV
cargo check -p tether-input-windows --all-targets --all-features --locked
cargo clippy -p tether-input-windows --all-targets --all-features --locked -- -D warnings
cargo test -p tether-input-windows --locked
```

Desktop executable builds also use the workspace's pinned Node.js 24.19.0,
pnpm 11.25.0 and locked Vite installation. From the x64 developer shell, put
the pinned Node on PATH or set `TETHER_BUILD_NODE` to its absolute executable
path. The Cargo build script generates and verifies the embedded bundle
directly from source.

```powershell
pnpm install --frozen-lockfile --ignore-scripts
cargo --config 'target.x86_64-pc-windows-msvc.rustflags=["-C","target-feature=+crt-static"]' build -p tether-desktop --release --target x86_64-pc-windows-msvc --locked
```

For the current build checkpoint, run the resulting executable with
`--verify-mobile-bundle` to validate the embedded data without starting a
listener. Node and the frontend workspace are build-time inputs.
[Windows mobile bundle evidence](2026-11-20-mobile-bundle.md)

Optional SSH access uses Microsoft's
[in-box OpenSSH Server](https://learn.microsoft.com/en-us/windows-server/administration/openssh/openssh_install_firstuse)
and [key authentication](https://learn.microsoft.com/en-us/windows-server/administration/openssh/openssh_keymanagement).
The owner handles feature installation and narrows firewall access to the test
host/private administration path. An SSH service session is not evidence that
input reached the user's interactive desktop. The guarded input test must be
launched/observed in the intended logged-in session.

## Android Chrome setup

1. Use a physical Android phone with current stable Chrome. Record model,
   Android version, and Chrome version without device serials or account data.
2. Connect the phone and Mac to the same trusted private Wi-Fi. Guest/client
   isolation, VPNs, and multicast filtering are separate negative cases later.
3. The agent prepares a test-only BV-001 HTTP/WebSocket page before asking for
   browser actions. It contains no pairing or input injection path.
4. The owner explicitly approves the selected LAN listener and opens the test
   URL in Chrome. Record navigation, any Local Network Access prompts,
   allow/deny, same-origin WebSocket with the active port, and reconnect results.
5. USB debugging is optional. If needed for console evidence, enable it only
   for the test and approve the specific Mac. It is not required to operate the
   eventual product and does not replace testing over the actual LAN.

## What the owner must do versus what the agent can automate

The owner chooses the hardware/storage and any paid software, approves OS
installation/licensing/security dialogs, creates or authorizes the test user,
and grants portal/native-injection consent at the documented test boundary.
After the connection is established and preparation is authorized, the agent
can configure the checkout, install approved build tools, run automated tests,
and collect the evidence. Record each host/VM/architecture independently and
retain all unperformed acceptance criteria as open.
