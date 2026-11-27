use crate::{
    config::{Config, Metric},
    frames::{FrameTail, GameProbe},
    gnumon::Gnumon,
    system::System,
};
use gtk::{
    Application, DrawingArea, Grid, Label, Orientation, Window, WindowType, cairo, gdk,
    glib::{self, ControlFlow, translate::ToGlibPtr},
    prelude::*,
};
use std::{
    cell::{Cell, RefCell},
    collections::{HashMap, VecDeque},
    path::Path,
    rc::Rc,
    time::{Duration, Instant},
};

#[link(name = "gtk-layer-shell")]
unsafe extern "C" {
    fn gtk_layer_is_supported() -> libc::c_int;
    fn gtk_layer_init_for_window(window: *mut gtk::ffi::GtkWindow);
    fn gtk_layer_set_layer(window: *mut gtk::ffi::GtkWindow, layer: libc::c_int);
    fn gtk_layer_set_anchor(
        window: *mut gtk::ffi::GtkWindow,
        edge: libc::c_int,
        anchor: libc::c_int,
    );
    fn gtk_layer_set_margin(
        window: *mut gtk::ffi::GtkWindow,
        edge: libc::c_int,
        margin: libc::c_int,
    );
    fn gtk_layer_set_exclusive_zone(window: *mut gtk::ffi::GtkWindow, zone: libc::c_int);
}

type History = Rc<RefCell<VecDeque<(Instant, f64, f64)>>>;
const SPAN: Duration = Duration::from_secs(30);
const HOVER_RIM: i32 = 48;

pub struct Overlay {
    window: Window,
    content: gtk::Box,
    labels: HashMap<Metric, Label>,
    graph: Option<DrawingArea>,
    frame_caption: Option<(Option<Label>, Option<Label>)>,
    frame_waiting: Option<Label>,
    history: History,
    tail: FrameTail,
    gnumon: Gnumon,
    probe: GameProbe,
    system: System,
    config: Config,
    pid: Option<u32>,
    last_system: Option<Instant>,
    preview: bool,
    sampler: Option<glib::SourceId>,
    input: Rc<Input>,
    captured: Option<bool>,
}

impl Overlay {
    pub fn new(app: &Application, config: Config) -> Result<Rc<RefCell<Self>>, &'static str> {
        // The protocol places this surface above fullscreen windows without taking keyboard focus.
        if unsafe { gtk_layer_is_supported() } == 0 {
            return Err("The HUD needs a Wayland compositor with layer-shell support.");
        }
        let window = Window::new(WindowType::Toplevel);
        window.set_application(Some(app));
        window.set_title("Vitals Monitor overlay");
        window.set_accept_focus(false);
        window.set_app_paintable(true);
        if let Some(visual) = gtk::prelude::WidgetExt::screen(&window).and_then(|s| s.rgba_visual())
        {
            window.set_visual(Some(&visual));
        }
        window.style_context().add_class("hud-surface");
        unsafe {
            let ptr = window.to_glib_none().0;
            gtk_layer_init_for_window(ptr);
            gtk_layer_set_layer(ptr, 3);
            gtk_layer_set_exclusive_zone(ptr, 0);
        }
        let content = gtk::Box::new(Orientation::Vertical, 0);
        content.style_context().add_class("hud");
        window.add(&content);
        let input = setup_input(&window, &content, config.hover_hide);
        let overlay = Rc::new(RefCell::new(Self {
            window,
            content,
            labels: HashMap::new(),
            graph: None,
            frame_caption: None,
            frame_waiting: None,
            history: Rc::default(),
            tail: FrameTail::default(),
            gnumon: Gnumon::default(),
            probe: GameProbe::default(),
            system: System::default(),
            config,
            pid: None,
            last_system: None,
            preview: false,
            sampler: None,
            input,
            captured: None,
        }));

        let weak = Rc::downgrade(&overlay);
        glib::timeout_add_local(Duration::from_secs(2), move || {
            let Some(overlay) = weak.upgrade() else {
                return ControlFlow::Break;
            };
            Self::refresh(&overlay);
            ControlFlow::Continue
        });
        Ok(overlay)
    }

    pub fn configure(this: &Rc<RefCell<Self>>, config: Config) {
        {
            let mut hud = this.borrow_mut();
            if hud.config.timing_path != config.timing_path || hud.config.target != config.target {
                hud.tail = FrameTail::default();
                hud.history.borrow_mut().clear();
                hud.pid = None;
            }
            if hud.config.gnumon != config.gnumon {
                hud.gnumon.stop();
            }
            hud.config = config;
            hud.last_system = None;
            hud.rebuild();
        }
        Self::refresh(this);
    }

    pub fn set_preview(this: &Rc<RefCell<Self>>, enabled: bool, mut config: Config) {
        this.borrow_mut().preview = enabled;
        if enabled {
            config.hover_hide = false;
        }
        Self::configure(this, config);
    }

    fn refresh(this: &Rc<RefCell<Self>>) {
        let mut hud = this.borrow_mut();
        let pid = if hud.config.overlay || hud.preview {
            if hud.config.auto_detect {
                let blacklist: Vec<String> = hud
                    .config
                    .blacklist
                    .iter()
                    .filter(|e| e.enabled && !e.name.is_empty())
                    .map(|e| e.name.clone())
                    .collect();
                let blacklist_refs: Vec<&str> = blacklist.iter().map(|s| s.as_str()).collect();
                hud.probe.find_any(&blacklist_refs)
            } else {
                let target = hud.config.target.clone();
                hud.probe.find(&target)
            }
        } else {
            None
        };
        if hud.pid != pid {
            hud.pid = pid;
            hud.tail = FrameTail::default();
            hud.history.borrow_mut().clear();
            hud.gnumon.stop();
        }
        let visible =
            hud.preview || (hud.config.overlay && (!hud.config.auto_show || pid.is_some()));
        if visible {
            if !hud.window.is_visible() {
                hud.window.show_all();
            }
            if hud.sampler.is_none() {
                hud.sample();
                let weak = Rc::downgrade(this);
                hud.sampler = Some(glib::timeout_add_local(
                    Duration::from_millis(250),
                    move || {
                        let Some(hud) = weak.upgrade() else {
                            return ControlFlow::Break;
                        };
                        hud.borrow_mut().sample();
                        ControlFlow::Continue
                    },
                ));
            }
        } else {
            let was_visible = hud.window.is_visible() || hud.sampler.is_some();
            hud.window.hide();
            if let Some(timer) = hud.sampler.take() {
                timer.remove();
            }
            hud.gnumon.stop();
            hud.system.release();
            hud.tail = FrameTail::default();
            hud.history.borrow_mut().clear();
            hud.last_system = None;
            if was_visible {
                #[cfg(target_env = "gnu")]
                unsafe {
                    libc::malloc_trim(0);
                }
            }
        }
    }

    fn rebuild(&mut self) {
        for child in self.content.children() {
            self.content.remove(&child);
        }
        self.labels.clear();
        self.captured = None;
        self.graph = None;
        self.content.set_width_request(self.config.width);
        let right = self.config.corner % 2 == 1;
        let bottom = self.config.corner >= 2;
        unsafe {
            let ptr = self.window.to_glib_none().0;
            // Anchor only the two perpendicular corner edges so the SHM surface matches the HUD size.
            gtk_layer_set_anchor(ptr, 0, i32::from(!right));
            gtk_layer_set_anchor(ptr, 1, i32::from(right));
            gtk_layer_set_anchor(ptr, 2, i32::from(!bottom));
            gtk_layer_set_anchor(ptr, 3, i32::from(bottom));
            gtk_layer_set_margin(ptr, 0, if right { 0 } else { self.config.x });
            gtk_layer_set_margin(ptr, 1, if right { self.config.x } else { 0 });
            gtk_layer_set_margin(ptr, 2, if bottom { 0 } else { self.config.y });
            gtk_layer_set_margin(ptr, 3, if bottom { self.config.y } else { 0 });
        }
        let rim = if self.config.hover_hide { HOVER_RIM } else { 0 };
        self.content.set_halign(if right {
            gtk::Align::End
        } else {
            gtk::Align::Start
        });
        self.content.set_valign(if bottom {
            gtk::Align::End
        } else {
            gtk::Align::Start
        });
        self.content.set_margin_start(if right { rim } else { 0 });
        self.content.set_margin_end(if right { 0 } else { rim });
        self.content.set_margin_top(if bottom { rim } else { 0 });
        self.content.set_margin_bottom(if bottom { 0 } else { rim });
        let header = gtk::Box::new(Orientation::Horizontal, 0);
        header.style_context().add_class("header");
        header.pack_start(
            &hud_label("●  SYSTEM MONITOR", "header-title", 0.),
            false,
            false,
            0,
        );
        header.pack_end(&hud_label("LIVE  1s", "header-status", 1.), true, true, 0);
        self.content.pack_start(&header, false, false, 0);
        let grid = Grid::new();
        grid.set_column_spacing(if self.config.columns == 1 { 0 } else { 8 });
        grid.set_column_homogeneous(true);
        self.content.pack_start(&grid, false, false, 0);
        self.frame_caption = None;
        self.frame_waiting = None;
        let columns = self.config.columns as i32;
        for (i, tile) in (0_i32..).zip(self.config.tiles.iter().filter(|tile| tile.enabled)) {
            let frame = tile.metric == Metric::FrameTime;
            let name = hud_label(
                if frame {
                    "FRAME TIME"
                } else {
                    tile.metric.title()
                },
                if frame { "frame-title" } else { "metric-name" },
                0.,
            );
            if !frame {
                name.style_context()
                    .add_class(if tile.metric == Metric::Fps {
                        "gpu"
                    } else {
                        tile.metric.class()
                    });
            }
            let value = hud_label(
                if tile.metric == Metric::Fps {
                    "-- / --"
                } else {
                    "--"
                },
                if frame { "frame-value" } else { "metric-value" },
                1.,
            );
            if columns > 1 {
                value.set_ellipsize(gtk::pango::EllipsizeMode::End);
            }
            let row = gtk::Box::new(Orientation::Horizontal, 0);
            row.pack_start(&name, false, false, 0);
            if !frame || tile.wants_field("avg") {
                row.pack_end(&value, true, true, 0);
                self.labels.insert(tile.metric, value);
            }
            if frame {
                let show_graph = tile.wants_field("graph");
                let show_p95 = tile.wants_field("p95");
                let show_peak = tile.wants_field("peak");
                let panel = gtk::Box::new(Orientation::Vertical, 5);
                panel.style_context().add_class("frame-history");
                panel.pack_start(&row, false, false, 0);
                if show_graph {
                    let frame_overlay = gtk::Overlay::new();
                    let graph = chart(Rc::clone(&self.history));
                    frame_overlay.add(&graph);
                    let waiting = hud_label("WAITING FOR FRAME DATA", "frame-waiting", 0.5);
                    frame_overlay.add_overlay(&waiting);
                    frame_overlay.set_overlay_pass_through(&waiting, true);
                    panel.pack_start(&frame_overlay, false, false, 0);
                    self.graph = Some(graph);
                    self.frame_waiting = Some(waiting);
                } else {
                    self.history.borrow_mut().clear();
                }
                if show_graph || show_p95 || show_peak {
                    let captions = gtk::Box::new(Orientation::Horizontal, 0);
                    let scale = if show_graph {
                        let l = hud_label("30s   0–40 ms", "frame-caption", 0.);
                        captions.pack_start(&l, false, false, 0);
                        Some(l)
                    } else {
                        None
                    };
                    let peak = if show_p95 || show_peak {
                        let l = hud_label("PEAK  --", "frame-caption", 1.);
                        captions.pack_end(&l, true, true, 0);
                        Some(l)
                    } else {
                        None
                    };
                    panel.pack_start(&captions, false, false, 0);
                    self.frame_caption = Some((scale, peak));
                }
                grid.attach(&panel, i % columns, i / columns, 1, 1);
            } else {
                row.style_context().add_class("metric");
                row.style_context().add_class(tile.metric.class());
                grid.attach(&row, i % columns, i / columns, 1, 1);
            }
        }
        self.window.resize(1, 1);
        self.content.show_all();
        self.sync_input();
    }

    fn sync_input(&self) {
        self.input.enabled.set(self.config.hover_hide);
        self.input.apply(&self.window);
    }

    fn sample(&mut self) {
        let wants_frames = self.config.wants(Metric::Fps)
            || self.config.wants(Metric::FrameTime)
            || self.config.wants(Metric::FrameStats)
            || self.config.wants(Metric::Dlss);
        if wants_frames {
            let frames = self
                .pid
                .and_then(|_| self.tail.sample(Path::new(&self.config.timing_path)));
            if let Some(label) = self.labels.get(&Metric::Fps) {
                let want_native = self.config.wants_field(Metric::Fps, "native");
                let want_output = self.config.wants_field(Metric::Fps, "output");
                let capture = if want_output {
                    self.gnumon.sample(Path::new(&self.config.gnumon), self.pid)
                } else {
                    self.gnumon.stop();
                    None
                };
                let left = frames
                    .map(|f| format!("{:.0}", f.render))
                    .unwrap_or_else(|| "--".into());
                let right = capture
                    .or_else(|| frames.and_then(|f| f.generated))
                    .map(|v| format!("{v:.0}"))
                    .unwrap_or_else(|| "--".into());
                let text = match (want_native, want_output) {
                    (true, true) => format!("{left} / {right}"),
                    (true, false) => left,
                    (false, true) => right,
                    (false, false) => "--".into(),
                };
                set(label, &text);
                if self.captured != Some(capture.is_some()) {
                    self.captured = Some(capture.is_some());
                    label.set_tooltip_text(Some(if capture.is_some() {
                        "Native (NGX) / GNUMON captured FPS"
                    } else {
                        "Native / estimated output from logged generated frames"
                    }));
                }
            } else {
                self.gnumon.stop();
            }
            if let Some(label) = self.labels.get(&Metric::FrameStats) {
                let want_p50 = self.config.wants_field(Metric::FrameStats, "p50");
                let want_p95 = self.config.wants_field(Metric::FrameStats, "p95");
                let want_low = self.config.wants_field(Metric::FrameStats, "low");
                let text = frames.map_or_else(
                    || "--".into(),
                    |f| {
                        let mut parts = Vec::with_capacity(3);
                        if want_p50 {
                            let v = f
                                .p50_ms
                                .map_or("--".into(), |ms| format!("{:.0}", 1000. / ms));
                            parts.push(format!("P50 {v}"));
                        }
                        if want_p95 {
                            let v = f
                                .p95_ms
                                .map_or("--".into(), |ms| format!("{:.0}", 1000. / ms));
                            parts.push(format!("P95 {v}"));
                        }
                        if want_low {
                            let v = f
                                .low_1pct_fps
                                .map_or("--".into(), |fps| format!("{fps:.0}"));
                            parts.push(format!("LOW {v}"));
                        }
                        if parts.is_empty() {
                            "--".into()
                        } else {
                            parts.join("  ")
                        }
                    },
                );
                set(label, &text);
            }
            if let Some(label) = self.labels.get(&Metric::Dlss) {
                let want_sr = self.config.wants_field(Metric::Dlss, "sr");
                let want_fg = self.config.wants_field(Metric::Dlss, "fg");
                let want_res = self.config.wants_field(Metric::Dlss, "res");
                let text = frames.map_or_else(
                    || "--".into(),
                    |f| {
                        let mut parts = Vec::with_capacity(3);
                        if want_sr {
                            parts.push(f.sr_ms.map_or("SR --".into(), |v| format!("SR {v:.2}ms")));
                        }
                        if want_fg {
                            parts.push(match (f.fg_ms, f.fg_factor) {
                                (Some(ms), Some(n)) => format!("FG {ms:.2}ms x{n}"),
                                (Some(ms), None) => format!("FG {ms:.2}ms"),
                                (None, Some(n)) => format!("FG x{n}"),
                                (None, None) => "FG off".into(),
                            });
                        }
                        if want_res {
                            match (f.sr_res, f.out_res) {
                                (Some((sw, sh)), Some((ow, oh))) => {
                                    parts.push(format!("{sw}x{sh}→{ow}x{oh}"))
                                }
                                (Some((sw, sh)), None) => parts.push(format!("{sw}x{sh}")),
                                _ => {}
                            }
                        }
                        if parts.is_empty() {
                            "--".into()
                        } else {
                            parts.join("  ")
                        }
                    },
                );
                set(label, &text);
            }
            if self.config.wants(Metric::FrameTime) {
                if let Some(label) = self.labels.get(&Metric::FrameTime) {
                    let value = frames
                        .map(|f| format!("{:.2} ms", f.average_ms))
                        .unwrap_or_else(|| "--".into());
                    set(label, &value);
                }
                let updated = if self.graph.is_some() {
                    let mut history = self.history.borrow_mut();
                    let updated = if let Some((peak, avg)) =
                        frames.and_then(|f| f.batch_peak_ms.map(|p| (p, f.average_ms)))
                    {
                        history.push_back((Instant::now(), avg, peak));
                        true
                    } else if frames.is_none() && !history.is_empty() {
                        history.clear();
                        true
                    } else {
                        false
                    };
                    while history.front().is_some_and(|s| s.0.elapsed() > SPAN) {
                        history.pop_front();
                    }
                    if let Some((Some(scale), _)) = &self.frame_caption {
                        set(scale, &format!("30s   0–{:.0} ms", chart_scale(&history)));
                    }
                    updated
                } else {
                    false
                };
                if let Some(waiting) = &self.frame_waiting {
                    waiting.set_visible(frames.is_none());
                }
                if let Some((_, Some(peak))) = &self.frame_caption {
                    let want_p95 = self.config.wants_field(Metric::FrameTime, "p95");
                    let want_peak = self.config.wants_field(Metric::FrameTime, "peak");
                    let caption = match frames {
                        None => match (want_p95, want_peak) {
                            (true, false) => "P95  --".into(),
                            (false, true) | (true, true) => "PEAK  --".into(),
                            (false, false) => String::new(),
                        },
                        Some(f) => match (want_p95, want_peak, f.p95_ms) {
                            (true, true, Some(p95)) => {
                                format!("P95 {:.1}  PEAK {:.1} ms", p95, f.peak_ms)
                            }
                            (true, true, None) | (false, true, _) => {
                                format!("PEAK  {:.2} ms", f.peak_ms)
                            }
                            (true, false, Some(p95)) => format!("P95  {p95:.2} ms"),
                            (true, false, None) => "P95  --".into(),
                            (false, false, _) => String::new(),
                        },
                    };
                    set(peak, &caption);
                }
                if updated && let Some(graph) = &self.graph {
                    graph.queue_draw();
                }
            }
        } else {
            self.gnumon.stop();
        }
        if self
            .last_system
            .is_none_or(|t| t.elapsed() >= Duration::from_millis(950))
        {
            self.last_system = Some(Instant::now());
            self.system.sample(&self.config, self.pid, |metric, value| {
                if let Some(label) = self.labels.get(&metric) {
                    set(label, &value);
                }
            });
        }
    }
}

fn hud_label(text: &str, class: &str, align: f32) -> Label {
    let label = Label::new(Some(text));
    label.set_xalign(align);
    label.style_context().add_class(class);
    label
}

fn chart_scale(history: &VecDeque<(Instant, f64, f64)>) -> f64 {
    let peak = history.iter().map(|s| s.2).fold(0_f64, f64::max);
    (peak * 1.1 / 10.).ceil().max(4.) * 10.
}

fn set(label: &Label, value: &str) {
    if label.text().as_str() != value {
        label.set_text(value);
    }
}

fn chart(history: History) -> DrawingArea {
    let graph = DrawingArea::new();
    graph.set_height_request(35);
    graph.style_context().add_class("chart");
    graph.style_context().add_class("frame-grid");
    graph.set_tooltip_text(Some(
        "Last 30 seconds. Bright: sampled spikes. Dim: rolling average. Measured at NGX Evaluate.",
    ));
    graph.connect_draw(move |area, cr| {
        let w = f64::from(area.allocated_width());
        let h = f64::from(area.allocated_height());
        let foreground = area.style_context().color(gtk::StateFlags::NORMAL);
        let color = |alpha| {
            cr.set_source_rgba(
                foreground.red(),
                foreground.green(),
                foreground.blue(),
                alpha * foreground.alpha(),
            )
        };
        let style = area.style_context();
        gtk::render_background(&style, cr, 0., 0., w, h);
        gtk::render_frame(&style, cr, 0., 0., w, h);
        let history = history.borrow();
        let max = chart_scale(&history);
        for (peak, alpha) in [(false, 0.32), (true, 1.)] {
            color(alpha);
            cr.set_line_width(if peak { 1.8 } else { 1. });
            for (i, &(time, average, high)) in history.iter().enumerate() {
                let x = w * (1. - time.elapsed().as_secs_f64() / SPAN.as_secs_f64());
                let y = h * (1. - if peak { high } else { average } / max);
                if i == 0 {
                    cr.move_to(x, y);
                } else {
                    cr.line_to(x, y);
                }
            }
            let _ = cr.stroke();
        }
        glib::Propagation::Proceed
    });
    graph
}

struct Input {
    content: glib::WeakRef<gtk::Box>,
    enabled: Cell<bool>,
    hidden: Cell<bool>,
}
impl Input {
    fn bounds(&self, window: &Window) -> Option<cairo::RectangleInt> {
        let content = self.content.upgrade()?;
        let (x, y) = content.translate_coordinates(window, 0, 0)?;
        Some(cairo::RectangleInt::new(
            x,
            y,
            content.allocated_width(),
            content.allocated_height(),
        ))
    }
    fn apply(&self, window: &Window) {
        let (Some(surface), Some(bounds), Some(content)) =
            (window.window(), self.bounds(window), self.content.upgrade())
        else {
            return;
        };
        if !self.enabled.get() {
            self.hidden.set(false);
        }
        let region = if !self.enabled.get() {
            cairo::Region::create()
        } else if self.hidden.get() {
            let region = cairo::Region::create_rectangle(&cairo::RectangleInt::new(
                0,
                0,
                window.allocated_width(),
                window.allocated_height(),
            ));
            let _ = region.subtract_rectangle(&bounds);
            region
        } else {
            cairo::Region::create_rectangle(&bounds)
        };
        surface.input_shape_combine_region(&region, 0, 0);
        content.set_opacity(if self.hidden.get() { 0. } else { 1. });
    }
}

fn setup_input(window: &Window, content: &gtk::Box, enabled: bool) -> Rc<Input> {
    let state = Rc::new(Input {
        content: content.downgrade(),
        enabled: Cell::new(enabled),
        hidden: Cell::new(false),
    });
    window.add_events(gdk::EventMask::ENTER_NOTIFY_MASK | gdk::EventMask::LEAVE_NOTIFY_MASK);
    let input = Rc::clone(&state);
    let crossing = move |window: &Window, event: &gdk::EventCrossing| {
        if input.enabled.get() && event.detail() != gdk::NotifyType::Inferior {
            let (x, y) = event.position();
            if let Some(b) = input.bounds(window) {
                input.hidden.set(
                    x >= f64::from(b.x())
                        && y >= f64::from(b.y())
                        && x < f64::from(b.x() + b.width())
                        && y < f64::from(b.y() + b.height()),
                );
            }
            input.apply(window);
        }
        glib::Propagation::Proceed
    };
    window.connect_enter_notify_event(crossing.clone());
    window.connect_leave_notify_event(crossing);
    let input = Rc::clone(&state);
    let weak_window = window.downgrade();
    content.connect_size_allocate(move |_, _| {
        if let Some(window) = weak_window.upgrade() {
            input.apply(&window);
        }
    });
    let input = Rc::clone(&state);
    window.connect_map(move |window| input.apply(window));
    state
}
