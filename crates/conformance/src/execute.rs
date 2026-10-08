use std::sync::mpsc::{self, Receiver, Sender};
use std::time::{Duration, Instant};

use browser::{Tab, tab::PageFrameStatus};
use bus::{CoreCommand, CoreEvent, HtmlParseFailure};
use core_types::{NetworkResponseInfo, ResourceKind};
use egui::{Color32, FullOutput, RawInput, Rect, Shape, pos2, vec2};

use crate::model::{CanvasColor, HarnessError};

const URL: &str = "https://borrowser.invalid/ag1/fixture.html";
const DEADLINE: Duration = Duration::from_secs(5);

fn viewport() -> Rect {
    Rect::from_min_size(pos2(0.0, 0.0), vec2(640.0, 480.0))
}

fn forward_commands(
    rx: &Receiver<CoreCommand>,
    parser: &Sender<CoreCommand>,
) -> Result<(), HarnessError> {
    for command in rx.try_iter() {
        match command {
            CoreCommand::ParseHtmlStart { .. }
            | CoreCommand::ParseHtmlChunk { .. }
            | CoreCommand::ParseHtmlDone { .. }
            | CoreCommand::CancelRequest { .. } => {
                parser.send(command).map_err(|_| {
                    HarnessError::new("transport.closed", "parser command receiver closed")
                })?;
            }
            CoreCommand::FetchStream { kind, .. } => {
                return Err(HarnessError::new(
                    "execution.subresource",
                    format!(
                        "unexpected {} request; fixtures must be self-contained",
                        kind.as_str()
                    ),
                ));
            }
            _ => {
                return Err(HarnessError::new(
                    "execution.command",
                    "unexpected stylesheet command",
                ));
            }
        }
    }
    Ok(())
}

fn apply(tab: &mut Tab, event: CoreEvent) -> Result<(), HarnessError> {
    tab.on_core_event(event)
        .map_err(|error| HarnessError::new("publication.failed", format!("{error:?}")))
}

fn parser_failure(error: HtmlParseFailure) -> HarnessError {
    let code = match error {
        HtmlParseFailure::Initialization(_) => "parser.initialization",
        HtmlParseFailure::Execution(_) => "parser.execution",
        HtmlParseFailure::Finalization(_) => "parser.finalization",
        HtmlParseFailure::DomHandleExhausted => "parser.handle-exhausted",
        HtmlParseFailure::PreSelectionBudgetExceeded => "parser.preselection-budget",
        HtmlParseFailure::DocumentModeUnavailable => "parser.mode-unavailable",
        HtmlParseFailure::DocumentModeChanged { .. } => "parser.mode-changed",
        HtmlParseFailure::InputClosed => "parser.input-closed",
    };
    HarnessError::new(code, "document parsing did not complete successfully")
}

pub(crate) fn execute_html(html: &[u8]) -> Result<CanvasColor, HarnessError> {
    let (commands, command_rx) = mpsc::channel();
    let (parser, parser_rx) = mpsc::channel();
    let (events, event_rx) = mpsc::channel();
    runtime_parse::start_parse_runtime(parser_rx, events);
    let mut tab = Tab::new(1);
    tab.set_bus_sender(commands);
    tab.navigate_to_new(URL.into());
    let request_id = tab.nav_gen;
    match command_rx.try_recv() {
        Ok(CoreCommand::FetchStream {
            tab_id: 1,
            request_id: id,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            url,
        }) if id == request_id && url == URL => {}
        _ => {
            return Err(HarnessError::new(
                "execution.navigation",
                "navigation did not request the fixture document",
            ));
        }
    }
    let response = NetworkResponseInfo {
        requested_url: URL.into(),
        final_url: URL.into(),
        status_code: Some(200),
        content_type: Some("text/html; charset=utf-8".into()),
    };
    apply(
        &mut tab,
        CoreEvent::NetworkStart {
            tab_id: 1,
            request_id,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response: response.clone(),
        },
    )?;
    forward_commands(&command_rx, &parser)?;
    // Fixed chunk boundaries are transport inputs, not parser policy.
    for bytes in html.chunks(128) {
        apply(
            &mut tab,
            CoreEvent::NetworkChunk {
                tab_id: 1,
                request_id,
                stylesheet_slot_id: None,
                kind: ResourceKind::Html,
                url: URL.into(),
                bytes: bytes.to_vec(),
            },
        )?;
        forward_commands(&command_rx, &parser)?;
    }
    apply(
        &mut tab,
        CoreEvent::NetworkDone {
            tab_id: 1,
            request_id,
            stylesheet_slot_id: None,
            kind: ResourceKind::Html,
            response,
            bytes_received: html.len(),
        },
    )?;
    forward_commands(&command_rx, &parser)?;
    let deadline = Instant::now() + DEADLINE;
    loop {
        let event = event_rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => HarnessError::new(
                    "execution.timeout",
                    "parser terminal event was not received",
                ),
                mpsc::RecvTimeoutError::Disconnected => HarnessError::new(
                    "transport.closed",
                    "parser event channel closed before completion",
                ),
            })?;
        let terminal = match &event {
            CoreEvent::DomPatchUpdate {
                tab_id: 1,
                request_id: id,
                ..
            } if *id == request_id => None,
            CoreEvent::HtmlParseFinished {
                tab_id: 1,
                request_id: id,
                result,
            } if *id == request_id => Some(result.clone()),
            _ => {
                return Err(HarnessError::new(
                    "execution.event",
                    "unexpected parser event or request identity",
                ));
            }
        };
        apply(&mut tab, event)?;
        forward_commands(&command_rx, &parser)?;
        if let Some(result) = terminal {
            result.map_err(parser_failure)?;
            break;
        }
    }
    if tab.nav_gen != request_id {
        return Err(HarnessError::new(
            "execution.navigation",
            "navigation changed before rendering",
        ));
    }
    let ctx = egui::Context::default();
    ctx.set_zoom_factor(1.0);
    let mut input = RawInput {
        screen_rect: Some(viewport()),
        time: Some(0.0),
        ..Default::default()
    };
    input
        .viewports
        .get_mut(&egui::ViewportId::ROOT)
        .expect("root viewport")
        .native_pixels_per_point = Some(1.0);
    ctx.begin_pass(input);
    let frame = tab.ui_content(&ctx);
    let output = ctx.end_pass();
    forward_commands(&command_rx, &parser)?;
    if tab.nav_gen != request_id {
        return Err(HarnessError::new(
            "execution.navigation",
            "navigation changed during rendering",
        ));
    }
    let status = frame
        .map_err(|_| HarnessError::new("render.style", "production style preparation failed"))?;
    accept_frame(status, &output)?;
    capture_canvas_color(&output)
}

fn accept_frame(status: PageFrameStatus, output: &FullOutput) -> Result<(), HarnessError> {
    match status {
        PageFrameStatus::NoDocument => {
            return Err(HarnessError::new(
                "render.no-document",
                "no page frame was produced",
            ));
        }
        PageFrameStatus::FollowupRequired => {
            return Err(HarnessError::new(
                "render.followup",
                "page frame requires follow-up work",
            ));
        }
        PageFrameStatus::Rendered => {}
    }
    if output.platform_output.requested_discard() {
        return Err(HarnessError::new(
            "render.discard",
            "egui discarded the current pass",
        ));
    }
    Ok(())
}

/// Certify an unobstructed opaque canvas sample. This does not rasterize shapes
/// or infer a DOM element's identity, and never receives the expected color.
pub(crate) fn capture_canvas_color(output: &FullOutput) -> Result<CanvasColor, HarnessError> {
    if output.pixels_per_point != 1.0 {
        return Err(HarnessError::new(
            "capture.scale",
            "expected one physical pixel per logical point",
        ));
    }
    // Includes the sample pixel and a conservative antialiasing guard.
    let guard = Rect::from_min_max(pos2(30.0, 30.0), pos2(35.0, 35.0));
    let mut color = None;
    let mut stack: Vec<_> = output
        .shapes
        .iter()
        .map(|item| (item.clip_rect, &item.shape))
        .collect();
    while let Some((clip, shape)) = stack.pop() {
        match shape {
            Shape::Noop => continue,
            Shape::Vec(children) => {
                stack.extend(children.iter().map(|shape| (clip, shape)));
                continue;
            }
            Shape::Callback(_) => {
                return Err(HarnessError::new(
                    "capture.callback",
                    "custom paint callbacks cannot be certified",
                ));
            }
            _ => {}
        }
        let bounds = shape.visual_bounding_rect();
        if bounds == Rect::NOTHING {
            continue;
        }
        if !bounds.is_finite() || !clip.is_finite() {
            return Err(HarnessError::new(
                "capture.geometry",
                "paint geometry must be finite",
            ));
        }
        if let Shape::Rect(rect) = shape {
            let eligible = rect.rect.contains_rect(viewport())
                && clip.contains_rect(viewport())
                && rect.fill.a() == 255
                && rect.fill != Color32::TRANSPARENT
                && rect.corner_radius == egui::CornerRadius::ZERO
                && rect.stroke.is_empty()
                && rect.blur_width == 0.0
                && rect.brush.is_none();
            if eligible {
                if color.is_some() {
                    return Err(HarnessError::new(
                        "capture.ambiguous",
                        "multiple viewport-covering opaque rectangles",
                    ));
                }
                let [r, g, b, _] = rect.fill.to_array();
                color = Some(CanvasColor([r, g, b]));
                continue;
            }
        }
        if bounds.intersect(clip).intersects(guard) {
            return Err(HarnessError::new(
                "capture.overlap",
                format!(
                    "additional paint intersects sample guard: bounds=({}, {})-({}, {})",
                    bounds.min.x, bounds.min.y, bounds.max.x, bounds.max.y
                ),
            ));
        }
    }
    color.ok_or_else(|| {
        HarnessError::new(
            "capture.missing",
            "no eligible opaque viewport canvas rectangle",
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::epaint::{ClippedShape, RectShape};

    fn output(shapes: Vec<Shape>) -> FullOutput {
        FullOutput {
            pixels_per_point: 1.0,
            shapes: shapes
                .into_iter()
                .map(|shape| ClippedShape {
                    clip_rect: viewport(),
                    shape,
                })
                .collect(),
            ..Default::default()
        }
    }

    fn canvas() -> Shape {
        Shape::rect_filled(viewport(), 0.0, Color32::from_rgb(18, 52, 86))
    }

    #[test]
    fn real_fixture_gate_and_authored_css_sensitivity() {
        let root = include_bytes!("../fixtures/root-canvas-color.html");
        assert_eq!(execute_html(root).unwrap(), CanvasColor([18, 52, 86]));
        assert_eq!(
            execute_html(include_bytes!("../fixtures/cascade-canvas-color.html")).unwrap(),
            CanvasColor([52, 86, 120])
        );
        let changed = std::str::from_utf8(root)
            .unwrap()
            .replace("#123456", "#2468ac");
        assert_eq!(
            execute_html(changed.as_bytes()).unwrap(),
            CanvasColor([36, 104, 172])
        );
        assert_eq!(execute_html(root).unwrap(), CanvasColor([18, 52, 86]));
    }

    #[test]
    fn collector_accepts_only_unambiguous_opaque_canvas() {
        assert_eq!(
            capture_canvas_color(&output(vec![canvas()])).unwrap(),
            CanvasColor([18, 52, 86])
        );
        let decoration = Shape::rect_filled(
            Rect::from_min_size(pos2(500.0, 20.0), vec2(20.0, 20.0)),
            0.0,
            Color32::from_rgb(18, 52, 86),
        );
        assert!(
            capture_canvas_color(&output(vec![Shape::Vec(vec![canvas(), decoration])])).is_ok()
        );
        assert_eq!(
            capture_canvas_color(&output(vec![])).unwrap_err().code,
            "capture.missing"
        );
        assert_eq!(
            capture_canvas_color(&output(vec![canvas(), canvas()]))
                .unwrap_err()
                .code,
            "capture.ambiguous"
        );
        let overlap = Shape::rect_filled(
            Rect::from_min_size(pos2(32.0, 32.0), vec2(5.0, 5.0)),
            0.0,
            Color32::from_rgb(18, 52, 86),
        );
        assert_eq!(
            capture_canvas_color(&output(vec![canvas(), overlap]))
                .unwrap_err()
                .code,
            "capture.overlap"
        );
    }

    #[test]
    fn collector_rejects_clips_effects_shapes_and_invalid_geometry() {
        let mut clipped = output(vec![canvas()]);
        clipped.shapes[0].clip_rect.max.x = 10.0;
        assert_eq!(
            capture_canvas_color(&clipped).unwrap_err().code,
            "capture.missing"
        );
        let mut scaled = output(vec![canvas()]);
        scaled.pixels_per_point = 2.0;
        assert_eq!(
            capture_canvas_color(&scaled).unwrap_err().code,
            "capture.scale"
        );
        let base = RectShape::filled(viewport(), 0.0, Color32::WHITE);
        let mut rounded = base.clone();
        rounded.corner_radius = egui::CornerRadius::same(4);
        let mut stroked = base.clone();
        stroked.stroke = egui::Stroke::new(1.0, Color32::RED);
        let mut blurred = base.clone();
        blurred.blur_width = 2.0;
        let mut transparent = base.clone();
        transparent.fill = Color32::from_white_alpha(128);
        let mut textured = base;
        textured.brush = Some(std::sync::Arc::new(egui::epaint::Brush {
            fill_texture_id: egui::TextureId::Managed(0),
            uv: Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        }));
        for rect in [rounded, stroked, blurred, transparent, textured] {
            assert!(capture_canvas_color(&output(vec![Shape::Rect(rect)])).is_err());
        }
        for shape in [
            Shape::circle_filled(pos2(32.5, 32.5), 2.0, Color32::WHITE),
            Shape::line_segment(
                [pos2(31.0, 31.0), pos2(34.0, 34.0)],
                egui::Stroke::new(1.0, Color32::WHITE),
            ),
            Shape::Callback(egui::PaintCallback {
                rect: viewport(),
                callback: std::sync::Arc::new(()),
            }),
        ] {
            assert!(capture_canvas_color(&output(vec![canvas(), shape])).is_err());
        }
        let mut invalid = output(vec![canvas()]);
        invalid.shapes[0].clip_rect.min.x = f32::NAN;
        assert_eq!(
            capture_canvas_color(&invalid).unwrap_err().code,
            "capture.geometry"
        );
        // Layer transforms are already applied by egui before FullOutput.
        let mut translated = canvas();
        translated.translate(vec2(100.0, 100.0));
        assert!(capture_canvas_color(&output(vec![translated])).is_err());
    }

    #[test]
    fn collector_rejects_text_mesh_and_path_over_the_sample() {
        let mut mesh = egui::epaint::Mesh::default();
        mesh.add_colored_rect(
            Rect::from_min_size(pos2(31.0, 31.0), vec2(5.0, 5.0)),
            Color32::WHITE,
        );
        let path = Shape::convex_polygon(
            vec![pos2(30.0, 30.0), pos2(35.0, 30.0), pos2(32.0, 35.0)],
            Color32::WHITE,
            egui::Stroke::NONE,
        );
        let ctx = egui::Context::default();
        ctx.begin_pass(RawInput::default());
        let text = ctx.fonts(|fonts| {
            Shape::text(
                fonts,
                pos2(32.5, 32.5),
                egui::Align2::CENTER_CENTER,
                "M",
                egui::FontId::proportional(16.0),
                Color32::WHITE,
            )
        });
        let _ = ctx.end_pass();
        for shape in [Shape::mesh(mesh), path, text] {
            assert_eq!(
                capture_canvas_color(&output(vec![canvas(), shape]))
                    .unwrap_err()
                    .code,
                "capture.overlap"
            );
        }
    }

    #[test]
    fn stale_shapes_and_unready_frames_cannot_be_observed() {
        let ctx = egui::Context::default();
        ctx.begin_pass(RawInput {
            screen_rect: Some(viewport()),
            ..Default::default()
        });
        ctx.layer_painter(egui::LayerId::background()).add(canvas());
        assert!(capture_canvas_color(&ctx.end_pass()).is_ok());
        ctx.begin_pass(RawInput {
            screen_rect: Some(viewport()),
            ..Default::default()
        });
        assert!(capture_canvas_color(&ctx.end_pass()).is_err());
        for status in [
            PageFrameStatus::NoDocument,
            PageFrameStatus::FollowupRequired,
        ] {
            assert!(accept_frame(status, &output(vec![canvas()])).is_err());
        }
        ctx.begin_pass(RawInput::default());
        ctx.request_discard("regression test");
        assert_eq!(
            accept_frame(PageFrameStatus::Rendered, &ctx.end_pass())
                .unwrap_err()
                .code,
            "render.discard"
        );
    }

    #[test]
    fn unresolved_stylesheet_and_image_requests_are_execution_errors() {
        for html in [
            b"<!doctype html><link rel=stylesheet href=style.css>".as_slice(),
            b"<!doctype html><img src=image.png>".as_slice(),
        ] {
            assert_eq!(
                execute_html(html).unwrap_err().code,
                "execution.subresource"
            );
        }
    }
}
