use qrcode::{Color, EcLevel, QrCode};
use serde::Serialize;
use tauri::ipc::{CommandArg, CommandItem, InvokeError};
use tauri::{Manager, RunEvent, Runtime, WindowEvent};
use tether_agent_core::windows_test::{Command, PORT_OPTIONS, Phase, Pilot, Snapshot};
use tether_network::assets::EmbeddedAssets;

mod notifications;
mod startup;
mod tray;

pub(super) use notifications::show_startup_error;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct View {
    phase: &'static str,
    revision: String,
    networks: Vec<Network>,
    port: u16,
    ports: [u16; PORT_OPTIONS.len()],
    generation: String,
    pending: Vec<Phone>,
    qr: Option<Code>,
    listening_address: Option<String>,
    paired: usize,
    connected: usize,
    controlling: bool,
    notice: Option<&'static str>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Network {
    index: usize,
    label: String,
    address: String,
    auto_listen_available: bool,
    auto_listen_saved: bool,
}

#[derive(Serialize)]
struct Phone {
    id: String,
    label: String,
}

#[derive(Serialize)]
struct Code {
    width: usize,
    bits: String,
}

fn code(url: &str) -> Option<Code> {
    let code = QrCode::with_error_correction_level(url.as_bytes(), EcLevel::M).ok()?;
    let bits = code
        .to_colors()
        .into_iter()
        .map(|pixel| if pixel == Color::Dark { '1' } else { '0' })
        .collect();
    Some(Code {
        width: code.width(),
        bits,
    })
}

#[derive(Clone, Copy)]
struct Local<'a>(&'a Pilot);

impl<'a, R: Runtime> CommandArg<'a, R> for Local<'a> {
    fn from_command(command: CommandItem<'a, R>) -> Result<Self, InvokeError> {
        let webview = command.message.webview_ref();
        if webview.label() != "main" {
            return Err("Unavailable from this window.".into());
        }
        webview
            .try_state::<Pilot>()
            .map(|state| Self(state.inner()))
            .ok_or_else(|| "The local control service is unavailable.".into())
    }
}

fn parse_id(value: &str) -> Result<u64, &'static str> {
    if value.len() > 20 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("That selection is no longer current.");
    }
    value
        .parse()
        .map_err(|_| "That selection is no longer current.")
}

impl From<Snapshot> for View {
    fn from(state: Snapshot) -> Self {
        Self {
            phase: match state.phase {
                Phase::Startup => "startup",
                Phase::ChooseNetwork => "choose",
                Phase::Starting => "starting",
                Phase::Listening => "listening",
                Phase::Stopping => "stopping",
                Phase::Unavailable => "unavailable",
            },
            revision: state.inventory_revision.to_string(),
            networks: state
                .networks
                .into_iter()
                .map(|choice| Network {
                    index: choice.index,
                    label: choice.label,
                    address: choice.address.to_string(),
                    auto_listen_available: choice.auto_listen_available,
                    auto_listen_saved: choice.auto_listen_saved,
                })
                .collect(),
            port: state.port,
            ports: PORT_OPTIONS,
            generation: state.generation.to_string(),
            pending: state
                .phones
                .into_iter()
                .map(|phone| Phone {
                    id: phone.id.to_string(),
                    label: phone.label,
                })
                .collect(),
            qr: state.offer.as_deref().and_then(code),
            listening_address: state
                .listening_address
                .map(|address| format!("http://{address}/")),
            paired: state.paired,
            connected: state.connected,
            controlling: state.controlling,
            notice: state.notice,
        }
    }
}

#[tauri::command]
fn snapshot(pilot: Local<'_>) -> View {
    pilot.0.snapshot().into()
}

#[tauri::command]
fn start_network(
    pilot: Local<'_>,
    revision: &str,
    index: usize,
    port: u16,
    auto_listen: bool,
) -> Result<(), &'static str> {
    pilot.0.send(Command::Start {
        revision: parse_id(revision)?,
        index,
        port,
        auto_listen,
    })
}

#[tauri::command]
fn refresh_networks(pilot: Local<'_>) -> Result<(), &'static str> {
    pilot.0.send(Command::Refresh)
}

#[tauri::command]
fn stop_control(pilot: Local<'_>) {
    pilot.0.stop();
}

#[tauri::command]
fn new_code(pilot: Local<'_>) -> Result<(), &'static str> {
    pilot.0.send(Command::NewCode)
}

#[tauri::command]
fn decide_phone(
    pilot: Local<'_>,
    generation: &str,
    request: &str,
    approve: bool,
) -> Result<(), &'static str> {
    pilot.0.send(Command::Decide {
        generation: parse_id(generation)?,
        request: parse_id(request)?,
        approve,
    })
}

#[tauri::command]
fn revoke_phones(pilot: Local<'_>) -> Result<(), &'static str> {
    pilot.0.send(Command::RevokePhones)
}

#[tauri::command]
fn get_autostart_state(_pilot: Local<'_>) -> Result<startup::StartupState, &'static str> {
    startup::state()
}

#[tauri::command]
fn set_autostart(_pilot: Local<'_>, enabled: bool) -> Result<startup::StartupState, &'static str> {
    startup::set(enabled)
}

pub(super) fn run(assets: EmbeddedAssets) -> tauri::Result<()> {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, args, _| {
            if !args.iter().skip(1).any(|arg| arg == startup::ARG) {
                tray::show(app);
            }
        }))
        .setup(move |app| {
            let window = app
                .get_webview_window("main")
                .ok_or_else(|| std::io::Error::other("Tether window is unavailable"))?;
            // Use the Windows settings root. Tauri manages its browser data
            // in the separate identifier directory with Windows default ACLs.
            let path = app.path().local_data_dir()?.join("Tether");
            let pilot = Pilot::spawn(path, assets).map_err(std::io::Error::other)?;
            notifications::attach(window.hwnd()?, pilot.stop_handle())?;
            app.manage(pilot);
            app.manage(tray::Tray::install(app.handle())?);
            if !std::env::args_os().skip(1).any(|arg| arg == startup::ARG) {
                window.show()?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() == "main"
                && let WindowEvent::CloseRequested { api, .. } = event
            {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .invoke_handler(tauri::generate_handler![
            snapshot,
            start_network,
            refresh_networks,
            stop_control,
            new_code,
            decide_phone,
            revoke_phones,
            get_autostart_state,
            set_autostart,
        ])
        .build(tauri::generate_context!())?;
    let mut last_update = None;
    let mut previous = "";
    app.run(move |app, event| {
        if matches!(event, RunEvent::MainEventsCleared)
            && last_update.is_none_or(|at: std::time::Instant| {
                at.elapsed() >= std::time::Duration::from_millis(500)
            })
            && let Some(tray) = app.try_state::<tray::Tray>()
        {
            last_update = Some(std::time::Instant::now());
            let label = tray::label(&app.state::<Pilot>().snapshot());
            if label != previous && tray.update(label).is_ok() {
                previous = label;
            }
        }
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit)
            && let Some(pilot) = app.try_state::<Pilot>()
        {
            pilot.shutdown();
        }
    });
    Ok(())
}
