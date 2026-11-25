use tauri::menu::{MenuBuilder, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};
use tether_agent_core::windows_test::{Phase, Pilot, Snapshot};

pub(super) fn show(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub(super) struct Tray {
    icon: TrayIcon<Wry>,
    status: MenuItem<Wry>,
}

impl Tray {
    pub(super) fn install(app: &AppHandle) -> tauri::Result<Self> {
        let status = MenuItem::new(app, "Starting", false, None::<&str>)?;
        let menu = MenuBuilder::new(app)
            .item(&status)
            .text("show", "Show Tether")
            .text("stop", "Stop control and listening")
            .separator()
            .text("quit", "Quit Tether")
            .build()?;
        let image = app
            .default_window_icon()
            .ok_or(tauri::Error::AssetNotFound("Tether icon".into()))?;
        let icon = TrayIconBuilder::with_id("tether")
            .icon(image.clone())
            .menu(&menu)
            .tooltip("Tether")
            .show_menu_on_left_click(false)
            .on_tray_icon_event(|icon, event| {
                if matches!(
                    event,
                    TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    }
                ) {
                    show(icon.app_handle());
                }
            })
            .on_menu_event(|app, event| match event.id().as_ref() {
                "show" => show(app),
                "stop" => app.state::<Pilot>().stop(),
                "quit" => app.exit(0),
                _ => {}
            })
            .build(app)?;
        Ok(Self { icon, status })
    }

    pub(super) fn update(&self, label: &str) -> tauri::Result<()> {
        self.status.set_text(label)?;
        self.icon.set_tooltip(Some(format!("Tether · {label}")))
    }
}

pub(super) fn label(view: &Snapshot) -> &'static str {
    match view.phase {
        Phase::Startup => "Starting",
        Phase::ChooseNetwork => "Ready to connect",
        Phase::Starting => "Starting control",
        Phase::Listening if view.controlling => "Phone in control",
        Phase::Listening if !view.phones.is_empty() => "Pairing approval requested",
        Phase::Listening if view.connected > 0 => "Phone connected",
        Phase::Listening => "Listening",
        Phase::Stopping => "Stopping control",
        Phase::Unavailable => "Control unavailable",
    }
}
