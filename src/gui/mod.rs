//! Shared Flip Stage UI — used by `vertify-gui` and visual snapshot tests.

mod widgets;

use crate::{
    build_plan, convert, ensure_tools, extract_preview_png, parse_fill_color, probe,
    ConvertOptions, Fill, Orientation, ProbeInfo, Target,
};
use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, FontData, FontDefinitions, FontFamily, FontId, Frame,
    Pos2, Rect, RichText, Sense, Ui, Vec2, WidgetInfo, WidgetType,
};
use image::imageops::{self, FilterType};
use image::{DynamicImage, RgbaImage};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;
use std::time::{Duration, Instant};
use widgets::{
    chip_button, color_swatch, fill_segment, ghost_button, primary_button, px_stroke, status_chip,
    tip,
};

// --- palette: cool mist stage, ink, citrus accent (restrained product) ---
pub const BG_TOP: Color32 = Color32::from_rgb(198, 214, 220);
pub const BG_MID: Color32 = Color32::from_rgb(220, 230, 234);
pub const BG_BOT: Color32 = Color32::from_rgb(232, 238, 240);
pub const INK: Color32 = Color32::from_rgb(16, 28, 34);
pub const INK_MUTED: Color32 = Color32::from_rgb(68, 86, 94);
pub const DESK: Color32 = Color32::from_rgb(170, 188, 196);
pub const FRAME: Color32 = Color32::from_rgb(18, 28, 34);
pub const ACCENT: Color32 = Color32::from_rgb(214, 122, 32);
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(232, 142, 48);
pub const PANEL: Color32 = Color32::from_rgb(250, 252, 253);
pub const PANEL_HOVER: Color32 = Color32::from_rgb(255, 246, 232);
pub const SURFACE: Color32 = Color32::from_rgb(238, 244, 246);
pub const STROKE_SOFT: Color32 = Color32::from_rgb(186, 200, 206);
pub const DANGER: Color32 = Color32::from_rgb(176, 48, 48);
pub const OK: Color32 = Color32::from_rgb(36, 120, 78);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Idle,
    Loading,
    Ready,
    Encoding,
    Done,
    Error,
}

struct PreviewTextures {
    fg: egui::TextureHandle,
    bg_blur: egui::TextureHandle,
    source_w: u32,
    source_h: u32,
}

enum WorkerMsg {
    Loaded {
        path: PathBuf,
        probe: ProbeInfo,
        frame: RgbaImage,
    },
    Encoded {
        output: PathBuf,
    },
    Failed(String),
}

/// Flip Stage application state.
pub struct VertifyApp {
    pub phase: Phase,
    pub input: Option<PathBuf>,
    pub probe: Option<ProbeInfo>,
    preview: Option<PreviewTextures>,
    pub to: Target,
    pub fill: Fill,
    pub size: u32,
    pub color: String,
    pub blur: u32,
    pub crf: u32,
    pub fast: bool,
    pub overwrite: bool,
    pub dry_run: bool,
    pub backstage_open: bool,
    pub error: Option<String>,
    pub output: Option<PathBuf>,
    rx: Option<Receiver<WorkerMsg>>,
    started: Instant,
    /// When set, drives animations instead of wall clock (snapshot determinism).
    pub frozen_time: Option<f32>,
    /// Skip dialogs / threads — for harness tests.
    pub headless: bool,
    tools_ok: Result<(), String>,
    hover_phone: bool,
    drag_hover: bool,
}

impl VertifyApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        Self::with_tools(ensure_tools().map_err(|e| e.to_string()))
    }

    /// Deterministic app for visual tests (no ffmpeg required for idle/backstage).
    pub fn for_snapshot(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        let mut app = Self::with_tools(Ok(()));
        app.headless = true;
        app.frozen_time = Some(0.35);
        app
    }

    fn with_tools(tools_ok: Result<(), String>) -> Self {
        let error = tools_ok.as_ref().err().cloned();
        Self {
            phase: if error.is_some() {
                Phase::Error
            } else {
                Phase::Idle
            },
            input: None,
            probe: None,
            preview: None,
            to: Target::Auto,
            fill: Fill::Blur,
            size: 1920,
            color: "black".into(),
            blur: 40,
            crf: 21,
            fast: false,
            overwrite: false,
            dry_run: false,
            backstage_open: false,
            error,
            output: None,
            rx: None,
            started: Instant::now(),
            frozen_time: None,
            headless: false,
            tools_ok,
            hover_phone: false,
            drag_hover: false,
        }
    }

    pub fn clock(&self, _ctx: &egui::Context) -> f32 {
        self.frozen_time
            .unwrap_or_else(|| self.started.elapsed().as_secs_f32())
    }

    /// Inject a synthetic landscape preview for snapshot / demo states.
    pub fn load_fixture_preview(&mut self, ctx: &egui::Context, width: u32, height: u32) {
        let frame = synthetic_frame(width, height);
        self.apply_loaded(
            ctx,
            PathBuf::from("fixture_landscape.mp4"),
            ProbeInfo {
                width,
                height,
                duration_secs: 12.0,
            },
            frame,
        );
    }

    pub fn set_phase_done(&mut self, output: PathBuf) {
        self.output = Some(output);
        self.phase = Phase::Done;
        self.error = None;
    }

    fn options(&self) -> Option<ConvertOptions> {
        Some(ConvertOptions {
            input: self.input.clone()?,
            output: None,
            to: self.to,
            fill: self.fill,
            size: self.size,
            color: self.color.clone(),
            blur: self.blur,
            fast: self.fast,
            crf: self.crf,
            overwrite: self.overwrite,
            dry_run: self.dry_run,
        })
    }

    pub fn resolved_target(&self) -> Option<Target> {
        let probe = self.probe.as_ref()?;
        build_plan(
            &ConvertOptions {
                input: self.input.clone().unwrap_or_default(),
                to: self.to,
                fill: self.fill,
                size: self.size,
                color: self.color.clone(),
                blur: self.blur,
                ..Default::default()
            },
            probe,
        )
        .ok()
        .map(|p| p.target)
    }

    pub fn whisper(&self) -> String {
        if let Some(err) = &self.error {
            return err.clone();
        }
        match self.phase {
            Phase::Idle => "Drop a video · flip without cropping".into(),
            Phase::Loading => "Reading clip…".into(),
            Phase::Ready => {
                let orient = self
                    .probe
                    .as_ref()
                    .map(|p| p.orientation())
                    .unwrap_or(Orientation::Landscape);
                let dest = match self.resolved_target() {
                    Some(Target::Vertical) => "Vertical 9:16",
                    Some(Target::Horizontal) => "Horizontal 16:9",
                    _ => "?",
                };
                let from = match orient {
                    Orientation::Landscape => "Landscape",
                    Orientation::Portrait => "Portrait",
                    Orientation::Square => "Square",
                };
                let fill = match self.fill {
                    Fill::Blur => "soft blur bars",
                    Fill::Color => "solid color bars",
                };
                format!("{from} -> {dest} · {fill}")
            }
            Phase::Encoding => {
                if self.dry_run {
                    "Dry run — printing ffmpeg command…".into()
                } else {
                    "Encoding the flipped frame…".into()
                }
            }
            Phase::Done => {
                if let Some(p) = &self.output {
                    format!("Exported · {}", file_name(p))
                } else {
                    "Exported".into()
                }
            }
            Phase::Error => self
                .error
                .clone()
                .unwrap_or_else(|| "Something went wrong".into()),
        }
    }

    fn whisper_detail(&self) -> Option<String> {
        match self.phase {
            Phase::Ready => {
                let (ow, oh) = self.resolved_target().map(|t| {
                    let long = self.size & !1;
                    let short = ((long * 9 + 8) / 16) & !1;
                    match t {
                        Target::Vertical => (short, long),
                        _ => (long, short),
                    }
                })?;
                let name = self.input.as_ref().map(|p| file_name(p)).unwrap_or_default();
                Some(format!(
                    "{name}\nOutput canvas {ow}×{oh}\nFill: {}\nClick the frame or press Enter to export.",
                    match self.fill {
                        Fill::Blur => format!("blur ({})", self.blur),
                        Fill::Color => format!("color ({})", self.color),
                    }
                ))
            }
            Phase::Done => self.output.as_ref().map(|p| {
                format!("{}\nClick the frame to re-export, or Reveal to open the file.", p.display())
            }),
            Phase::Idle => Some(
                "Drag a video onto the stage, or click the frame / press O to open.\nThe whole frame is kept — never cropped.".into(),
            ),
            Phase::Error => self.error.clone(),
            _ => None,
        }
    }

    fn pick_file(&mut self, ctx: &egui::Context) {
        if self.tools_ok.is_err() || self.rx.is_some() || self.headless {
            return;
        }
        if let Some(path) = rfd::FileDialog::new()
            .add_filter(
                "Video",
                &["mp4", "mov", "mkv", "webm", "m4v", "avi", "mpg", "mpeg"],
            )
            .pick_file()
        {
            self.begin_load(path, ctx);
        }
    }

    pub fn begin_load(&mut self, path: PathBuf, ctx: &egui::Context) {
        if self.headless {
            // Tests inject fixtures instead of probing real files.
            return;
        }
        self.phase = Phase::Loading;
        self.error = None;
        self.output = None;
        self.preview = None;
        self.input = Some(path.clone());
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        ctx.request_repaint();
        thread::spawn(move || {
            let result = (|| {
                let info = probe(&path)?;
                let at = if info.duration_secs > 2.0 {
                    info.duration_secs * 0.2
                } else {
                    0.0
                };
                let png = extract_preview_png(&path, at)?;
                let img = image::load_from_memory(&png)?.to_rgba8();
                Ok::<_, anyhow::Error>((info, img))
            })();
            match result {
                Ok((probe, frame)) => {
                    let _ = tx.send(WorkerMsg::Loaded { path, probe, frame });
                }
                Err(e) => {
                    let _ = tx.send(WorkerMsg::Failed(e.to_string()));
                }
            }
        });
    }

    fn begin_export(&mut self, ctx: &egui::Context) {
        if self.phase != Phase::Ready && self.phase != Phase::Done {
            return;
        }
        if self.probe.as_ref().is_some_and(|p| p.is_square()) && self.to == Target::Auto {
            self.error = Some("Square clip — choose 9:16 or 16:9 in Backstage".into());
            self.backstage_open = true;
            return;
        }
        let Some(mut opts) = self.options() else {
            return;
        };
        let Some(probe) = self.probe.as_ref() else {
            return;
        };
        let Ok(plan) = build_plan(&opts, probe) else {
            self.error = Some("Could not plan conversion — check target for square clips".into());
            self.backstage_open = true;
            return;
        };

        if !self.headless && plan.output.exists() && !opts.overwrite && !opts.dry_run {
            let replace = rfd::MessageDialog::new()
                .set_title("Replace existing file?")
                .set_description(format!(
                    "{} already exists. Replace it?",
                    plan.output.display()
                ))
                .set_buttons(rfd::MessageButtons::YesNo)
                .show();
            if replace != rfd::MessageDialogResult::Yes {
                return;
            }
            opts.overwrite = true;
        }

        if self.headless {
            self.set_phase_done(plan.output);
            return;
        }
        self.spawn_encode(opts, ctx);
    }

    fn spawn_encode(&mut self, opts: ConvertOptions, ctx: &egui::Context) {
        self.phase = Phase::Encoding;
        self.error = None;
        let (tx, rx) = mpsc::channel();
        self.rx = Some(rx);
        ctx.request_repaint();
        thread::spawn(move || match convert(&opts) {
            Ok(output) => {
                let _ = tx.send(WorkerMsg::Encoded { output });
            }
            Err(e) => {
                let _ = tx.send(WorkerMsg::Failed(e.to_string()));
            }
        });
    }

    fn poll_worker(&mut self, ctx: &egui::Context) {
        let Some(rx) = &self.rx else {
            return;
        };
        match rx.try_recv() {
            Ok(WorkerMsg::Loaded { path, probe, frame }) => {
                self.rx = None;
                self.apply_loaded(ctx, path, probe, frame);
            }
            Ok(WorkerMsg::Encoded { output }) => {
                self.rx = None;
                self.output = Some(output);
                self.phase = Phase::Done;
            }
            Ok(WorkerMsg::Failed(msg)) => {
                self.rx = None;
                self.error = Some(msg);
                self.phase = if self.preview.is_some() {
                    Phase::Ready
                } else {
                    Phase::Error
                };
            }
            Err(TryRecvError::Empty) => {
                ctx.request_repaint_after(Duration::from_millis(50));
            }
            Err(TryRecvError::Disconnected) => {
                self.rx = None;
            }
        }
    }

    fn apply_loaded(
        &mut self,
        ctx: &egui::Context,
        path: PathBuf,
        probe: ProbeInfo,
        frame: RgbaImage,
    ) {
        self.input = Some(path);
        if probe.is_square() && self.to == Target::Auto {
            self.backstage_open = true;
            self.error = Some("Square clip — pick 9:16 or 16:9".into());
        }
        let source_w = frame.width();
        let source_h = frame.height();
        let fg = ctx.load_texture(
            "vertify-fg",
            rgba_to_color_image(&frame),
            Default::default(),
        );
        let blurred = make_blur_bg(&frame, 28);
        let bg_blur = ctx.load_texture(
            "vertify-bg",
            rgba_to_color_image(&blurred),
            Default::default(),
        );
        self.preview = Some(PreviewTextures {
            fg,
            bg_blur,
            source_w,
            source_h,
        });
        self.probe = Some(probe);
        self.phase = Phase::Ready;
    }

    pub fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.input = None;
        self.probe = None;
        self.preview = None;
        self.output = None;
        self.error = None;
        self.to = Target::Auto;
        self.backstage_open = false;
    }

    /// Main UI — shared by eframe and visual harnesses.
    pub fn ui(&mut self, ctx: &egui::Context) {
        self.poll_worker(ctx);

        if !self.headless {
            let dropped: Vec<_> = ctx.input(|i| i.raw.dropped_files.clone());
            if let Some(f) = dropped.into_iter().next() {
                if let Some(path) = f.path {
                    self.begin_load(path, ctx);
                }
            }
        }

        // Shortcuts
        ctx.input(|i| {
            if i.key_pressed(egui::Key::O) && i.modifiers.command {
                // handled below with plain O for discoverability
            }
        });
        if ctx.input(|i| i.key_pressed(egui::Key::O))
            && matches!(
                self.phase,
                Phase::Idle | Phase::Error | Phase::Ready | Phase::Done
            )
        {
            self.pick_file(ctx);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter) || i.key_pressed(egui::Key::Space))
            && matches!(self.phase, Phase::Ready | Phase::Done)
        {
            self.begin_export(ctx);
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Comma)) {
            self.backstage_open = !self.backstage_open;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && self.backstage_open {
            self.backstage_open = false;
        }

        let t = self.clock(ctx);
        self.drag_hover = !self.headless
            && ctx.input(|i| !i.raw.hovered_files.is_empty())
            && matches!(
                self.phase,
                Phase::Idle | Phase::Error | Phase::Ready | Phase::Done
            );

        egui::CentralPanel::default()
            .frame(Frame::NONE.fill(BG_TOP))
            .show(ctx, |ui| {
                paint_atmosphere(ui, t, self.frozen_time.is_some());
                let full = ui.max_rect();

                // Top brand block
                ui.allocate_new_ui(
                    egui::UiBuilder::new().max_rect(Rect::from_min_size(
                        full.min + Vec2::new(0.0, 20.0),
                        Vec2::new(full.width(), 96.0),
                    )),
                    |ui| {
                        ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                            brand_header(ui);
                            ui.add_space(8.0);
                            whisper_line(ui, self);
                        });
                    },
                );

                // Bottom chrome first so we know remaining stage height
                let chrome_h = 72.0;
                let chrome = Rect::from_min_max(
                    Pos2::new(full.left() + 24.0, full.bottom() - chrome_h - 18.0),
                    Pos2::new(full.right() - 24.0, full.bottom() - 18.0),
                );
                draw_chrome_bar(ui, chrome, self, ctx);

                // Stage between header and chrome
                let stage_top = full.top() + 120.0;
                let stage_bottom = chrome.top() - 12.0;
                let stage_rect = Rect::from_min_max(
                    Pos2::new(full.center().x - 430.0, stage_top),
                    Pos2::new(full.center().x + 430.0, stage_bottom),
                )
                .intersect(Rect::from_min_max(
                    Pos2::new(full.left() + 40.0, stage_top),
                    Pos2::new(full.right() - 40.0, stage_bottom),
                ));

                let stage_resp = ui.interact(stage_rect, ui.id().with("stage"), Sense::click());
                if stage_resp.hovered()
                    && matches!(self.phase, Phase::Idle | Phase::Error)
                    && self.tools_ok.is_ok()
                {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                if stage_resp.clicked()
                    && matches!(self.phase, Phase::Idle | Phase::Error)
                    && self.tools_ok.is_ok()
                {
                    self.pick_file(ctx);
                }
                tip(
                    &stage_resp,
                    "Drop a video anywhere on the stage\nor click to choose a file (O)",
                );

                if self.drag_hover {
                    ui.painter().rect_filled(
                        stage_rect,
                        CornerRadius::same(24),
                        Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), 28),
                    );
                    ui.painter().rect_stroke(
                        stage_rect.shrink(2.0),
                        CornerRadius::same(22),
                        px_stroke(2.0, ACCENT),
                        egui::StrokeKind::Inside,
                    );
                }

                draw_desk(ui, stage_rect);
                let phone = phone_rect(stage_rect, self.resolved_target());
                let phone_resp = draw_phone(ui, phone, self, t);
                self.hover_phone = phone_resp.hovered();

                if phone_resp.hovered() && !matches!(self.phase, Phase::Loading | Phase::Encoding) {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
                phone_resp.clone().on_hover_ui(|ui| {
                    ui.set_max_width(220.0);
                    ui.label(RichText::new(phone_tooltip(self)).color(INK));
                });
                phone_resp.widget_info(|| {
                    WidgetInfo::labeled(WidgetType::Button, true, phone_a11y_label(self))
                });

                if phone_resp.clicked() {
                    match self.phase {
                        Phase::Idle | Phase::Error => self.pick_file(ctx),
                        Phase::Ready | Phase::Done => self.begin_export(ctx),
                        _ => {}
                    }
                }

                if self.backstage_open {
                    draw_backstage(ctx, self);
                }
            });
    }
}

impl eframe::App for VertifyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ui(ctx);
    }
}

pub fn run() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 740.0])
            .with_min_inner_size([740.0, 580.0])
            .with_title("Vertify"),
        ..Default::default()
    };
    eframe::run_native(
        "Vertify",
        options,
        Box::new(|cc| Ok(Box::new(VertifyApp::new(cc)))),
    )
}

fn brand_header(ui: &mut Ui) {
    let resp = ui.label(
        RichText::new("vertify")
            .family(FontFamily::Name("syne".into()))
            .size(48.0)
            .color(INK),
    );
    tip(
        &resp,
        "Vertify flips 16:9 <-> 9:16 without cropping.\nEmpty space gets a blurred copy or solid bars.",
    );
    ui.add_space(2.0);
    ui.label(
        RichText::new("Flip the frame. Keep every pixel.")
            .font(FontId::proportional(14.0))
            .color(INK_MUTED),
    );
}

fn whisper_line(ui: &mut Ui, app: &VertifyApp) {
    let resp = status_chip(ui, &app.whisper(), app.error.is_some());
    if let Some(detail) = app.whisper_detail() {
        resp.on_hover_ui(|ui| {
            ui.set_max_width(280.0);
            ui.label(RichText::new(detail).color(INK).size(13.0));
        });
    }
}

fn paint_atmosphere(ui: &mut Ui, t: f32, frozen: bool) {
    let rect = ui.max_rect();
    let painter = ui.painter();
    // Soft vertical gradient — cohesive stage, no harsh split
    painter.rect_filled(rect, 0.0, BG_TOP);
    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(rect.left(), rect.top() + rect.height() * 0.28),
            Pos2::new(rect.right(), rect.bottom()),
        ),
        0.0,
        Color32::from_rgba_unmultiplied(BG_MID.r(), BG_MID.g(), BG_MID.b(), 160),
    );
    painter.rect_filled(
        Rect::from_min_max(
            Pos2::new(rect.left(), rect.bottom() - rect.height() * 0.38),
            rect.max,
        ),
        0.0,
        Color32::from_rgba_unmultiplied(BG_BOT.r(), BG_BOT.g(), BG_BOT.b(), 200),
    );
    // Soft spotlight behind the phone
    let spot = Rect::from_center_size(
        Pos2::new(rect.center().x, rect.center().y - 20.0),
        Vec2::new(rect.width() * 0.55, rect.height() * 0.62),
    );
    painter.rect_filled(
        spot,
        CornerRadius::same(200),
        Color32::from_rgba_unmultiplied(255, 255, 255, 55),
    );

    let phase = if frozen { 0.35 } else { t };
    for i in 0..28 {
        let x = rect.left() + ((i * 97) % 900) as f32 * (rect.width() / 900.0);
        let y = rect.top() + ((i * 53) % 700) as f32 * (rect.height() / 700.0);
        let a = (28.0 + 14.0 * ((phase * 0.35 + i as f32).sin())) as u8;
        painter.circle_filled(
            Pos2::new(x, y),
            1.1,
            Color32::from_rgba_unmultiplied(255, 255, 255, a),
        );
    }
}

fn draw_desk(ui: &mut Ui, stage: Rect) {
    let desk = Rect::from_center_size(
        Pos2::new(stage.center().x, stage.bottom() - 28.0),
        Vec2::new((stage.width() * 0.36).clamp(140.0, 280.0), 12.0),
    );
    ui.painter().rect_filled(
        desk.translate(Vec2::new(0.0, 3.0)),
        CornerRadius::same(8),
        Color32::from_rgba_unmultiplied(40, 56, 64, 30),
    );
    ui.painter().rect_filled(desk, CornerRadius::same(8), DESK);
}

fn phone_rect(stage: Rect, target: Option<Target>) -> Rect {
    let vertical = !matches!(target, Some(Target::Horizontal));
    let max_h = stage.height() - 70.0;
    let max_w = stage.width() - 80.0;
    let (w, h) = if vertical {
        let h = max_h.min(520.0);
        let w = (h * 9.0 / 16.0).min(max_w);
        (w, w * 16.0 / 9.0)
    } else {
        let w = max_w.min(640.0);
        let h = (w * 9.0 / 16.0).min(max_h);
        (w, h)
    };
    Rect::from_center_size(
        Pos2::new(stage.center().x, stage.center().y - 10.0),
        Vec2::new(w, h),
    )
}

fn phone_tooltip(app: &VertifyApp) -> String {
    match app.phase {
        Phase::Idle | Phase::Error => "Open a video\nDrop a file here or press O".into(),
        Phase::Loading => "Reading the clip…".into(),
        Phase::Encoding => "Encoding in progress…".into(),
        Phase::Ready => {
            format!(
                "Export flipped video\nClick or press Enter\n{}",
                app.whisper()
            )
        }
        Phase::Done => "Re-export · click the frame again\nOr use Reveal below".into(),
    }
}

fn phone_a11y_label(app: &VertifyApp) -> &'static str {
    match app.phase {
        Phase::Idle | Phase::Error => "Open video",
        Phase::Loading => "Loading video",
        Phase::Encoding => "Encoding video",
        Phase::Ready => "Export flipped video",
        Phase::Done => "Re-export video",
    }
}

fn draw_phone(ui: &mut Ui, phone: Rect, app: &VertifyApp, t: f32) -> egui::Response {
    let hovered = ui.rect_contains_pointer(phone);
    let ready_glow = if matches!(app.phase, Phase::Ready) {
        0.55 + 0.45 * (t * 2.2).sin()
    } else {
        0.22
    };
    let glow = if hovered {
        (ready_glow + 0.35).min(1.0)
    } else {
        ready_glow
    };
    let expand = if hovered { 18.0 } else { 14.0 };
    let screen = phone.shrink(10.0);
    let phase = app.phase;

    {
        let painter = ui.painter();
        // Drop shadow
        painter.rect_filled(
            phone.translate(Vec2::new(0.0, 10.0)).expand(4.0),
            CornerRadius::same(26),
            Color32::from_rgba_unmultiplied(18, 32, 38, if hovered { 55 } else { 35 }),
        );
        painter.rect_filled(
            phone.expand(expand),
            CornerRadius::same(28),
            Color32::from_rgba_unmultiplied(
                ACCENT.r(),
                ACCENT.g(),
                ACCENT.b(),
                (34.0 * glow) as u8,
            ),
        );
        painter.rect_filled(phone, CornerRadius::same(22), FRAME);
        if hovered {
            painter.rect_stroke(
                phone,
                CornerRadius::same(22),
                px_stroke(1.5, ACCENT_HOVER),
                egui::StrokeKind::Outside,
            );
        }
        painter.rect_filled(screen, CornerRadius::same(14), Color32::from_rgb(8, 14, 18));
    }

    match phase {
        Phase::Idle | Phase::Error => {
            let painter = ui.painter();
            // Dashed-feel inset ring for empty state
            painter.rect_stroke(
                screen.shrink(18.0),
                CornerRadius::same(12),
                px_stroke(
                    1.5,
                    if app.drag_hover || hovered {
                        Color32::from_rgb(ACCENT.r(), ACCENT.g(), ACCENT.b())
                    } else {
                        Color32::from_rgb(70, 86, 94)
                    },
                ),
                egui::StrokeKind::Inside,
            );
            let title = if matches!(phase, Phase::Error) && app.error.is_some() {
                "Can't open video"
            } else if app.drag_hover {
                "Drop to open"
            } else if hovered {
                "Drop it here"
            } else {
                "Drop a video here"
            };
            painter.text(
                screen.center() - Vec2::new(0.0, 18.0),
                egui::Align2::CENTER_CENTER,
                title,
                FontId::proportional(18.0),
                Color32::from_rgb(220, 228, 232),
            );
            painter.text(
                screen.center() + Vec2::new(0.0, 10.0),
                egui::Align2::CENTER_CENTER,
                if app.drag_hover {
                    "release to load"
                } else if hovered {
                    "or click to browse"
                } else {
                    "click · or press O"
                },
                FontId::proportional(13.0),
                Color32::from_rgb(140, 154, 160),
            );
            painter.text(
                screen.center() + Vec2::new(0.0, 36.0),
                egui::Align2::CENTER_CENTER,
                "Never crops · keeps every pixel",
                FontId::proportional(11.0),
                Color32::from_rgb(100, 116, 124),
            );
        }
        Phase::Loading | Phase::Encoding => {
            let painter = ui.painter();
            let label = if phase == Phase::Loading {
                "Reading…"
            } else {
                "Encoding…"
            };
            let spin = (t * 3.0) % std::f32::consts::TAU;
            painter.circle_stroke(
                screen.center(),
                22.0,
                px_stroke(3.0, Color32::from_rgb(80, 96, 104)),
            );
            let a = screen.center() + Vec2::angled(spin) * 22.0;
            painter.circle_filled(a, 4.0, ACCENT);
            painter.text(
                screen.center() + Vec2::new(0.0, 48.0),
                egui::Align2::CENTER_CENTER,
                label,
                FontId::proportional(15.0),
                Color32::from_rgb(200, 210, 214),
            );
        }
        Phase::Ready | Phase::Done => {
            if let Some(tex) = &app.preview {
                paint_letterbox(ui, screen, tex, app);
            }
            let painter = ui.painter();
            // Output badge
            if let Some(target) = app.resolved_target() {
                let (ow, oh) = {
                    let long = app.size & !1;
                    let short = ((long * 9 + 8) / 16) & !1;
                    match target {
                        Target::Vertical => (short, long),
                        _ => (long, short),
                    }
                };
                let badge = format!("{ow}×{oh}");
                let badge_pos = Pos2::new(screen.left() + 14.0, screen.top() + 14.0);
                let galley = painter.layout_no_wrap(
                    badge,
                    FontId::proportional(11.0),
                    Color32::from_rgb(240, 244, 246),
                );
                let br = Rect::from_min_size(badge_pos, galley.size() + Vec2::new(12.0, 6.0));
                painter.rect_filled(
                    br,
                    CornerRadius::same(8),
                    Color32::from_rgba_unmultiplied(0, 0, 0, 140),
                );
                painter.galley(
                    badge_pos + Vec2::new(6.0, 3.0),
                    galley,
                    Color32::from_rgb(240, 244, 246),
                );
            }

            if phase == Phase::Done {
                painter.rect_filled(
                    screen,
                    CornerRadius::same(14),
                    Color32::from_rgba_unmultiplied(8, 40, 24, 150),
                );
                painter.text(
                    screen.center() - Vec2::new(0.0, 8.0),
                    egui::Align2::CENTER_CENTER,
                    "Exported",
                    FontId::proportional(18.0),
                    Color32::WHITE,
                );
                painter.text(
                    screen.center() + Vec2::new(0.0, 16.0),
                    egui::Align2::CENTER_CENTER,
                    if hovered {
                        "click to re-export"
                    } else {
                        "click again to re-export"
                    },
                    FontId::proportional(13.0),
                    Color32::from_rgb(200, 230, 210),
                );
            } else {
                let hint = if hovered {
                    "Click to export · Enter"
                } else {
                    "Click frame to export"
                };
                painter.rect_filled(
                    Rect::from_center_size(
                        Pos2::new(screen.center().x, screen.bottom() - 22.0),
                        Vec2::new(screen.width() * 0.72, 28.0),
                    ),
                    CornerRadius::same(10),
                    Color32::from_rgba_unmultiplied(0, 0, 0, if hovered { 160 } else { 110 }),
                );
                painter.text(
                    Pos2::new(screen.center().x, screen.bottom() - 22.0),
                    egui::Align2::CENTER_CENTER,
                    hint,
                    FontId::proportional(12.0),
                    Color32::from_rgba_unmultiplied(255, 255, 255, 220),
                );
            }
        }
    }

    {
        let painter = ui.painter();
        let notch = Rect::from_center_size(
            Pos2::new(phone.center().x, phone.top() + 16.0),
            Vec2::new(phone.width() * 0.28, 6.0),
        );
        painter.rect_filled(notch, CornerRadius::same(3), Color32::from_rgb(10, 16, 20));
    }

    ui.interact(phone, ui.id().with("phone"), Sense::click())
}

fn paint_letterbox(ui: &mut Ui, screen: Rect, tex: &PreviewTextures, app: &VertifyApp) {
    let clip = screen.intersect(ui.clip_rect());
    let painter = ui.painter().with_clip_rect(clip);
    let rgb = parse_fill_color(&app.color);
    let fill_color = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);

    match app.fill {
        Fill::Color => {
            painter.rect_filled(screen, CornerRadius::same(14), fill_color);
        }
        Fill::Blur => {
            let bg_aspect = tex.source_w as f32 / tex.source_h as f32;
            let screen_aspect = screen.width() / screen.height();
            let bg = if bg_aspect > screen_aspect {
                let h = screen.height();
                let w = h * bg_aspect;
                Rect::from_center_size(screen.center(), Vec2::new(w, h))
            } else {
                let w = screen.width();
                let h = w / bg_aspect;
                Rect::from_center_size(screen.center(), Vec2::new(w, h))
            };
            // Fill screen first so transparent edges aren't noisy
            painter.rect_filled(
                screen,
                CornerRadius::same(14),
                Color32::from_rgb(20, 24, 28),
            );
            painter.image(
                tex.bg_blur.id(),
                bg,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
    }

    let fg_aspect = tex.source_w as f32 / tex.source_h as f32;
    let screen_aspect = screen.width() / screen.height();
    let fg = if fg_aspect > screen_aspect {
        let w = screen.width();
        let h = w / fg_aspect;
        Rect::from_center_size(screen.center(), Vec2::new(w, h))
    } else {
        let h = screen.height();
        let w = h * fg_aspect;
        Rect::from_center_size(screen.center(), Vec2::new(w, h))
    };
    painter.image(
        tex.fg.id(),
        fg,
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        Color32::WHITE,
    );
}

fn draw_chrome_bar(ui: &mut Ui, chrome: Rect, app: &mut VertifyApp, ctx: &egui::Context) {
    let painter = ui.painter();
    painter.rect_filled(
        chrome.translate(Vec2::new(0.0, 4.0)),
        CornerRadius::same(22),
        Color32::from_rgba_unmultiplied(18, 32, 38, 22),
    );
    painter.rect_filled(chrome, CornerRadius::same(22), PANEL);
    painter.rect_stroke(
        chrome,
        CornerRadius::same(22),
        px_stroke(1.0, STROKE_SOFT),
        egui::StrokeKind::Outside,
    );

    ui.allocate_new_ui(
        egui::UiBuilder::new().max_rect(chrome.shrink2(Vec2::new(18.0, 0.0))),
        |ui| {
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                ui.set_min_height(chrome.height());
                ui.spacing_mut().item_spacing.x = 10.0;

                let blur_sel = app.fill == Fill::Blur;
                let (blur, color) = fill_segment(ui, blur_sel);
                if blur.clicked() {
                    app.fill = Fill::Blur;
                }
                if color.clicked() {
                    app.fill = Fill::Color;
                }

                ui.add_space(6.0);
                let div_x = ui.cursor().left();
                ui.painter().line_segment(
                    [
                        Pos2::new(div_x, chrome.center().y - 12.0),
                        Pos2::new(div_x, chrome.center().y + 12.0),
                    ],
                    px_stroke(1.0, STROKE_SOFT),
                );
                ui.add_space(12.0);

                if ghost_button(
                    ui,
                    "Open…",
                    "Choose a video file (O)\nMP4, MOV, MKV, WebM, and more",
                )
                .clicked()
                {
                    app.pick_file(ctx);
                }

                match app.phase {
                    Phase::Ready
                        if primary_button(
                            ui,
                            "Export",
                            "Encode the flipped video (Enter)\nUses current fill, size, and quality",
                        )
                        .clicked() =>
                    {
                        app.begin_export(ctx);
                    }
                    Phase::Done => {
                        if primary_button(ui, "Reveal", "Open the exported file").clicked() {
                            if let Some(p) = &app.output {
                                if !app.headless {
                                    let _ = open::that(p);
                                }
                            }
                        }
                        if ghost_button(
                            ui,
                            "Flip another",
                            "Clear the stage and start over with a new clip",
                        )
                        .clicked()
                        {
                            app.reset();
                        }
                    }
                    _ => {}
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let label = if app.backstage_open {
                        "Close"
                    } else {
                        "Settings"
                    };
                    if ghost_button(
                        ui,
                        label,
                        "Backstage settings (,)\nTarget aspect, size, blur/color, quality, dry-run",
                    )
                    .clicked()
                    {
                        app.backstage_open = !app.backstage_open;
                    }
                });
            });
        },
    );
}

fn draw_backstage(ctx: &egui::Context, app: &mut VertifyApp) {
    egui::Window::new("Backstage")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::RIGHT_BOTTOM, [-28.0, -100.0])
        .frame(
            Frame::window(&ctx.style())
                .fill(PANEL)
                .stroke(px_stroke(1.0, STROKE_SOFT))
                .corner_radius(CornerRadius::same(18))
                .inner_margin(18.0)
                .shadow(egui::Shadow {
                    offset: [0, 10],
                    blur: 28,
                    spread: 0,
                    color: Color32::from_rgba_unmultiplied(18, 32, 38, 45),
                }),
        )
        .show(ctx, |ui| {
            ui.set_width(308.0);
            ui.label(
                RichText::new("Encode & frame")
                    .family(FontFamily::Name("syne".into()))
                    .size(18.0)
                    .color(INK),
            );
            ui.label(
                RichText::new("Applies on the next export")
                    .size(12.0)
                    .color(INK_MUTED),
            );
            ui.add_space(12.0);

            section(ui, "Target");
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (label, val, tip_text) in [
                    (
                        "Auto",
                        Target::Auto,
                        "Flip orientation automatically\nLandscape -> 9:16, portrait -> 16:9",
                    ),
                    (
                        "9:16",
                        Target::Vertical,
                        "Force vertical output\nRequired for square sources",
                    ),
                    (
                        "16:9",
                        Target::Horizontal,
                        "Force horizontal output\nRequired for square sources",
                    ),
                ] {
                    if chip_button(ui, label, app.to == val, tip_text).clicked() {
                        app.to = val;
                        app.error = None;
                    }
                }
            });

            ui.add_space(12.0);
            section(ui, "Long edge");
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                for (s, tip_text) in [
                    (1080_u32, "720p-class when vertical"),
                    (1920, "1080p-class — default for social"),
                    (2560, "1440p-class"),
                    (3840, "4K-class — slower encode"),
                ] {
                    if chip_button(ui, &format!("{s}"), app.size == s, tip_text).clicked() {
                        app.size = s;
                    }
                }
            });

            ui.add_space(12.0);
            if app.fill == Fill::Blur {
                section(ui, "Blur strength");
                let r = ui.add(egui::Slider::new(&mut app.blur, 5..=80).suffix(" px"));
                tip(
                    &r,
                    "How soft the background copy is\nHigher = dreamier bars",
                );
            } else {
                section(ui, "Bar color");
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;
                    for (name, rgb) in [
                        ("black", [0_u8, 0, 0]),
                        ("white", [255, 255, 255]),
                        ("#101010", [16, 16, 16]),
                        ("#e8eef0", [232, 238, 240]),
                    ] {
                        if color_swatch(ui, name, rgb, app.color == name).clicked() {
                            app.color = name.into();
                        }
                    }
                });
                ui.add_space(6.0);
                let r = ui.text_edit_singleline(&mut app.color);
                tip(&r, "CSS name or hex, e.g. black, white, #101010");
            }

            ui.add_space(12.0);
            section(ui, "Quality");
            let r = ui.add(egui::Slider::new(&mut app.crf, 16..=28).text("CRF"));
            tip(
                &r,
                "x264 quality — lower is sharper / larger\n18-23 looks great; 21 is the default",
            );
            let r = ui.checkbox(&mut app.fast, "Fast encode");
            tip(
                &r,
                "ultrafast preset — quicker, larger file\nUse for drafts; turn off for finals",
            );
            let r = ui.checkbox(&mut app.overwrite, "Overwrite existing");
            tip(&r, "Replace the output file if it already exists (-y)");
            let r = ui.checkbox(&mut app.dry_run, "Dry run");
            tip(&r, "Print the ffmpeg command instead of encoding");

            ui.add_space(14.0);
            if ghost_button(ui, "Close backstage", "Hide these settings (Esc or ,)").clicked() {
                app.backstage_open = false;
            }
            if app.tools_ok.is_err() {
                ui.add_space(8.0);
                ui.colored_label(DANGER, "Install ffmpeg + ffprobe, then restart Vertify.");
            }
            let _ = OK;
        });
}

fn section(ui: &mut Ui, title: &str) {
    ui.label(RichText::new(title).strong().size(13.0).color(INK));
}

fn make_blur_bg(src: &RgbaImage, radius: u32) -> RgbaImage {
    let w = 480u32;
    let h = ((src.height() as f32 / src.width() as f32) * w as f32).max(1.0) as u32;
    let small = imageops::resize(src, w, h.max(1), FilterType::Triangle);
    let dynimg = DynamicImage::ImageRgba8(small);
    let mut img = dynimg.blur(radius as f32 * 0.35).to_rgba8();
    for px in img.pixels_mut() {
        px.0[0] = (px.0[0] as f32 * 0.85) as u8;
        px.0[1] = (px.0[1] as f32 * 0.85) as u8;
        px.0[2] = (px.0[2] as f32 * 0.85) as u8;
    }
    img
}

fn rgba_to_color_image(img: &RgbaImage) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied(
        [img.width() as usize, img.height() as usize],
        img.as_raw(),
    )
}

fn synthetic_frame(width: u32, height: u32) -> RgbaImage {
    let mut img = RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let u = x as f32 / width as f32;
            let v = y as f32 / height as f32;
            let r = (40.0 + 140.0 * u) as u8;
            let g = (90.0 + 80.0 * (1.0 - v)) as u8;
            let b = (120.0 + 90.0 * v) as u8;
            // Soft “subject” rectangle in the center
            let cx = (x as i32 - width as i32 / 2).unsigned_abs();
            let cy = (y as i32 - height as i32 / 2).unsigned_abs();
            let in_subject = cx < width / 5 && cy < height / 3;
            let px = if in_subject {
                image::Rgba([240, 220, 190, 255])
            } else {
                image::Rgba([r, g, b, 255])
            };
            img.put_pixel(x, y, px);
        }
    }
    img
}

fn file_name(path: &std::path::Path) -> String {
    path.file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string())
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();
    fonts.font_data.insert(
        "syne".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/Syne-Bold.ttf"
        ))),
    );
    fonts.font_data.insert(
        "syne_reg".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/Syne-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "source_sans".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/SourceSans3-Regular.ttf"
        ))),
    );
    fonts.font_data.insert(
        "source_sans_sb".into(),
        std::sync::Arc::new(FontData::from_static(include_bytes!(
            "../../assets/fonts/SourceSans3-SemiBold.ttf"
        ))),
    );

    fonts
        .families
        .entry(FontFamily::Name("syne".into()))
        .or_default()
        .insert(0, "syne".into());
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(0, "source_sans".into());
    fonts
        .families
        .entry(FontFamily::Proportional)
        .or_default()
        .insert(1, "source_sans_sb".into());

    ctx.set_fonts(fonts);
}
