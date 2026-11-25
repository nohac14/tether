//! Thin desktop composition root.
#![cfg_attr(
    all(windows, feature = "windows-test", not(debug_assertions)),
    windows_subsystem = "windows"
)]

mod mobile_bundle;
#[cfg(all(windows, feature = "windows-test"))]
mod windows_test;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let assets = mobile_bundle::load()?;
    let verify = std::env::args_os()
        .nth(1)
        .is_some_and(|argument| argument == "--verify-mobile-bundle");
    if verify {
        println!("Embedded mobile bundle verified.");
        return Ok(());
    }
    #[cfg(all(windows, feature = "windows-test"))]
    if windows_test::run(assets).is_err() {
        windows_test::show_startup_error();
        return Err(std::io::Error::other("Tether could not open its desktop window").into());
    }
    #[cfg(not(all(windows, feature = "windows-test")))]
    drop(assets);
    Ok(())
}
