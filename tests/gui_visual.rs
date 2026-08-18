//! Visual regression tests for the Flip Stage GUI.
//!
//! Update baselines after intentional UI changes:
//!
//! ```text
//! UPDATE_SNAPSHOTS=true cargo test --test gui_visual
//! ```
//!
//! PowerShell:
//!
//! ```text
//! $env:UPDATE_SNAPSHOTS="true"; cargo test --test gui_visual
//! ```

use egui::Vec2;
use egui_kittest::kittest::Queryable;
use egui_kittest::Harness;
use std::path::PathBuf;
use vertify::gui::{Phase, VertifyApp};
use vertify::{Fill, Target};

fn harness_for(
    setup: impl FnOnce(&mut VertifyApp, &egui::Context) + 'static,
) -> Harness<'static, VertifyApp> {
    let mut setup_cell = Some(setup);
    Harness::builder()
        .with_size(Vec2::new(900.0, 640.0))
        .build_eframe(move |cc| {
            let mut app = VertifyApp::for_snapshot(cc);
            if let Some(f) = setup_cell.take() {
                f(&mut app, &cc.egui_ctx);
            }
            app
        })
}

#[test]
fn snapshot_idle_stage() {
    let mut harness = harness_for(|_app, _ctx| {});
    harness.run();
    harness.snapshot("idle_stage");
}

#[test]
fn snapshot_ready_blur_preview() {
    let mut harness = harness_for(|app, ctx| {
        app.load_fixture_preview(ctx, 320, 180);
        app.fill = Fill::Blur;
        app.frozen_time = Some(0.4);
    });
    harness.run();
    harness.snapshot("ready_blur");
}

#[test]
fn snapshot_ready_color_preview() {
    let mut harness = harness_for(|app, ctx| {
        app.load_fixture_preview(ctx, 320, 180);
        app.fill = Fill::Color;
        app.color = "white".into();
        app.frozen_time = Some(0.4);
    });
    harness.run();
    harness.snapshot("ready_color_white");
}

#[test]
fn snapshot_backstage_open() {
    let mut harness = harness_for(|app, ctx| {
        app.load_fixture_preview(ctx, 320, 180);
        app.backstage_open = true;
        app.frozen_time = Some(0.4);
    });
    harness.run();
    harness.snapshot("backstage_open");
}

#[test]
fn snapshot_done_overlay() {
    let mut harness = harness_for(|app, ctx| {
        app.load_fixture_preview(ctx, 320, 180);
        app.set_phase_done(PathBuf::from("fixture_landscape_vertical.mp4"));
        app.frozen_time = Some(0.4);
    });
    harness.run();
    harness.snapshot("done_overlay");
}

#[test]
fn snapshot_horizontal_target() {
    let mut harness = harness_for(|app, ctx| {
        app.load_fixture_preview(ctx, 180, 320);
        app.to = Target::Horizontal;
        app.fill = Fill::Blur;
        app.frozen_time = Some(0.4);
    });
    harness.run();
    harness.snapshot("horizontal_target");
}

#[test]
fn accessible_controls_expose_labels() {
    let mut harness = harness_for(|_app, _ctx| {});
    harness.run();

    harness.get_by_label("Open video");
    harness.get_by_label("Blur");
    harness.get_by_label("Color");
    harness.get_by_label("Open…");
    harness.get_by_label("Settings");
}

#[test]
fn fill_toggle_updates_preview_snapshot() {
    let mut harness = harness_for(|app, ctx| {
        app.load_fixture_preview(ctx, 320, 180);
        app.fill = Fill::Blur;
        app.frozen_time = Some(0.4);
    });
    harness.run();

    harness.get_by_label("Color").click();
    harness.run();

    assert_eq!(harness.state().fill, Fill::Color);
    harness.snapshot("after_color_toggle");
}

#[test]
fn phases_match_fixture_setup() {
    let mut harness = harness_for(|app, ctx| {
        assert_eq!(app.phase, Phase::Idle);
        app.load_fixture_preview(ctx, 160, 90);
        assert_eq!(app.phase, Phase::Ready);
    });
    harness.run();
    assert_eq!(harness.state().phase, Phase::Ready);
}
