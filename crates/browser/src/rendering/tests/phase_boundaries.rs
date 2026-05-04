use crate::rendering::*;
use css::Display;
use gfx::paint::PaintPhaseInput;
use html::Node;
use layout::{LayoutPhaseInput, layout_document};

use super::support::*;

#[test]
fn render_phase_boundary_debug_snapshot_is_stable_for_simple_text_flow() {
    let mut page = page_with_dom(
        "<!doctype html><html style=\"display: inline;\"><head style=\"display: inline;\"><style style=\"display: inline;\">html { background-color: white; } p { color: red; }</style></head><body style=\"display: inline;\"><p style=\"display: inline;\">Hello</p></body></html>",
    );
    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::DocumentReplaced,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::StylesheetSetChanged,
    ));

    let snapshot = render_phase_boundary_debug_snapshot(
        &mut page,
        pending,
        320.0,
        &FixedTextMeasurer,
        None,
        false,
    )
    .expect("snapshot should build")
    .expect("document should produce a pipeline snapshot");
    let first = snapshot.to_debug_snapshot();
    let second = render_phase_boundary_debug_snapshot(
        &mut page,
        pending_for_simple_text_flow(),
        320.0,
        &FixedTextMeasurer,
        None,
        false,
    )
    .expect("snapshot should rebuild deterministically")
    .expect("document should still produce a pipeline snapshot")
    .to_debug_snapshot();
    assert_eq!(first, second);
    assert_eq!(
        first,
        r#"version: 1
render-phase-boundaries
style-output:
  version: 1
  style-phase-output
  root-id: 0
  styled-nodes: 8
  node[0]: id=0 kind=document children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
    node[1]: id=0 kind=element name="html" children=2 style=display=inline color=rgba(0,0,0,255) background=rgba(255,255,255,255) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      node[2]: id=0 kind=element name="head" children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        node[3]: id=0 kind=element name="style" children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          node[4]: id=0 kind=text text="html { background-color: white; } p { color: red; }" children=0 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      node[5]: id=0 kind=element name="body" children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        node[6]: id=0 kind=element name="p" children=1 style=display=inline color=rgba(255,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          node[7]: id=0 kind=text text="Hello" children=0 style=display=inline color=rgba(255,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
layout-input:
  version: 1
  layout-phase-input
  available-width: 320.00
  style-root-id: 0
  style-root: document
  style-nodes: 8
  has-replaced-info: false
layout-output:
  version: 1
  layout-phase-output
  viewport-width: 320.00
  document-rect: x=0.00 y=0.00 w=320.00 h=0.00
  layout-boxes: 8
  box[0]: box-id=b0 anchor-id=0 source=dom(0) node=document kind=block cb=none establishes-cb=yes fc=none establishes-fc=block block-participation=root rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
    box[1]: box-id=b1 anchor-id=0 source=dom(0) node=element("html") kind=block cb=b0 establishes-cb=yes fc=b0 establishes-fc=block block-participation=block-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=2 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(255,255,255,255) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      box[2]: box-id=b2 anchor-id=0 source=dom(0) node=element("head") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[3]: box-id=b3 anchor-id=0 source=dom(0) node=element("style") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[4]: box-id=b4 anchor-id=0 source=dom(0) node=text("html { background-color: white; } p { color: red; }") kind=block cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=0 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      box[5]: box-id=b5 anchor-id=0 source=dom(0) node=element("body") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[6]: box-id=b6 anchor-id=0 source=dom(0) node=element("p") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(255,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[7]: box-id=b7 anchor-id=0 source=dom(0) node=text("Hello") kind=block cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=0 marker=none replaced=none intrinsic=none style=display=inline color=rgba(255,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
paint-input:
  version: 1
  paint-phase-input
  layout-root-id: 0
  viewport-width: 320.00
  document-rect: x=0.00 y=0.00 w=320.00 h=0.00
    layout-boxes: 8
    box[0]: box-id=b0 anchor-id=0 source=dom(0) node=document kind=block cb=none establishes-cb=yes fc=none establishes-fc=block block-participation=root rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      box[1]: box-id=b1 anchor-id=0 source=dom(0) node=element("html") kind=block cb=b0 establishes-cb=yes fc=b0 establishes-fc=block block-participation=block-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=2 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(255,255,255,255) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[2]: box-id=b2 anchor-id=0 source=dom(0) node=element("head") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[3]: box-id=b3 anchor-id=0 source=dom(0) node=element("style") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
            box[4]: box-id=b4 anchor-id=0 source=dom(0) node=text("html { background-color: white; } p { color: red; }") kind=block cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=0 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[5]: box-id=b5 anchor-id=0 source=dom(0) node=element("body") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[6]: box-id=b6 anchor-id=0 source=dom(0) node=element("p") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(255,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
            box[7]: box-id=b7 anchor-id=0 source=dom(0) node=text("Hello") kind=block cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=320.00 h=0.00 children=0 marker=none replaced=none intrinsic=none style=display=inline color=rgba(255,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
orchestration:
  version: 1
  render-frame-execution-trace
  triggered-entry-points: 2
    - document-replaced
    - stylesheet-set-changed
  style: phase=style kind=requested
    direct-triggers: 2
      - dom-replaced
      - stylesheet-set-changed
    cascaded-from: 0
  layout: phase=layout kind=requested
    direct-triggers: 0
    cascaded-from: 1
      - style
  paint: phase=paint kind=requested
    direct-triggers: 0
    cascaded-from: 1
      - layout
  frame-orchestration: phase=frame-orchestration kind=requested
    direct-triggers: 0
    cascaded-from: 1
      - style
  semantic-phase-order: style -> layout -> paint
"#
    );
}

#[test]
fn render_phase_boundary_debug_snapshot_is_stable_for_replaced_element_flow() {
    let mut page = page_with_dom(
        "<!doctype html><html style=\"display: inline;\"><head style=\"display: inline;\"><style style=\"display: inline;\">img { display: inline-block; }</style></head><body style=\"display: inline;\"><img src=\"hero.png\"></body></html>",
    );
    let warm_style = style_output_for_test(&mut page);
    drop(warm_style);

    let mut pending = PendingRenderWork::default();
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::ResourceStateChanged,
    ));
    pending.push(render_invalidation_request(
        RenderInvalidationEntryPoint::InputStateChanged,
    ));

    let snapshot = render_phase_boundary_debug_snapshot(
        &mut page,
        pending,
        240.0,
        &FixedTextMeasurer,
        Some(&FixedReplacedInfo),
        true,
    )
    .expect("snapshot should build")
    .expect("document should produce a pipeline snapshot");
    let first = snapshot.to_debug_snapshot();
    let second = render_phase_boundary_debug_snapshot(
        &mut page,
        pending_for_replaced_element_flow(),
        240.0,
        &FixedTextMeasurer,
        Some(&FixedReplacedInfo),
        true,
    )
    .expect("snapshot should rebuild deterministically")
    .expect("document should still produce a pipeline snapshot")
    .to_debug_snapshot();
    assert_eq!(first, second);
    assert_eq!(
        first,
        r#"version: 1
render-phase-boundaries
style-output:
  version: 1
  style-phase-output
  root-id: 0
  styled-nodes: 7
  node[0]: id=0 kind=document children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
    node[1]: id=0 kind=element name="html" children=2 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      node[2]: id=0 kind=element name="head" children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        node[3]: id=0 kind=element name="style" children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          node[4]: id=0 kind=text text="img { display: inline-block; }" children=0 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      node[5]: id=0 kind=element name="body" children=1 style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        node[6]: id=0 kind=element name="img" children=0 style=display=inline-block color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
layout-input:
  version: 1
  layout-phase-input
  available-width: 240.00
  style-root-id: 0
  style-root: document
  style-nodes: 7
  has-replaced-info: true
layout-output:
  version: 1
  layout-phase-output
  viewport-width: 240.00
  document-rect: x=0.00 y=0.00 w=240.00 h=0.00
  layout-boxes: 7
  box[0]: box-id=b0 anchor-id=0 source=dom(0) node=document kind=block cb=none establishes-cb=yes fc=none establishes-fc=block block-participation=root rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
    box[1]: box-id=b1 anchor-id=0 source=dom(0) node=element("html") kind=block cb=b0 establishes-cb=yes fc=b0 establishes-fc=block block-participation=block-level rect=x=0.00 y=0.00 w=240.00 h=0.00 children=2 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      box[2]: box-id=b2 anchor-id=0 source=dom(0) node=element("head") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[3]: box-id=b3 anchor-id=0 source=dom(0) node=element("style") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[4]: box-id=b4 anchor-id=0 source=dom(0) node=text("img { display: inline-block; }") kind=block cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=240.00 h=0.00 children=0 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      box[5]: box-id=b5 anchor-id=0 source=dom(0) node=element("body") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[6]: box-id=b6 anchor-id=0 source=dom(0) node=element("img") kind=replaced-inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=atomic-inline rect=x=0.00 y=0.00 w=64.00 h=32.00 children=0 marker=none replaced=img intrinsic=w=64.00px h=32.00px ratio=2.0000 style=display=inline-block color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
paint-input:
  version: 1
  paint-phase-input
  layout-root-id: 0
  viewport-width: 240.00
  document-rect: x=0.00 y=0.00 w=240.00 h=0.00
    layout-boxes: 7
    box[0]: box-id=b0 anchor-id=0 source=dom(0) node=document kind=block cb=none establishes-cb=yes fc=none establishes-fc=block block-participation=root rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
      box[1]: box-id=b1 anchor-id=0 source=dom(0) node=element("html") kind=block cb=b0 establishes-cb=yes fc=b0 establishes-fc=block block-participation=block-level rect=x=0.00 y=0.00 w=240.00 h=0.00 children=2 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[2]: box-id=b2 anchor-id=0 source=dom(0) node=element("head") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[3]: box-id=b3 anchor-id=0 source=dom(0) node=element("style") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
            box[4]: box-id=b4 anchor-id=0 source=dom(0) node=text("img { display: inline-block; }") kind=block cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=240.00 h=0.00 children=0 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
        box[5]: box-id=b5 anchor-id=0 source=dom(0) node=element("body") kind=inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=inline-level rect=x=0.00 y=0.00 w=240.00 h=0.00 children=1 marker=none replaced=none intrinsic=none style=display=inline color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
          box[6]: box-id=b6 anchor-id=0 source=dom(0) node=element("img") kind=replaced-inline cb=b1 establishes-cb=no fc=b1 establishes-fc=none block-participation=atomic-inline rect=x=0.00 y=0.00 w=64.00 h=32.00 children=0 marker=none replaced=img intrinsic=w=64.00px h=32.00px ratio=2.0000 style=display=inline-block color=rgba(0,0,0,255) background=rgba(0,0,0,0) font-size=16.00px width=auto height=auto margin=[0.00,0.00,0.00,0.00] padding=[0.00,0.00,0.00,0.00]
orchestration:
  version: 1
  render-frame-execution-trace
  triggered-entry-points: 3
    - resource-state-changed
    - input-state-changed
    - viewport-changed
  style: phase=style kind=materialized-from-retained-artifacts
    direct-triggers: 0
    cascaded-from: 0
  layout: phase=layout kind=requested
    direct-triggers: 2
      - resource-state-changed
      - viewport-changed
    cascaded-from: 0
  paint: phase=paint kind=requested
    direct-triggers: 2
      - resource-state-changed
      - input-state-changed
    cascaded-from: 1
      - layout
  frame-orchestration: phase=frame-orchestration kind=requested
    direct-triggers: 3
      - resource-state-changed
      - input-state-changed
      - viewport-changed
    cascaded-from: 0
  semantic-phase-order: style -> layout -> paint
"#
    );
}

#[test]
fn render_phase_boundary_debug_snapshot_preserves_non_zero_dom_identity_across_handoffs() {
    let mut page = page_with_node(doc_with_explicit_ids());

    let warm_style = style_output_for_test(&mut page);
    assert_eq!(warm_style.root().node_id, html::internal::Id(1));

    let paragraph = find_styled_node_id(warm_style.root(), html::internal::Id(4))
        .expect("paragraph styled node");
    assert!(matches!(paragraph.node, Node::Element { name, .. } if name.as_ref() == "p"));
    drop(warm_style);

    let style_output = style_output_for_test(&mut page);
    let layout_input =
        LayoutPhaseInput::from_style_output(&style_output, 360.0, &FixedTextMeasurer, None);
    assert_eq!(layout_input.style_root().node_id, html::internal::Id(1));

    let layout_output = layout_document(layout_input);
    assert_eq!(layout_output.root().node_id(), html::internal::Id(1));
    let paragraph_box = find_layout_box_by_id(layout_output.root(), html::internal::Id(4))
        .expect("paragraph layout box");
    assert_eq!(paragraph_box.node_id(), html::internal::Id(4));

    let paint_input = PaintPhaseInput::new(&layout_output);
    assert_eq!(paint_input.layout_root().node_id(), html::internal::Id(1));
    drop(style_output);

    let snapshot = render_phase_boundary_debug_snapshot(
        &mut page,
        PendingRenderWork::default(),
        360.0,
        &FixedTextMeasurer,
        None,
        false,
    )
    .expect("snapshot should build")
    .expect("document should produce a pipeline snapshot")
    .to_debug_snapshot();

    assert!(snapshot.contains("root-id: 1"));
    assert!(snapshot.contains("style-root-id: 1"));
    assert!(snapshot.contains("layout-root-id: 1"));
    assert!(snapshot.contains("id=4 kind=element name=\"p\""));
    assert!(snapshot.contains("anchor-id=4 source=dom(4) node=element(\"p\")"));
}

#[test]
fn style_to_layout_handoff_uses_explicit_phase_output_models() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    let style_output = style_output_for_test(&mut page);
    let paragraph = find_styled_element(style_output.root(), "p").expect("paragraph");
    let measurer = FixedTextMeasurer;

    let layout_input = LayoutPhaseInput::from_style_output(&style_output, 320.0, &measurer, None);
    assert!(std::ptr::eq(layout_input.style_root(), style_output.root()));
    assert_eq!(layout_input.available_width(), 320.0);

    let layout_output = layout_document(layout_input);
    let layout_root = layout_output.root();
    let paragraph_box =
        find_layout_box_by_id(layout_root, paragraph.node_id).expect("paragraph layout box");
    assert_eq!(layout_output.document_rect(), layout_root.rect);
    assert_eq!(layout_output.viewport_width(), 320.0);
    assert_eq!(layout_root.node_id(), style_output.root().node_id);
    assert_eq!(paragraph_box.node_id(), paragraph.node_id);
}

#[test]
fn runtime_style_phase_applies_minimal_ua_display_defaults() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><title>Hidden</title><meta name=\"x\" content=\"y\"><link rel=\"stylesheet\" href=\"missing.css\"><style>p { color: red; }</style><script>hidden()</script></head><body><p>Hello <span>world</span></p><ul><li>One</li></ul><input><button>Go</button><textarea>Text</textarea></body></html>",
    );
    let style_output = style_output_for_test(&mut page);

    assert_eq!(
        styled_element_display(style_output.root(), "html"),
        Display::Block
    );
    assert_eq!(
        styled_element_display(style_output.root(), "body"),
        Display::Block
    );
    assert_eq!(
        styled_element_display(style_output.root(), "p"),
        Display::Block
    );
    assert_eq!(
        styled_element_display(style_output.root(), "span"),
        Display::Inline
    );
    assert_eq!(
        styled_element_display(style_output.root(), "li"),
        Display::ListItem
    );
    assert_eq!(
        styled_element_display(style_output.root(), "button"),
        Display::InlineBlock
    );
    assert_eq!(
        styled_element_display(style_output.root(), "input"),
        Display::InlineBlock
    );
    assert_eq!(
        styled_element_display(style_output.root(), "textarea"),
        Display::InlineBlock
    );
    assert_eq!(
        styled_element_display(style_output.root(), "head"),
        Display::None
    );
    assert_eq!(
        styled_element_display(style_output.root(), "title"),
        Display::None
    );
    assert_eq!(
        styled_element_display(style_output.root(), "meta"),
        Display::None
    );
    assert_eq!(
        styled_element_display(style_output.root(), "link"),
        Display::None
    );
    assert_eq!(
        styled_element_display(style_output.root(), "style"),
        Display::None
    );
    assert_eq!(
        styled_element_display(style_output.root(), "script"),
        Display::None
    );

    let measurer = FixedTextMeasurer;
    let layout_output = layout_document(LayoutPhaseInput::from_style_output(
        &style_output,
        320.0,
        &measurer,
        None,
    ));

    assert!(
        layout_output.content_height() > 0.0,
        "minimal UA display defaults should let ordinary body text produce visible layout"
    );

    let layout_snapshot = layout_output.to_debug_snapshot();
    assert!(!layout_snapshot.contains("node=element(\"head\")"));
    assert!(!layout_snapshot.contains("node=element(\"title\")"));
    assert!(!layout_snapshot.contains("node=element(\"meta\")"));
    assert!(!layout_snapshot.contains("node=element(\"link\")"));
    assert!(!layout_snapshot.contains("node=element(\"style\")"));
    assert!(!layout_snapshot.contains("node=element(\"script\")"));
    assert!(!layout_snapshot.contains("Hidden"));
    assert!(!layout_snapshot.contains("p { color: red; }"));
    assert!(!layout_snapshot.contains("hidden()"));
}

#[test]
fn runtime_ua_display_defaults_are_author_and_inline_overridable() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { display: inline; } head, title { display: block; }</style><title>Shown title</title></head><body><p id=\"author\">Author</p><div style=\"display: inline-block;\">Inline</div></body></html>",
    );
    let style_output = style_output_for_test(&mut page);

    assert_eq!(
        styled_element_display(style_output.root(), "p"),
        Display::Inline
    );
    assert_eq!(
        styled_element_display(style_output.root(), "div"),
        Display::InlineBlock
    );
    assert_eq!(
        styled_element_display(style_output.root(), "head"),
        Display::Block
    );
    assert_eq!(
        styled_element_display(style_output.root(), "title"),
        Display::Block
    );

    let measurer = FixedTextMeasurer;
    let layout_output = layout_document(LayoutPhaseInput::from_style_output(
        &style_output,
        320.0,
        &measurer,
        None,
    ));
    let layout_snapshot = layout_output.to_debug_snapshot();

    assert!(layout_snapshot.contains("node=element(\"title\")"));
    assert!(layout_snapshot.contains("text(\"Shown title\")"));
}

#[test]
fn authored_stylesheet_reporting_excludes_built_in_ua_styles() {
    let page = page_with_dom(
        "<!doctype html><html><head><style>p { display: inline; }</style></head><body><p>Hello</p></body></html>",
    );

    assert_eq!(
        page.css_stylesheets().len(),
        1,
        "authored stylesheet reporting must not include the built-in UA stylesheet"
    );
}

#[test]
fn layout_to_paint_handoff_wraps_layout_phase_output_without_reinterpretation() {
    let mut page = page_with_dom(
        "<!doctype html><html><head><style>p { color: red; }</style></head><body><p>Hello</p></body></html>",
    );
    let style_output = style_output_for_test(&mut page);
    let measurer = FixedTextMeasurer;
    let layout_output = layout_document(LayoutPhaseInput::from_style_output(
        &style_output,
        480.0,
        &measurer,
        None,
    ));

    let paint_input = PaintPhaseInput::new(&layout_output);
    assert!(std::ptr::eq(paint_input.layout(), &layout_output));
    assert!(std::ptr::eq(
        paint_input.layout_root(),
        layout_output.root()
    ));
    assert_eq!(
        paint_input.layout().document_rect(),
        layout_output.document_rect()
    );
    assert_eq!(
        paint_input.layout_root().node_id(),
        layout_output.root().node_id()
    );
}
