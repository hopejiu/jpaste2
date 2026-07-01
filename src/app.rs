//! Application — clipboard monitoring, system tray, global hotkey, egui UI.

use std::path::PathBuf;

use anyhow::Result;
use crossbeam_channel::Receiver;

use crate::clipboard::capture::CapturedData;
use crate::clipboard::service::ClipboardService;
use crate::clipboard::watcher::{start_watcher, ClipboardEvent, WatcherContext};
use crate::ops;
use crate::settings::SettingsService;
use crate::storage::db;
use crate::storage::image_store::ImageStore;
use crate::storage::repository::Repository;
use crate::ui::entry_list::{entry_list, EntryListState};
use crate::ui::filo_bar::{filo_bar, FiloBarState};
use crate::ui::search_bar::{search_bar, SearchBarState};
use crate::ui::settings_page::{settings_page, SettingsPageState};
use crate::ui::shortcut_help::shortcut_help_window;
use crate::ui::tab_bar::tab_bar;
use crate::ui::theme::apply_theme;
use crate::ui::toast::{start_toast_thread, ToastHandle};

pub fn data_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("jPastev2")
}

/// Application state. Some fields are kept alive for their side effects
/// (e.g., toast thread, hotkey registration) even if never read directly.
#[allow(dead_code)]
pub struct App {
    settings: SettingsService,
    repo: Repository,
    image_store: ImageStore,
    clipboard_svc: ClipboardService,
    toast: ToastHandle,
    entries: Vec<crate::storage::repository::Entry>,
    has_more: bool,
    pub selected_index: usize,
    tag_mask: i32,
    search_state: SearchBarState,
    settings_state: SettingsPageState,
    shortcut_help_open: bool,
    pub pinned: bool,
    pub window_visible: bool,
    clear_all_requested: bool,
    clipboard_rx: Receiver<ClipboardEvent>,
    _watcher_ctx: WatcherContext,
    hotkey_manager: global_hotkey::GlobalHotKeyManager,
    hotkey: global_hotkey::hotkey::HotKey,
}

impl App {
    pub fn new(data_dir: &PathBuf) -> Result<Self> {
        let log_path = data_dir.join("jpaste.log");
        if let Ok(file) = std::fs::File::create(&log_path) {
            env_logger::Builder::from_env(
                env_logger::Env::default().default_filter_or("info"),
            )
            .target(env_logger::Target::Pipe(Box::new(file)))
            .init();
        } else {
            env_logger::init();
        }
        log::info!("jPaste v2 starting, data dir: {:?}", data_dir);

        let settings = SettingsService::load(data_dir)?;
        let db_path = data_dir.join("clipboard.db");
        let db_conn = db::init_db(&db_path)?;
        let repo = Repository::new(db_conn);
        let image_store = ImageStore::new(data_dir);
        let toast = start_toast_thread();

        let (clipboard_tx, clipboard_rx) = crossbeam_channel::unbounded();
        let watcher_ctx = start_watcher(clipboard_tx.clone());

        let filo_stack = crate::filostack::service::FiloStackService::new()
            .with_write_text(Box::new(move |text: &str| {
                if let Ok(clip) = clipboard_rs::ClipboardContext::new() {
                    use clipboard_rs::Clipboard;
                    let _ = clip.set_text(text.to_string());
                }
            }))
            .with_notify(Box::new(move |_title, msg| {
                log::info!("filo: {}", msg);
            }));

        let sort_f = &settings.config().sort_field;
        let sort_o = &settings.config().sort_order;
        let entries = repo.get_history(0, "", 0, None, sort_f, sort_o, 21)?;
        let has_more = entries.len() > 20;
        let entries = if has_more { entries[..20].to_vec() } else { entries };

        let mut search_state = SearchBarState::default();
        search_state.sort_field = sort_f.clone();
        search_state.sort_order = sort_o.clone();
        search_state.total_count = repo.get_stats().map(|s| s.0).unwrap_or(0);
        search_state.filtered_count = entries.len() as i64;

        let settings_state = SettingsPageState::from_config(settings.config());

        if let Ok(paths) = repo.cleanup(settings.config().retain_days) {
            for p in &paths { let _ = image_store.delete(p); }
            log::info!("cleanup: {}", paths.len());
        }

        let hotkey_manager = global_hotkey::GlobalHotKeyManager::new()?;
        let hk_str = settings.config().hotkey.clone();
        let hotkey = parse_hotkey(&hk_str);
        if let Err(e) = hotkey_manager.register(hotkey) {
            log::warn!("hotkey '{}': {}", hk_str, e);
        }

        Ok(Self {
            settings, repo, image_store, toast,
            clipboard_svc: ClipboardService::new(filo_stack),
            entries, has_more, selected_index: 0, tag_mask: 0,
            search_state, settings_state, shortcut_help_open: false,
            pinned: false, window_visible: true,
            clear_all_requested: false,
            clipboard_rx, _watcher_ctx: watcher_ctx,
            hotkey_manager, hotkey,
        })
    }

    // ── Run event loop ────────────────────────────────────────

    pub fn run(mut self) -> Result<()> {
        use winit::event_loop::ControlFlow;

        let event_loop = winit::event_loop::EventLoop::new()?;

        // Tray
        let tray_menu = tray_icon::menu::Menu::new();
        let show_item = tray_icon::menu::MenuItem::new("显示", true, None);
        let settings_item = tray_icon::menu::MenuItem::new("设置", true, None);
        let quit_item = tray_icon::menu::MenuItem::new("退出", true, None);
        tray_menu.append(&show_item).ok();
        tray_menu.append(&settings_item).ok();
        tray_menu.append(&quit_item).ok();

        let _tray = tray_icon::TrayIconBuilder::new()
            .with_menu(Box::new(tray_menu))
            .with_tooltip("jPaste")
            .build()?;

        // Window
        let window_attrs = winit::window::WindowAttributes::default()
            .with_title("jPaste")
            .with_inner_size(winit::dpi::LogicalSize::new(480.0, 560.0));
        #[allow(deprecated)]
        let window = std::sync::Arc::new(event_loop.create_window(window_attrs)?);

        // ── egui-winit State ──
        let egui_ctx = egui::Context::default();
        let mut egui_winit_state = egui_winit::State::new(
            egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );

        // ── wgpu initialization ──
        let wgpu_config = egui_wgpu::WgpuConfiguration {
            surface: egui_wgpu::SurfaceConfig::LOW_LATENCY,
            ..Default::default()
        };
        let mut painter = pollster::block_on(egui_wgpu::winit::Painter::new(
            egui_ctx.clone(),
            wgpu_config,
            false,
            egui_wgpu::RendererOptions::default(),
        ));
        pollster::block_on(painter.set_window(
            egui::ViewportId::ROOT,
            Some(window.clone()),
        ))?;

        let window_arc = window;

        #[allow(deprecated)]
        event_loop.run(move |event, target| {
            target.set_control_flow(ControlFlow::Poll);

            match event {
                winit::event::Event::WindowEvent { event, .. } => {
                    let egui_response = egui_winit_state.on_window_event(&window_arc, &event);

                    match event {
                        winit::event::WindowEvent::CloseRequested => {
                            self.window_visible = false;
                            window_arc.set_visible(false);
                        }
                        winit::event::WindowEvent::Focused(false) => {
                            if !self.pinned {
                                self.window_visible = false;
                                window_arc.set_visible(false);
                            }
                        }
                        winit::event::WindowEvent::Focused(true) => {
                            self.window_visible = true;
                        }
                        winit::event::WindowEvent::Resized(phys) => {
                            if phys.width > 0 && phys.height > 0 {
                                painter.on_window_resized(
                                    egui::ViewportId::ROOT,
                                    std::num::NonZeroU32::new(phys.width).unwrap(),
                                    std::num::NonZeroU32::new(phys.height).unwrap(),
                                );
                            }
                        }
                        winit::event::WindowEvent::RedrawRequested => {
                            self.process_clipboard_events();
                            self.poll_global_events();

                            // Take egui input and run UI
                            let input = egui_winit_state.take_egui_input(&window_arc);
                            let egui::FullOutput {
                                platform_output,
                                textures_delta,
                                shapes,
                                pixels_per_point,
                                ..
                            } = egui_ctx.run_ui(input, |ctx| {
                                self.ui_update(ctx);
                            });

                            // Handle egui output
                            egui_winit_state.handle_platform_output(
                                &window_arc,
                                platform_output,
                            );

                            // Tessellate and paint with wgpu
                            let clipped_primitives =
                                egui_ctx.tessellate(shapes, pixels_per_point);
                            let clear_color = [0.11, 0.105, 0.13, 1.0];
                            let _vsync = painter.paint_and_update_textures(
                                egui::ViewportId::ROOT,
                                pixels_per_point,
                                clear_color,
                                &clipped_primitives,
                                &textures_delta,
                                vec![],
                                &window_arc,
                            );
                        }
                        _ => {}
                    }

                    if egui_response.repaint {
                        window_arc.request_redraw();
                    }
                }
                winit::event::Event::AboutToWait => {
                    window_arc.request_redraw();
                }
                _ => {}
            }
        })?;

        Ok(())
    }

    fn poll_global_events(&mut self) {
        if let Ok(te) = tray_icon::TrayIconEvent::receiver().try_recv() {
            match te {
                tray_icon::TrayIconEvent::Click { .. } => self.toggle_window_visibility(),
                _ => {}
            }
        }

        if let Ok(me) = tray_icon::menu::MenuEvent::receiver().try_recv() {
            let id_s = me.id.0.to_string();
            match id_s.as_str() {
                "1" => { self.window_visible = true; }
                "2" => { self.settings_state.open = true; self.window_visible = true; }
                "3" => {
                    self.save_settings();
                    log::info!("quit via tray");
                    std::process::exit(0);
                }
                _ => {}
            }
        }

        if let Ok(hke) = global_hotkey::GlobalHotKeyEvent::receiver().try_recv() {
            if hke.state == global_hotkey::HotKeyState::Pressed {
                self.toggle_window_visibility();
            }
        }
    }

    fn toggle_window_visibility(&mut self) {
        self.window_visible = !self.window_visible;
    }

    // ── Clipboard ────────────────────────────────────────────

    pub fn process_clipboard_events(&mut self) {
        while let Ok(event) = self.clipboard_rx.try_recv() {
            match event {
                ClipboardEvent::Captured(data) => self.handle_captured(data),
            }
        }
    }

    fn handle_captured(&mut self, data: CapturedData) {
        let handled = self.clipboard_svc
            .handle(&data, &self.repo, &self.image_store)
            .unwrap_or(None)
            .is_some();
        if handled {
            self.refresh_entries();
        }
    }

    pub fn refresh_entries(&mut self) {
        let q = if self.search_state.query.is_empty() { None }
                else { Some(self.search_state.query.as_str()) };
        self.entries = self.repo.get_history(
            self.tag_mask, "", 0, q,
            &self.search_state.sort_field, &self.search_state.sort_order, 21,
        ).unwrap_or_default();
        self.has_more = self.entries.len() > 20;
        if self.has_more { self.entries.truncate(20); }
        self.search_state.filtered_count = self.entries.len() as i64;
        self.selected_index = 0;
    }

    pub fn load_more(&mut self) {
        if !self.has_more { return; }
        let last = match self.entries.last() { Some(e) => e, None => return };
        let q = if self.search_state.query.is_empty() { None }
                else { Some(self.search_state.query.as_str()) };
        let more = self.repo.get_history(
            self.tag_mask, &last.updated_at, last.id, q,
            &self.search_state.sort_field, &self.search_state.sort_order, 21,
        ).unwrap_or_default();
        self.has_more = more.len() > 20;
        let mut new: Vec<_> = if self.has_more { more[..20].to_vec() } else { more };
        self.entries.append(&mut new);
        self.search_state.filtered_count = self.entries.len() as i64;
    }

    pub fn copy_entry(&mut self, index: usize) {
        if index >= self.entries.len() { return; }
        if ops::copy_entry(&self.repo, self.clipboard_svc.tracker_mut(), self.entries[index].id)
            .unwrap_or(None).is_some()
        {
            self.window_visible = false;
        }
    }

    pub fn delete_entry(&mut self, index: usize) {
        let _ = ops::delete_entry(
            &self.repo, &mut self.entries, index, &mut self.selected_index,
        );
    }

    pub fn toggle_favorite(&mut self, index: usize) {
        if index >= self.entries.len() { return; }
        let _ = ops::toggle_favorite(&self.repo, &mut self.entries[index]);
    }

    pub fn open_in_editor(&self, index: usize) {
        if index >= self.entries.len() { return; }
        let _ = ops::open_in_editor(&self.repo, self.entries[index].id);
    }

    pub fn clear_all_data(&mut self) {
        if let Ok(paths) = self.repo.delete_all(true) {
            for p in &paths { let _ = self.image_store.delete(p); }
        }
        self.entries.clear();
        self.has_more = false;
        self.selected_index = 0;
        self.search_state.filtered_count = 0;
        self.search_state.total_count = 0;
    }

    fn save_settings(&mut self) {
        self.settings_state.apply_to(self.settings.config_mut());
        let _ = self.settings.flush();
    }

    // ── UI update ─────────────────────────────────────────────

    fn ui_update(&mut self, ctx: &egui::Context) {
        apply_theme(ctx);
        use egui::Id;


        self.process_clipboard_events();
        let screen = ctx.input(|i| i.raw.screen_rect).unwrap_or(
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(480.0, 560.0)),
        );

        // Tab bar
        egui::Area::new(Id::new("tabs"))
            .fixed_pos(egui::pos2(0.0, 0.0))
            .show(ctx, |ui| {
                let w = screen.width();
                ui.allocate_ui(egui::vec2(w, 32.0), |ui| {
                    tab_bar(ui, &mut self.tag_mask);
                });
                ui.painter().line_segment(
                    [egui::pos2(0.0, 32.0), egui::pos2(w, 32.0)],
                    egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 48, 58)),
                );
            });

        // Bottom status bar
        egui::Area::new(Id::new("filo"))
            .fixed_pos(egui::pos2(0.0, screen.bottom() - 28.0))
            .show(ctx, |ui| {
                let mut fb = FiloBarState::new(self.clipboard_svc.filo_stack().mode());
                fb.queue_items = self.clipboard_svc.filo_stack().queue_items();
                filo_bar(ui, &mut fb, &mut |nm| {
                    self.clipboard_svc.filo_stack_mut().set_mode(nm);
                    self.settings.config_mut().paste_order = nm.to_string();
                    let _ = self.settings.flush();
                });
            });

        // Central area
        egui::Area::new(Id::new("main"))
            .fixed_pos(egui::pos2(0.0, 34.0))
            .show(ctx, |ui| {
                let avail = egui::vec2(screen.width(), screen.height() - 62.0);
                ui.allocate_ui(avail, |ui| {
                    let changed = search_bar(ui, &mut self.search_state);
                    if changed { self.refresh_entries(); }
                    ui.add_space(4.0);

                    let entries_clone = self.entries.clone();
                    let mut ls = EntryListState {
                        selected_index: self.selected_index,
                        loading: false,
                    };
                    let has_more = self.has_more;
                    entry_list(ui, &entries_clone, &mut ls, has_more, &mut || {
                        self.load_more();
                    });
                    self.selected_index = ls.selected_index;

                    // Shortcuts
                    if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                        if self.search_state.query.is_empty() {
                            self.window_visible = false;
                        } else {
                            self.search_state.query.clear();
                            self.refresh_entries();
                        }
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Enter))
                        && self.selected_index < self.entries.len() {
                        self.copy_entry(self.selected_index);
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Delete))
                        && self.selected_index < self.entries.len() {
                        self.delete_entry(self.selected_index);
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::Slash)) {
                        self.shortcut_help_open = true;
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                        self.selected_index = (self.selected_index + 1)
                            .min(self.entries.len().saturating_sub(1));
                    }
                    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                        self.selected_index = self.selected_index.saturating_sub(1);
                    }
                });
            });

        // Settings window (use flag pattern to avoid borrow conflicts)
        let stats = self.repo.get_stats().unwrap_or((0, 0));
        self.settings_state.stats = stats;
        let was_open = self.settings_state.open;
        settings_page(ctx, &mut self.settings_state, &mut || {
            // Flag is set — handled below
        });
        if self.settings_state.clear_all_clicked {
            self.clear_all_data();
            self.settings_state.clear_all_clicked = false;
        }
        if was_open && !self.settings_state.open {
            self.save_settings();
        }

        // Shortcut help
        if self.shortcut_help_open {
            shortcut_help_window(ctx, &mut self.shortcut_help_open);
        }
    }
}

// ── Hotkey parsing ─────────────────────────────────────────────

fn parse_hotkey(s: &str) -> global_hotkey::hotkey::HotKey {
    use global_hotkey::hotkey::{Code, HotKey, Modifiers};
    let mut mods = Modifiers::empty();
    let mut code = Code::KeyV;

    for part in s.split('+') {
        match part.to_lowercase().as_str() {
            "alt" => mods.insert(Modifiers::ALT),
            "ctrl" | "control" => mods.insert(Modifiers::CONTROL),
            "shift" => mods.insert(Modifiers::SHIFT),
            "win" | "cmd" | "super" | "meta" => mods.insert(Modifiers::SUPER),
            "v" => code = Code::KeyV, "c" => code = Code::KeyC,
            "z" => code = Code::KeyZ, "a" => code = Code::KeyA,
            "x" => code = Code::KeyX, "l" => code = Code::KeyL,
            "e" => code = Code::KeyE, "d" => code = Code::KeyD,
            "s" => code = Code::KeyS, "q" => code = Code::KeyQ,
            "space" => code = Code::Space, "delete" => code = Code::Delete,
            "enter" => code = Code::Enter, "escape" | "esc" => code = Code::Escape,
            "tab" => code = Code::Tab, "home" => code = Code::Home,
            "end" => code = Code::End, "pageup" => code = Code::PageUp,
            "pagedown" => code = Code::PageDown, "up" => code = Code::ArrowUp,
            "down" => code = Code::ArrowDown, "left" => code = Code::ArrowLeft,
            "right" => code = Code::ArrowRight,
            f if f.starts_with('f') && f.len() <= 3 => {
                if let Ok(n) = f[1..].parse::<u8>() {
                    code = match n {
                        1 => Code::F1, 2 => Code::F2, 3 => Code::F3,
                        4 => Code::F4, 5 => Code::F5, 6 => Code::F6,
                        7 => Code::F7, 8 => Code::F8, 9 => Code::F9,
                        10 => Code::F10, 11 => Code::F11, 12 => Code::F12,
                        _ => continue,
                    };
                }
            }
            _ => {}
        }
    }

    HotKey::new(Some(mods), code)
}

// ── Auto-start ─────────────────────────────────────────────────

#[allow(unused)]
pub fn set_autostart(_enabled: bool) -> Result<()> {
    // Registry auto-start is disabled in this build for API compat.
    // Use windows crate RegOpenKeyExW/RegSetValueExW.
    log::info!("auto-start: {}", if _enabled { "enable" } else { "disable" });
    Ok(())
}
