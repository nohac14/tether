# Tether

A LAN-only remote mouse and keyboard application.

The desktop application hosts a local mobile web interface. A phone or
tablet on the same LAN connects through its browser and controls the
computer's mouse and keyboard.

Target desktop platforms:

- Windows
- macOS
- Linux

Windows 11 x64 is the first delivery target. See the
[validation matrix](docs/operations/support-matrix.md) for current evidence and
the [Windows setup guide](docs/manual-tests/test-environment-setup.md#windows-11-x86-64-setup)
for build prerequisites.

## Personal Windows test

Run `artifacts/Tether-Windows-Test/Tether.exe` on the Windows 11 x64 PC you
want to control. This unsigned build uses the PC's installed Edge WebView2
Runtime. It opens a local window and waits for you to select a network.

1. Connect the PC and phone to the same trusted private LAN. Ethernet on the
   PC and Wi-Fi on the phone work if both connect to the same LAN.
2. In Tether, select the network and click **Start on this network**. If Windows
   blocks the connection port, pick another one from **Connection port** and
   start again. If Windows Firewall asks, allow Tether on private networks only.
3. Scan the QR code with your phone and approve the phone in the Tether
   window. Try the touchpad and keyboard in a normal Windows app.
4. Click **Stop control** to disconnect. Closing the window keeps Tether running.
   To exit, right-click the Tether icon in the Windows system tray and choose
   **Quit Tether**.

The personal Windows build starts listening only after **Start on this
network**. To launch the app in the tray when you sign in to Windows, check
**Start Tether in the tray when I sign in**. You still select **Start on this
network** after sign-in. You can keep a paired phone's page on the iPhone Home Screen.
Reopen that page after starting Tether on the same address and port. The PC
window shows the reconnect URL while listening. Tap **Retry** on the phone
if the PC took longer to start. Scanning a pairing QR in a browser that still
has its valid credential also reconnects the existing phone. Pairing
expires after 30 days without an authenticated connection. A changed PC IP or
cleared browser data can require pairing again.

To rebuild it with the pinned Windows toolchain from Visual Studio Developer
PowerShell, run the following command. The executable is under Cargo's release
output for the selected target.

```powershell
$env:RUSTFLAGS = '-C target-feature=+crt-static'
cargo build --locked --target x86_64-pc-windows-msvc --release -p tether-desktop --features windows-test
```

## Project status

The architecture review is accepted and reconciled into `SPEC.md`. The manual
environment inventory and verified repository/toolchain skeleton are complete.
The platform-neutral input domain, normalization, mock backend, bounded
fail-safe dispatcher, settings/installation-identity storage, canonical key
mapping, and inert backend conformance suite are implemented. Native backends,
sessions, networking, and user interfaces proceed through the
dependency-ordered plan.

- [Architecture and feasibility review](docs/architecture-review.md)
- [Accepted architecture decisions](docs/architecture-decisions.md)
- [Open risks and validation gates](docs/open-risks.md)
- [Repository structure](docs/repository-structure.md)
- [Component documentation plan](docs/documentation-plan.md)
- [Dependency-ordered implementation plan](docs/implementation-plan.md)
- [Dependency and supply-chain policy](docs/dependency-policy.md)
- [Initial dependency and license inventory](docs/dependency-inventory.md)
- [Agent development instructions](AGENTS.md)
