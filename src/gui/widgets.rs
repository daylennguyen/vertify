use eframe::egui::{
    self, Color32, CornerRadius, CursorIcon, FontId, Pos2, Rect, RichText, Sense, Stroke, Vec2,
};

use super::{ACCENT, ACCENT_HOVER, INK, INK_MUTED, PANEL, PANEL_HOVER, STROKE_SOFT, SURFACE};

pub fn tip(resp: &egui::Response, text: &str) {
    resp.clone().on_hover_ui(|ui| {
        ui.set_max_width(260.0);
        ui.label(RichText::new(text).size(13.0).color(INK));
    });
}

/// Connected Blur | Color segmented control.
pub fn fill_segment(ui: &mut egui::Ui, blur_selected: bool) -> (egui::Response, egui::Response) {
    let height = 36.0;
    let width = 168.0;
    let (outer, _) = ui.allocate_exact_size(Vec2::new(width, height), Sense::hover());
    let painter = ui.painter();

    painter.rect_filled(outer, CornerRadius::same(18), SURFACE);
    painter.rect_stroke(
        outer,
        CornerRadius::same(18),
        Stroke::new(1.0, STROKE_SOFT),
        egui::StrokeKind::Outside,
    );

    let mid = outer.center().x;
    let left =
        Rect::from_min_max(outer.min, Pos2::new(mid, outer.max.y)).shrink2(Vec2::new(3.0, 3.0));
    let right =
        Rect::from_min_max(Pos2::new(mid, outer.min.y), outer.max).shrink2(Vec2::new(3.0, 3.0));

    let left_resp = ui.interact(left, ui.id().with("fill_blur"), Sense::click());
    let right_resp = ui.interact(right, ui.id().with("fill_color"), Sense::click());

    if left_resp.hovered() || right_resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    if blur_selected {
        painter.rect_filled(left, CornerRadius::same(15), ACCENT);
    } else if left_resp.hovered() {
        painter.rect_filled(left, CornerRadius::same(15), PANEL_HOVER);
    }
    if !blur_selected {
        painter.rect_filled(right, CornerRadius::same(15), ACCENT);
    } else if right_resp.hovered() {
        painter.rect_filled(right, CornerRadius::same(15), PANEL_HOVER);
    }

    painter.text(
        left.center(),
        egui::Align2::CENTER_CENTER,
        "Blur",
        FontId::proportional(14.0),
        if blur_selected { Color32::WHITE } else { INK },
    );
    painter.text(
        right.center(),
        egui::Align2::CENTER_CENTER,
        "Color",
        FontId::proportional(14.0),
        if !blur_selected { Color32::WHITE } else { INK },
    );

    tip(
        &left_resp,
        "Social-style fill\nBlurred copy of the video fills the empty bars",
    );
    tip(
        &right_resp,
        "Solid letterbox bars\nPick the bar color in Backstage (,)",
    );
    left_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Blur"));
    right_resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, "Color"));

    (left_resp, right_resp)
}

pub fn ghost_button(ui: &mut egui::Ui, label: &str, tooltip: &str) -> egui::Response {
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), FontId::proportional(14.0), INK);
    let size = Vec2::new((galley.size().x + 28.0).max(44.0), 36.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let fill = if hovered { PANEL_HOVER } else { PANEL };
    let stroke = if hovered {
        Stroke::new(1.2, ACCENT)
    } else {
        Stroke::new(1.0, STROKE_SOFT)
    };

    let painter = ui.painter();
    if hovered {
        painter.rect_filled(
            rect.translate(Vec2::new(0.0, 2.0)),
            CornerRadius::same(18),
            Color32::from_rgba_unmultiplied(18, 32, 38, 16),
        );
    }
    painter.rect_filled(rect, CornerRadius::same(18), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(18),
        stroke,
        egui::StrokeKind::Outside,
    );
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        INK,
    );

    tip(&resp, tooltip);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}

pub fn primary_button(ui: &mut egui::Ui, label: &str, tooltip: &str) -> egui::Response {
    let galley =
        ui.painter()
            .layout_no_wrap(label.to_owned(), FontId::proportional(14.0), Color32::WHITE);
    let size = Vec2::new((galley.size().x + 32.0).max(96.0), 36.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    let hovered = resp.hovered();
    if hovered {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let fill = if hovered { ACCENT_HOVER } else { ACCENT };
    let painter = ui.painter();
    painter.rect_filled(
        rect.translate(Vec2::new(0.0, 2.0)),
        CornerRadius::same(18),
        Color32::from_rgba_unmultiplied(ACCENT.r(), ACCENT.g(), ACCENT.b(), 50),
    );
    painter.rect_filled(rect, CornerRadius::same(18), fill);
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        Color32::WHITE,
    );

    tip(&resp, tooltip);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, label));
    resp
}

pub fn chip_button(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    tooltip: &str,
) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(13.0),
        if selected { Color32::WHITE } else { INK },
    );
    let size = Vec2::new((galley.size().x + 20.0).max(48.0), 30.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    let fill = if selected {
        INK
    } else if resp.hovered() {
        PANEL_HOVER
    } else {
        SURFACE
    };
    let stroke = if selected {
        Stroke::NONE
    } else {
        Stroke::new(1.0, STROKE_SOFT)
    };

    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(10), fill);
    painter.rect_stroke(
        rect,
        CornerRadius::same(10),
        stroke,
        egui::StrokeKind::Outside,
    );
    painter.galley(
        Pos2::new(
            rect.center().x - galley.size().x * 0.5,
            rect.center().y - galley.size().y * 0.5,
        ),
        galley,
        if selected { Color32::WHITE } else { INK },
    );

    tip(&resp, tooltip);
    resp
}

pub fn color_swatch(
    ui: &mut egui::Ui,
    hex_or_name: &str,
    rgb: [u8; 3],
    selected: bool,
) -> egui::Response {
    let size = Vec2::splat(28.0);
    let (rect, resp) = ui.allocate_exact_size(size, Sense::click());
    if resp.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    let painter = ui.painter();
    let fill = Color32::from_rgb(rgb[0], rgb[1], rgb[2]);
    painter.circle_filled(rect.center(), 11.0, fill);
    painter.circle_stroke(
        rect.center(),
        11.0,
        Stroke::new(
            if selected { 2.5 } else { 1.0 },
            if selected { ACCENT } else { STROKE_SOFT },
        ),
    );
    if rgb[0] > 220 && rgb[1] > 220 && rgb[2] > 210 {
        painter.circle_stroke(
            rect.center(),
            11.0,
            Stroke::new(1.0, Color32::from_rgb(180, 176, 168)),
        );
    }
    tip(&resp, &format!("Bar color: {hex_or_name}"));
    resp
}

pub fn status_chip(ui: &mut egui::Ui, text: &str, danger: bool) -> egui::Response {
    let color = if danger {
        Color32::from_rgb(176, 48, 48)
    } else {
        INK_MUTED
    };
    let galley = ui
        .painter()
        .layout_no_wrap(text.to_owned(), FontId::proportional(14.0), color);
    let pad = Vec2::new(16.0, 8.0);
    let size = galley.size() + pad * 2.0;
    let (rect, resp) = ui.allocate_exact_size(size, Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, CornerRadius::same(14), PANEL);
    painter.rect_stroke(
        rect,
        CornerRadius::same(14),
        Stroke::new(
            1.0,
            if danger {
                Color32::from_rgb(220, 170, 170)
            } else {
                STROKE_SOFT
            },
        ),
        egui::StrokeKind::Outside,
    );
    painter.galley(rect.min + pad, galley, color);
    resp
}
