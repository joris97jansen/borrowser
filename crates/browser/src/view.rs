use crate::input_state::DocumentInputState;
use crate::page::PageState;
use crate::resources::ResourceManager;
use css::{StyledNode, build_style_tree};
use egui::{
    Align2, Area, CentralPanel, Color32, Context, CornerRadius, Frame, Id, Margin, Order, RichText,
    Stroke, vec2,
};
pub use gfx::input::PageAction;
use gfx::viewport::{ViewportCtx, page_viewport};
use html::Node;

pub fn content(
    ctx: &Context,
    page: &mut PageState,
    input_state: &mut DocumentInputState,
    resources: &ResourceManager,
    status: Option<&String>,
    loading: bool,
) -> Option<PageAction> {
    if page.dom.is_none() {
        let visuals = ctx.style().visuals.clone();
        CentralPanel::default()
            .frame(Frame::default().fill(visuals.panel_fill))
            .show(ctx, |ui| {
                if loading {
                    ui.label("⏳ Loading…");
                }
                if let Some(s) = status {
                    ui.label(s);
                }
            });
        return None;
    }

    // IMPORTANT: borrow of page.dom is contained in this block and ends here.
    let base_fill = {
        let dom = page.dom.as_deref().unwrap();
        let style_root = build_style_tree(dom, None);
        let page_bg = find_page_background_color(&style_root);

        if let Some((r, g, b, a)) = page_bg {
            Color32::from_rgba_unmultiplied(r, g, b, a)
        } else {
            Color32::WHITE
        }
    };

    let action = CentralPanel::default()
        .frame(Frame::default().fill(base_fill))
        .show(ctx, |ui| {
            // Rebuild style_root inside closure (needed anyway for layout/paint).
            let dom = page.dom.as_deref().unwrap();
            let style_root = build_style_tree(dom, None);

            // disjoint borrow: OK (dom is immutably borrowed, input_values mutably borrowed)
            let base_url = page.base_url.as_deref();
            let input_values = &mut input_state.input_values;
            let form_controls = &page.form_controls;
            let interaction = &mut input_state.interaction;

            page_viewport(ViewportCtx::new(
                ui,
                &style_root,
                base_url,
                resources,
                input_values,
                form_controls,
                interaction,
            ))
        })
        .inner;

    show_status_overlay(ctx, loading, status.map(|status| status.as_str()));
    action
}

fn show_status_overlay(ctx: &Context, loading: bool, status: Option<&str>) {
    let lines = overlay_lines(loading, status);
    if lines.is_empty() {
        return;
    }

    Area::new(Id::new("page_status_overlay"))
        .order(Order::Foreground)
        .anchor(Align2::RIGHT_TOP, vec2(-16.0, 16.0))
        .interactable(false)
        .show(ctx, |ui| {
            Frame::new()
                .fill(Color32::from_black_alpha(208))
                .stroke(Stroke::new(1.0, Color32::from_white_alpha(40)))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(8))
                .show(ui, |ui| {
                    ui.set_max_width(420.0);
                    for (idx, line) in lines.iter().enumerate() {
                        let text = if idx == 0 && loading {
                            RichText::new(line).strong().color(Color32::WHITE)
                        } else {
                            RichText::new(line).color(Color32::WHITE)
                        };
                        ui.label(text);
                    }
                });
        });
}

fn overlay_lines(loading: bool, status: Option<&str>) -> Vec<String> {
    let mut lines = Vec::new();
    if loading {
        lines.push("Loading…".to_string());
    }
    if let Some(status) = status {
        lines.push(status.to_string());
    }
    lines
}

fn find_page_background_color(root: &StyledNode<'_>) -> Option<(u8, u8, u8, u8)> {
    // We prefer <body> background if present and non-transparent.
    // If not, we fall back to <html>. Otherwise: None.
    fn is_non_transparent_rgba(rgba: (u8, u8, u8, u8)) -> bool {
        let (_r, _g, _b, a) = rgba;
        a > 0
    }

    fn from_elem(node: &StyledNode<'_>, want: &str) -> Option<(u8, u8, u8, u8)> {
        match node.node {
            Node::Element { name, .. } if name.eq_ignore_ascii_case(want) => {
                let rgba = node.style.background_color;
                if is_non_transparent_rgba(rgba) {
                    Some(rgba)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    // root.node is the Document. We look for <html> first-level children,
    // then <body> beneath those. This matches the usual structure.
    // Prefer <body>, fallback to <html>.
    let mut html_bg = None;
    let mut body_bg = None;

    for child in &root.children {
        if html_bg.is_none() {
            html_bg = from_elem(child, "html");
        }

        for gc in &child.children {
            if body_bg.is_none() {
                body_bg = from_elem(gc, "body");
            }
        }
    }

    body_bg.or(html_bg)
}

#[cfg(test)]
mod tests {
    use super::overlay_lines;

    #[test]
    fn overlay_lines_include_loading_and_status() {
        let lines = overlay_lines(true, Some("Document parsed • HTTP 200"));
        assert_eq!(lines, vec!["Loading…", "Document parsed • HTTP 200"]);
    }

    #[test]
    fn overlay_lines_are_empty_without_loading_or_status() {
        assert!(overlay_lines(false, None).is_empty());
    }
}
