use crate::{
    config::{Config, Metric, Tile},
    overlay::Overlay,
};
use gtk::{
    Align, Application, ApplicationWindow, Box as GtkBox, Button, CheckButton, ComboBoxText,
    CssProvider, Entry, FlowBox, Frame, Label, ListBox, Orientation, ScrolledWindow, SpinButton,
    TextView, gdk, glib, prelude::*,
};
use std::{cell::RefCell, collections::HashSet, rc::Rc};

const THEME: &str = include_str!("theme.css");

fn label(text: &str, class: &str) -> Label {
    let label = Label::new(Some(text));
    label.set_xalign(0.);
    label.style_context().add_class(class);
    label
}

fn button(text: &str, class: &str) -> Button {
    let button = Button::with_label(text);
    button.style_context().add_class(class);
    button
}

fn field(group: &GtkBox, title: &str, widget: &impl IsA<gtk::Widget>) {
    let row = GtkBox::new(Orientation::Horizontal, 12);
    row.pack_start(&label(title, "field-title"), false, false, 0);
    row.pack_end(widget, true, true, 0);
    group.pack_start(&row, false, false, 0);
}

fn page(title: &str, description: &str) -> GtkBox {
    let box_ = GtkBox::new(Orientation::Vertical, 12);
    box_.style_context().add_class("page");
    box_.pack_start(&label(title, "section-title"), false, false, 0);
    let subtitle = label(description, "muted");
    subtitle.set_line_wrap(true);
    box_.pack_start(&subtitle, false, false, 0);
    box_
}

fn description(metric: Metric) -> &'static str {
    match metric {
        Metric::Fps => "原生渲染與輸出影格率",
        Metric::FrameStats => "P50、P95 與低點影格率穩定度",
        Metric::Dlss => "SR / FG GPU 耗時、倍率與內部解析度",
        Metric::FrameTime => "平均影格時間、尖峰與 30 秒波形",
        Metric::Gpu => "使用率、溫度、時脈、功耗與風扇轉速",
        Metric::Vram => "顯示記憶體用量與記憶體時脈",
        Metric::Cpu => "使用率、溫度與最高核心時脈",
        Metric::Ram => "系統與 Swap 記憶體用量",
        Metric::GameProc => "遊戲程序 CPU、記憶體、執行緒與執行時間",
        Metric::Disk => "遊戲程序磁碟讀寫速率與 NVMe 溫度",
        Metric::Network => "實體網路卡即時下載與上傳速率",
    }
}

fn field_label(metric: Metric, id: &str) -> &str {
    metric
        .field_defs()
        .iter()
        .find(|&&(k, _)| k == id)
        .map_or(id, |&(_, name)| name)
}

fn drop_index(source: usize, target: usize, after: bool, count: usize) -> Option<usize> {
    if source >= count || target >= count {
        return None;
    }
    let slot = target + usize::from(after);
    Some(slot - usize::from(source < slot))
}

fn move_tile(
    list: &ListBox,
    tiles: &Rc<RefCell<Vec<Tile>>>,
    expanded: &Rc<RefCell<HashSet<Metric>>>,
    from: usize,
    to: usize,
    focus: bool,
) {
    if from != to {
        let mut state = tiles.borrow_mut();
        let tile = state.remove(from);
        state.insert(to, tile);
        drop(state);
        tile_list(list, tiles, expanded);
    }
    if focus
        && let Some(handle) = list
            .row_at_index(to as i32)
            .and_then(|row| row.child())
            .and_then(|child| child.downcast::<GtkBox>().ok())
            .and_then(|wrapper| wrapper.children().first().cloned())
            .and_then(|w| w.downcast::<GtkBox>().ok())
            .and_then(|row| row.children().first().cloned())
    {
        handle.grab_focus();
    }
}

fn tile_list(
    list: &ListBox,
    tiles: &Rc<RefCell<Vec<Tile>>>,
    expanded: &Rc<RefCell<HashSet<Metric>>>,
) {
    for child in list.children() {
        list.remove(&child);
    }
    for (index, tile) in tiles.borrow().iter().enumerate() {
        let row = gtk::ListBoxRow::new();
        row.set_activatable(false);
        row.set_selectable(false);
        row.style_context().add_class("tile-row");
        let wrapper = GtkBox::new(Orientation::Vertical, 4);
        row.add(&wrapper);

        let content = GtkBox::new(Orientation::Horizontal, 12);
        wrapper.pack_start(&content, false, false, 0);

        let grip = gtk::EventBox::new();
        grip.set_can_focus(true);
        grip.style_context().add_class("grip");
        grip.set_tooltip_text(Some("拖曳重新排序，或聚焦後按方向鍵"));
        if let Some(accessible) = grip.accessible() {
            accessible.set_name(&format!("Reorder {}", tile.metric.title()));
        }
        let dots = gtk::DrawingArea::new();
        dots.set_size_request(16, 20);
        dots.connect_draw(|widget, cr| {
            let color = widget.style_context().color(gtk::StateFlags::NORMAL);
            cr.set_source_rgba(color.red(), color.green(), color.blue(), color.alpha());
            let x = f64::from(widget.allocated_width()) / 2.;
            let y = f64::from(widget.allocated_height()) / 2.;
            for dx in [-2.6, 2.6] {
                for dy in [-5.2, 0., 5.2] {
                    cr.new_sub_path();
                    cr.arc(x + dx, y + dy, 1.35, 0., std::f64::consts::TAU);
                }
            }
            let _ = cr.fill();
            glib::Propagation::Proceed
        });
        grip.add(&dots);
        grip.connect_enter_notify_event(|button, _| {
            if let Some(window) = button.window() {
                window.set_cursor(gdk::Cursor::from_name(&window.display(), "grab").as_ref());
            }
            glib::Propagation::Proceed
        });
        content.pack_start(&grip, false, false, 0);
        let words = GtkBox::new(Orientation::Vertical, 2);
        words.set_valign(Align::Center);
        words.pack_start(&label(tile.metric.title(), "tile-name"), false, false, 0);
        words.pack_start(
            &label(description(tile.metric), "tile-description"),
            false,
            false,
            0,
        );
        content.pack_start(&words, true, true, 0);

        let enabled_count = tile.fields.iter().filter(|f| f.enabled).count();
        let total_count = tile.fields.len();
        let details_btn = button(&format!("細項 {enabled_count}/{total_count}"), "quiet");
        details_btn.set_valign(Align::Center);

        let toggle = gtk::Switch::new();
        toggle.set_valign(Align::Center);
        toggle.set_active(tile.enabled);
        if let Some(accessible) = toggle.accessible() {
            accessible.set_name(&format!("Show {}", tile.metric.title()));
        }
        let state = Rc::clone(tiles);
        toggle.connect_active_notify(move |toggle| {
            state.borrow_mut()[index].enabled = toggle.is_active()
        });
        content.pack_end(&toggle, false, false, 0);
        content.pack_end(&details_btn, false, false, 0);

        let fields_box = FlowBox::new();
        fields_box.set_selection_mode(gtk::SelectionMode::None);
        fields_box.set_max_children_per_line(4);
        fields_box.set_row_spacing(4);
        fields_box.set_column_spacing(10);
        fields_box.set_margin_start(34);
        fields_box.set_margin_bottom(6);
        fields_box.set_no_show_all(true);
        let is_open = expanded.borrow().contains(&tile.metric);
        fields_box.set_visible(is_open);

        for (fi, f) in tile.fields.iter().enumerate() {
            let cb = check(field_label(tile.metric, &f.id), f.enabled);
            cb.show();
            let state = Rc::clone(tiles);
            let btn_ref = details_btn.clone();
            cb.connect_toggled(move |cb| {
                let mut st = state.borrow_mut();
                if let Some(field) = st.get_mut(index).and_then(|t| t.fields.get_mut(fi)) {
                    field.enabled = cb.is_active();
                }
                if let Some(t) = st.get(index) {
                    let on = t.fields.iter().filter(|x| x.enabled).count();
                    btn_ref.set_label(&format!("細項 {on}/{}", t.fields.len()));
                }
            });
            fields_box.add(&cb);
        }
        wrapper.pack_start(&fields_box, false, false, 0);

        let exp_state = Rc::clone(expanded);
        let metric = tile.metric;
        let box_ref = fields_box.clone();
        details_btn.connect_clicked(move |_| {
            let mut set = exp_state.borrow_mut();
            let show = !set.contains(&metric);
            if show {
                set.insert(metric);
            } else {
                set.remove(&metric);
            }
            box_ref.set_visible(show);
        });

        let weak_list = list.downgrade();
        let state = Rc::clone(tiles);
        let exp = Rc::clone(expanded);
        grip.connect_key_press_event(move |_, event| {
            let count = state.borrow().len();
            let destination = match event.keyval() {
                gdk::keys::constants::Up => index.checked_sub(1),
                gdk::keys::constants::Down => (index + 1 < count).then_some(index + 1),
                _ => return glib::Propagation::Proceed,
            };
            if let (Some(to), Some(list)) = (destination, weak_list.upgrade()) {
                move_tile(&list, &state, &exp, index, to, true);
            }
            glib::Propagation::Stop
        });

        attach_drag(&grip, list, tiles, expanded, index);
        list.add(&row);
    }
    list.show_all();
}

fn attach_drag(
    grip: &gtk::EventBox,
    list: &ListBox,
    tiles: &Rc<RefCell<Vec<Tile>>>,
    expanded: &Rc<RefCell<HashSet<Metric>>>,
    index: usize,
) {
    let gesture = gtk::GestureDrag::new(grip);
    gesture.set_button(1);
    gesture.set_propagation_phase(gtk::PropagationPhase::Capture);
    let origin = Rc::new(std::cell::Cell::new(None));
    let weak_list = list.downgrade();
    let weak_grip = grip.downgrade();
    let start = Rc::clone(&origin);
    gesture.connect_drag_begin(move |gesture, x, y| {
        if let (Some(grip), Some(list)) = (weak_grip.upgrade(), weak_list.upgrade()) {
            start.set(
                grip.translate_coordinates(&list, x.round() as i32, y.round() as i32)
                    .map(|(_, y)| y),
            );
            gesture.set_state(gtk::EventSequenceState::Claimed);
        }
    });
    let weak_list = list.downgrade();
    let start = Rc::clone(&origin);
    gesture.connect_drag_update(move |_, _, dy| {
        if let Some(list) = weak_list.upgrade() {
            let slot = start
                .get()
                .filter(|_| dy.abs() >= 4.)
                .and_then(|y| insertion_at(&list, y + dy.round() as i32));
            for row in list
                .children()
                .into_iter()
                .filter_map(|w| w.downcast::<gtk::ListBoxRow>().ok())
            {
                let side = slot
                    .filter(|(target, _)| row.index() as usize == *target)
                    .map(|(_, after)| after);
                mark_drop(&row, side);
            }
        }
    });
    let weak_list = list.downgrade();
    let state = Rc::clone(tiles);
    let exp = Rc::clone(expanded);
    gesture.connect_drag_end(move |_, _, dy| {
        let Some(list) = weak_list.upgrade() else {
            return;
        };
        clear_drop(&list);
        let count = state.borrow().len();
        if let Some((target, after)) = origin
            .take()
            .filter(|_| dy.abs() >= 4.)
            .and_then(|y| insertion_at(&list, y + dy.round() as i32))
            && let Some(to) = drop_index(index, target, after, count)
        {
            let state = Rc::clone(&state);
            let exp = Rc::clone(&exp);
            glib::idle_add_local_once(move || move_tile(&list, &state, &exp, index, to, true));
        }
    });
    let weak_list = list.downgrade();
    gesture.connect_cancel(move |_, _| {
        if let Some(list) = weak_list.upgrade() {
            clear_drop(&list);
        }
    });
    grip.connect_destroy(move |_| {
        let _ = &gesture;
    });
}

fn insertion_at(list: &ListBox, y: i32) -> Option<(usize, bool)> {
    let y = y.clamp(0, list.allocated_height().saturating_sub(1));
    if let Some(row) = list.row_at_y(y) {
        let (_, row_y) = row.translate_coordinates(list, 0, 0)?;
        return Some((
            row.index() as usize,
            y >= row_y + row.allocated_height() / 2,
        ));
    }
    if y <= 2 {
        return Some((0, false));
    }
    let row = list
        .children()
        .last()?
        .clone()
        .downcast::<gtk::ListBoxRow>()
        .ok()?;
    Some((row.index() as usize, true))
}

fn clear_drop(list: &ListBox) {
    for row in list
        .children()
        .into_iter()
        .filter_map(|w| w.downcast::<gtk::ListBoxRow>().ok())
    {
        mark_drop(&row, None);
    }
}

fn mark_drop(row: &gtk::ListBoxRow, after: Option<bool>) {
    let style = row.style_context();
    for (class, side) in [("drop-before", false), ("drop-after", true)] {
        if after == Some(side) {
            style.add_class(class);
        } else {
            style.remove_class(class);
        }
    }
}

fn check(title: &str, active: bool) -> CheckButton {
    let check = CheckButton::with_label(title);
    check.set_active(active);
    check
}

fn number(value: i32, min: f64, max: f64) -> SpinButton {
    let widget = SpinButton::with_range(min, max, 1.);
    widget.set_value(value.into());
    widget
}

fn entry(value: &str) -> Entry {
    let entry = Entry::new();
    entry.set_width_chars(24);
    entry.set_text(value);
    entry
}

fn stylesheet(css: &str) -> Result<CssProvider, String> {
    let provider = CssProvider::new();
    provider
        .load_from_data(css.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(provider)
}

pub fn open(app: &Application) {
    let (config, error) = match Config::load() {
        Ok(config) => (config, None),
        Err(e) => (
            Config::default(),
            Some(format!("Could not load settings: {e}")),
        ),
    };
    let screen = gdk::Screen::default().expect("GTK display");
    gtk::StyleContext::add_provider_for_screen(
        &screen,
        &stylesheet(THEME).expect("built-in CSS"),
        600,
    );
    let custom = stylesheet(&config.css).ok();
    if let Some(provider) = &custom {
        gtk::StyleContext::add_provider_for_screen(&screen, provider, 800);
    }
    let active_css = Rc::new(RefCell::new(custom));
    let overlay = Overlay::new(app, config.clone());
    if let Ok(hud) = &overlay {
        Overlay::configure(hud, config.clone());
    }

    let window = ApplicationWindow::new(app);
    window.set_title("Vitals Monitor 設定");
    window.set_widget_name("vitals-monitor-settings");
    window.set_default_size(790, 660);
    let titlebar = gtk::HeaderBar::new();
    titlebar.set_show_close_button(true);
    titlebar.style_context().add_class("window-bar");
    titlebar.set_title(Some("Vitals Monitor"));
    window.set_titlebar(Some(&titlebar));
    window.style_context().add_class("settings");
    let root = GtkBox::new(Orientation::Vertical, 0);
    window.add(&root);

    let workspace = GtkBox::new(Orientation::Horizontal, 0);
    root.pack_start(&workspace, true, true, 0);
    let sidebar = GtkBox::new(Orientation::Vertical, 16);
    sidebar.style_context().add_class("sidebar");
    sidebar.set_width_request(158);
    workspace.pack_start(&sidebar, false, false, 0);
    sidebar.pack_start(&label("設定", "nav-heading"), false, false, 0);
    let stack = gtk::Stack::new();
    let navigation = gtk::StackSidebar::new();
    navigation.set_stack(&stack);
    navigation.style_context().add_class("navigation");
    sidebar.pack_start(&navigation, false, false, 0);
    let preview = button("預覽面板", "quiet");
    preview.set_sensitive(overlay.is_ok());

    let detail = GtkBox::new(Orientation::Vertical, 0);
    detail.set_halign(Align::Center);
    detail.set_width_request(560);
    detail.pack_start(&stack, true, true, 0);
    workspace.pack_start(&detail, true, true, 0);

    let layout = page(
        "面板配置",
        "拖曳左側把手即可排序。展開「細項」可勾選要顯示的子項目，或用開關切換整組。",
    );
    let list = ListBox::new();
    list.set_selection_mode(gtk::SelectionMode::None);
    list.style_context().add_class("tile-list");
    let tiles = Rc::new(RefCell::new(config.tiles.clone()));
    let expanded = Rc::new(RefCell::new(HashSet::new()));
    tile_list(&list, &tiles, &expanded);
    layout.pack_start(&list, false, false, 0);
    let columns = number(config.columns as i32, 1., 3.);
    let width = number(config.width, 180., 1000.);
    let x = number(config.x, 0., 500.);
    let y = number(config.y, 0., 500.);
    let corner = ComboBoxText::new();
    for text in ["左上", "右上", "左下", "右下"] {
        corner.append_text(text);
    }
    corner.set_active(Some(config.corner));
    layout.pack_start(&label("位置與尺寸", "group-title"), false, false, 0);
    let positions = GtkBox::new(Orientation::Horizontal, 12);
    for (text, spin) in [("欄數", &columns), ("寬度", &width), ("X", &x), ("Y", &y)] {
        let column = GtkBox::new(Orientation::Vertical, 4);
        column.pack_start(&label(text, "muted"), false, false, 0);
        column.pack_start(spin, true, true, 0);
        positions.pack_start(&column, true, true, 0);
    }
    layout.pack_start(&positions, false, false, 0);
    field(&layout, "對齊角落", &corner);
    add_page(&stack, &layout, "layout", "配置");

    let style = page(
        "自訂樣式",
        "用 GTK CSS 調整遊戲內面板。可從 .hud、.metric 或 .chart 開始。",
    );
    let css = TextView::new();
    css.set_monospace(true);
    css.set_wrap_mode(gtk::WrapMode::WordChar);
    let buffer = css.buffer().expect("text buffer");
    buffer.set_text(&config.css);
    let editor = ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    editor.set_min_content_height(200);
    editor.add(&css);
    let frame = Frame::new(None);
    frame.style_context().add_class("editor");
    frame.add(&editor);
    style.pack_start(&frame, true, true, 0);
    let example = button("插入範例", "quiet");
    example.set_halign(Align::Start);
    style.pack_start(&example, false, false, 0);
    let example_buffer = buffer.clone();
    example.connect_clicked(move |_| {
        example_buffer.insert_at_cursor(".hud { background-color: rgba(12, 17, 25, 0.85); }\n.hud .metric-value { color: #f1f5fa; }\n.hud .chart { color: #62dda6; }\n");
    });
    add_page(&stack, &style, "style", "樣式");

    let sources = page(
        "顯示與資料來源",
        "設定何時顯示面板，以及要讀取的遊戲與統計來源。",
    );
    let enabled = gtk::Switch::new();
    enabled.set_active(config.overlay);
    enabled.set_valign(Align::Center);
    if let Some(accessible) = enabled.accessible() {
        accessible.set_name("啟用面板");
    }
    let master = GtkBox::new(Orientation::Horizontal, 10);
    master.style_context().add_class("master");
    master.pack_start(&label("啟用遊戲內面板", "field-title"), true, true, 0);
    master.pack_end(&enabled, false, false, 0);
    layout.pack_end(&master, false, false, 0);
    let auto = check("僅在目標遊戲運行時顯示", config.auto_show);
    let hide = check("滑鼠移入時隱藏並允許點擊穿透", config.hover_hide);
    let behavior = GtkBox::new(Orientation::Vertical, 10);
    behavior.style_context().add_class("settings-group");
    for control in [&auto, &hide] {
        behavior.pack_start(control, false, false, 0);
    }
    sources.pack_start(&behavior, false, false, 0);
    sources.pack_start(&label("資料來源", "group-title"), false, false, 0);
    let target = entry(&config.target);
    let timing = entry(&config.timing_path);
    let gnumon = entry(&config.gnumon);
    field(&sources, "遊戲執行檔", &target);
    field(&sources, "NGX 影格日誌", &timing);
    field(&sources, "GNUMON CLI", &gnumon);
    let note = label(
        "FPS 依序顯示原生與輸出幀率。沒有 GNUMON 資料時，輸出幀率由 NGX 日誌估算。",
        "muted",
    );
    note.set_line_wrap(true);
    sources.pack_start(&note, false, false, 0);
    add_page(&stack, &sources, "sources", "來源");

    let footer = GtkBox::new(Orientation::Horizontal, 10);
    footer.style_context().add_class("footer");
    let status = label(
        &error.unwrap_or_else(|| overlay.as_ref().err().copied().unwrap_or("").to_string()),
        "muted",
    );
    status.set_ellipsize(gtk::pango::EllipsizeMode::End);
    footer.pack_start(&status, true, true, 0);
    let save = button("套用變更", "primary");
    footer.pack_end(&save, false, false, 0);
    footer.pack_end(&preview, false, false, 0);
    let quit = button("結束 Vitals Monitor", "quiet");
    sidebar.pack_end(&quit, false, false, 0);
    root.pack_end(&footer, false, false, 0);

    let keep_running = Rc::new(std::cell::Cell::new(config.overlay && overlay.is_ok()));
    let can_hide = Rc::clone(&keep_running);
    let close_app = app.downgrade();
    window.connect_delete_event(move |window, _| {
        if can_hide.get() {
            window.hide();
            glib::Propagation::Stop
        } else {
            if let Some(app) = close_app.upgrade() {
                app.quit();
            }
            glib::Propagation::Proceed
        }
    });
    let app_weak = app.downgrade();
    quit.connect_clicked(move |_| {
        if let Some(app) = app_weak.upgrade() {
            app.quit();
        }
    });

    let read_draft = Rc::new(move || Config {
        tiles: tiles.borrow().clone(),
        columns: columns.value_as_int() as u32,
        width: width.value_as_int(),
        x: x.value_as_int(),
        y: y.value_as_int(),
        corner: corner.active().unwrap_or(0),
        hover_hide: hide.is_active(),
        overlay: enabled.is_active(),
        auto_show: auto.is_active(),
        auto_detect: false,
        css: buffer
            .text(&buffer.start_iter(), &buffer.end_iter(), true)
            .unwrap_or_default()
            .to_string(),
        target: target.text().trim().to_string(),
        timing_path: timing.text().trim().to_string(),
        gnumon: gnumon.text().trim().to_string(),
        blacklist: Vec::new(),
    });
    let applied = Rc::new(RefCell::new(config));
    let preview_on = Rc::new(std::cell::Cell::new(false));
    let style = Rc::new(move |provider: CssProvider| {
        if let Some(old) = active_css.borrow_mut().replace(provider.clone()) {
            gtk::StyleContext::remove_provider_for_screen(&screen, &old);
        }
        gtk::StyleContext::add_provider_for_screen(&screen, &provider, 800);
    });
    if let Ok(hud) = &overlay {
        let hud = Rc::clone(hud);
        let state = Rc::clone(&applied);
        let showing = Rc::clone(&preview_on);
        let read_draft = Rc::clone(&read_draft);
        let preview_style = Rc::clone(&style);
        let status = status.clone();
        preview.connect_clicked(move |button| {
            let enabled = !showing.get();
            let config = if enabled {
                read_draft()
            } else {
                state.borrow().clone()
            };
            match stylesheet(&config.css) {
                Ok(provider) => preview_style(provider),
                Err(error) if enabled => {
                    status.set_text(&format!("CSS 語法有誤：{error}"));
                    return;
                }
                Err(_) => preview_style(stylesheet("").expect("empty CSS")),
            }
            Overlay::set_preview(&hud, enabled, config);
            showing.set(enabled);
            button.set_label(if enabled {
                "關閉預覽"
            } else {
                "預覽面板"
            });
            status.set_text(if enabled {
                "目前顯示未儲存的預覽"
            } else {
                ""
            });
        });
        let hud = Rc::clone(overlay.as_ref().expect("overlay available"));
        let state = Rc::clone(&applied);
        let showing = Rc::clone(&preview_on);
        let style = Rc::clone(&style);
        let preview = preview.clone();
        window.connect_hide(move |_| {
            if showing.replace(false) {
                let config = state.borrow().clone();
                if let Ok(provider) = stylesheet(&config.css) {
                    style(provider);
                }
                Overlay::set_preview(&hud, false, config);
                preview.set_label("預覽面板");
            }
        });
    }

    save.connect_clicked(move |_| {
        let next = read_draft();
        if next.target.is_empty() {
            status.set_text("請輸入遊戲執行檔名稱。");
            return;
        }
        let provider = match stylesheet(&next.css) {
            Ok(provider) => provider,
            Err(error) => {
                status.set_text(&format!("CSS 語法有誤：{error}"));
                status.set_tooltip_text(Some(&error));
                return;
            }
        };
        if let Err(error) = next.save() {
            status.set_text(&format!("無法儲存設定：{error}"));
            status.set_tooltip_text(Some(&error.to_string()));
            return;
        }
        style(provider);
        if let Ok(hud) = &overlay {
            Overlay::set_preview(hud, false, next.clone());
        }
        keep_running.set(next.overlay && overlay.is_ok());
        *applied.borrow_mut() = next;
        preview_on.set(false);
        preview.set_label("預覽面板");
        let summary = if keep_running.get() {
            "已套用"
        } else {
            "已儲存 · 面板未啟用"
        };
        status.set_text(if let Err(error) = &overlay {
            error
        } else {
            summary
        });
        status.set_tooltip_text(None);
    });
    window.show_all();
}

fn add_page(stack: &gtk::Stack, page: &GtkBox, name: &str, title: &str) {
    let scroll = ScrolledWindow::new(gtk::Adjustment::NONE, gtk::Adjustment::NONE);
    scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    scroll.add(page);
    stack.add_titled(&scroll, name, title);
}

#[cfg(test)]
mod tests {
    use super::drop_index;

    #[test]
    fn inserts_on_the_selected_side_of_a_row() {
        assert_eq!(drop_index(0, 4, false, 6), Some(3));
        assert_eq!(drop_index(0, 5, true, 6), Some(5));
        assert_eq!(drop_index(5, 0, false, 6), Some(0));
        assert_eq!(drop_index(5, 0, true, 6), Some(1));
        assert_eq!(drop_index(1, 1, true, 6), Some(1));
        assert_eq!(drop_index(6, 0, false, 6), None);
    }
}
