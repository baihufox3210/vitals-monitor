use crate::{config::Config, overlay::Overlay};
use gtk::{
    Application, CssProvider, StyleContext, gdk, gio, glib::translate::ToGlibPtr, prelude::*,
};
use std::{cell::RefCell, ffi::CString, process::Command, rc::Rc};

type Hud = Rc<RefCell<Overlay>>;

struct Session {
    hud: Hud,
    applied: Config,
    style: Option<CssProvider>,
    screen: gdk::Screen,
}

impl Session {
    fn new(app: &Application) -> Result<Self, String> {
        let screen = gdk::Screen::default().ok_or("無法連線到 Wayland 桌面")?;
        let base = provider(include_str!("theme.css"))?;
        StyleContext::add_provider_for_screen(&screen, &base, 600);
        let applied = Config::load().map_err(|e| e.to_string())?;
        let hud = Overlay::new(app, applied.clone()).map_err(str::to_string)?;
        let mut session = Self {
            hud,
            applied,
            style: None,
            screen,
        };
        if let Ok(css) = provider(&session.applied.css) {
            session.set_style(css);
        }
        Overlay::configure(&session.hud, session.applied.clone());
        Ok(session)
    }
    fn set_style(&mut self, provider: CssProvider) {
        if let Some(old) = self.style.replace(provider.clone()) {
            StyleContext::remove_provider_for_screen(&self.screen, &old);
        }
        StyleContext::add_provider_for_screen(&self.screen, &provider, 800);
    }
    fn update(&mut self, operation: &str, json: Option<&str>) -> Result<(), String> {
        match operation {
            "apply" | "preview" => {
                let config = Config::parse(json.ok_or("缺少設定資料")?.as_bytes())
                    .map_err(|e| e.to_string())?;
                if config.target.trim().is_empty() {
                    return Err("請輸入遊戲執行檔名稱".into());
                }
                let css = provider(&config.css)?;
                if operation == "apply" {
                    config.save().map_err(|e| format!("無法儲存設定：{e}"))?;
                    self.applied = config.clone();
                }
                self.set_style(css);
                Overlay::set_preview(&self.hud, operation == "preview", config);
            }
            "stop-preview" => {
                let css = provider(&self.applied.css).or_else(|_| provider(""))?;
                self.set_style(css);
                Overlay::set_preview(&self.hud, false, self.applied.clone());
            }
            _ => return Err("未知的 Vitals Monitor 操作".into()),
        }
        Ok(())
    }
}

fn provider(text: &str) -> Result<CssProvider, String> {
    let css = CssProvider::new();
    css.load_from_data(text.as_bytes())
        .map_err(|e| format!("CSS 語法有誤：{e}"))?;
    Ok(css)
}

pub fn read_config() -> i32 {
    match Config::load().and_then(|config| serde_json::to_string(&config).map_err(Into::into)) {
        Ok(json) => {
            println!("{json}");
            0
        }
        Err(e) => {
            eprintln!("無法讀取設定：{e}");
            1
        }
    }
}

pub fn control(args: &[String]) -> i32 {
    let active_only = args.get(2).is_some_and(|op| op == "stop-preview");
    let mut command = Command::new("systemctl");
    command.args([
        "--user",
        if active_only { "is-active" } else { "start" },
        "vitals-monitor-overlay.service",
    ]);
    match command.output() {
        Ok(output) if output.status.success() => run(args),
        _ if active_only => 0,
        Ok(output) => {
            eprintln!("{}", String::from_utf8_lossy(&output.stderr).trim());
            1
        }
        Err(e) => {
            eprintln!("無法啟用 Vitals Monitor：{e}");
            1
        }
    }
}

pub fn run(args: &[String]) -> i32 {
    let app = Application::new(
        Some("io.github.vitalsmonitor.Overlay"),
        gio::ApplicationFlags::HANDLES_COMMAND_LINE,
    );
    let state = RefCell::new(None::<Session>);
    app.connect_command_line(move |app, line| {
        let args = line.arguments();
        let argument = |i| {
            args.get(i)
                .and_then(|arg: &std::ffi::OsString| arg.to_str())
        };
        let result = (|| {
            let mut state = state.borrow_mut();
            if state.is_none() {
                *state = Some(Session::new(app)?);
            }
            if argument(1) == Some("control") {
                state
                    .as_mut()
                    .expect("initialized session")
                    .update(argument(2).ok_or("缺少操作")?, argument(3))?;
            }
            Ok::<_, String>(())
        })();
        if let Err(error) = result {
            if let Ok(message) = CString::new(error) {
                // GLib forwards stderr to the D-Bus caller. A constant format keeps CSS text inert.
                unsafe {
                    gio::ffi::g_application_command_line_printerr(
                        line.to_glib_none().0,
                        c"%s\n".as_ptr(),
                        message.as_ptr(),
                    );
                }
            }
            return 1;
        }
        0
    });
    app.run_with_args(args).value()
}
