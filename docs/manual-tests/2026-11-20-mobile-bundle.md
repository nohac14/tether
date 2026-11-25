# Windows embedded mobile bundle checkpoint

The pinned Vite build transforms the mobile scaffold to one HTML entry and
hashed JavaScript and CSS. It validates the manifest and emitted asset set
before writing outputs. Public directory copying, source maps, asset inlining
and project environment-file loading are disabled. The desktop build invokes
the same builder from source and generates an exact `include_bytes!` table in
Cargo's output directory. Cargo watches the mobile source, build scripts and
lockfile so changed inputs trigger a new bundle. The executable's responder
reads the compiled bytes directly.
[Vite build options](https://vite.dev/config/build-options)
[Cargo build-script inputs](https://doc.rust-lang.org/cargo/reference/build-scripts.html#inputs-to-the-build-script)

## Executable check

Built the release executable on Windows 11 x64 using Rust 1.99.0, MSVC,
Node.js 24.19.0 and Vite 8.1.6. The initial build imported
`VCRUNTIME140.dll`. The explicit `x86_64-pc-windows-msvc` build with
`target-feature=+crt-static` linked successfully and imported only Windows
system libraries. `dumpbin /dependents` showed no dynamic C-runtime imports
on that build.
[Rust CRT linkage](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes)

Copied only `tether-desktop.exe` into a fresh scratch directory, set PATH
to System32 for the check, and ran `--verify-mobile-bundle`. Verification
succeeded with no Node executable, frontend folder, or bundle alongside it.
The host already has Windows system libraries. This was a bundle
verification run. A clean Windows installation and an installer remain
unqualified.

## Automated checks

- All 228 Rust workspace tests passed on Windows. The new desktop test
  exercised the actual embedded HTML, JavaScript and CSS through
  `EmbeddedAssets`, checked MIME and cache headers, and confirmed source
  paths, the build manifest and source maps cannot be served.
- All ten mobile bundle tests passed, including two independent Vite runs
  that produced byte-identical embedded outputs. Tests also rejected path
  traversal, missing or extra asset entries, mismatched HTML and oversize
  output.
- Formatting, strict Rust Clippy, workspace and release builds, locked
  metadata, frontend build, TypeScript checks, ESLint and pinned protocol
  generation passed. Windows cargo-deny passed. The all-platform advisory
  check continues to report the three existing Wayland findings under R-019.
- Vitest passed 115 of 116 tests. Windows returned `EPERM` while creating
  the pre-existing dangling file symlink fixture. All four Node secret-scan
  tests passed. The fixture remains active.

HTTP integration through the listener, browser and phone validation, the
production mobile controls, and the packaged application remain pending.
