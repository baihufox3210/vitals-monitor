mod config;
mod dms;
mod frames;
mod gnumon;
mod overlay;
mod settings;
mod system;

use gtk::{Application, prelude::*};

fn main() {
    let mut args = std::env::args_os().skip(1);
    let command = args.next();
    if command.as_deref().is_some_and(|arg| arg == "--sample") {
        let Some(path) = args.next() else {
            eprintln!("Usage: vitals-monitor --sample PATH");
            std::process::exit(2);
        };
        let mut tail = frames::FrameTail::default();
        let Some(sample) = tail.sample(std::path::Path::new(&path)) else {
            eprintln!("No recent frame data");
            std::process::exit(1);
        };
        println!(
            "Native {:.2} FPS, output {}, average {:.2} ms, peak {:.2} ms",
            sample.render,
            sample
                .generated
                .map_or("--".into(), |fps| format!("~{fps:.2} FPS")),
            sample.average_ms,
            sample.peak_ms
        );
        return;
    }

    match command.and_then(|arg| arg.into_string().ok()).as_deref() {
        Some("--read-config") => std::process::exit(dms::read_config()),
        Some("--nvml") => std::process::exit(system::nvml_worker()),
        Some("--hud") => std::process::exit(dms::run(&["vitals-monitor".into(), "serve".into()])),
        Some("--control") => {
            let mut command = vec!["vitals-monitor".into(), "control".into()];
            command.extend(args.map(|arg| arg.to_string_lossy().into_owned()));
            std::process::exit(dms::control(&command));
        }
        _ => (),
    }

    let app = Application::new(
        Some("io.github.vitalsmonitor.VitalsMonitor"),
        Default::default(),
    );
    app.connect_activate(|app| {
        if let Some(window) = app
            .windows()
            .into_iter()
            .find(|w| w.widget_name() == "vitals-monitor-settings")
        {
            window.present();
            return;
        }
        settings::open(app);
    });
    app.run();
}
