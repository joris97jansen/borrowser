use super::super::*;
use super::support::*;

#[test]
fn box_tree_debug_snapshot_is_stable_and_structural() {
    let dom = doc(vec![element(
        2,
        "div",
        Vec::new(),
        vec![element(3, "span", Vec::new(), vec![text(4, "x")])],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);

    assert_eq!(
        tree.to_debug_snapshot(),
        concat!(
            "version: 1\n",
            "box-tree\n",
            "root: b0\n",
            "boxes: 4\n",
            "b0: parent=none cb=none establishes-cb=yes position=static flow=in-flow positioned-cb=none establishes-positioned-cb=yes fc=none establishes-fc=block block-participation=root flex-participation=none ifc=none establishes-ifc=no inline-participation=none source-id=1 source=document role=document-root kind=block display=inline behavior=document-root children=[b1] marker=none replaced=none intrinsic=none\n",
            "  b1: parent=b0 cb=b0 establishes-cb=yes position=static flow=in-flow positioned-cb=b0 establishes-positioned-cb=no fc=b0 establishes-fc=block block-participation=block-level flex-participation=none ifc=none establishes-ifc=yes inline-participation=none source-id=2 source=element(\"div\") role=document-element kind=block display=block behavior=document-element children=[b2] marker=none replaced=none intrinsic=none\n",
            "    b2: parent=b1 cb=b1 establishes-cb=no position=static flow=in-flow positioned-cb=b1 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=inline-level flex-participation=none ifc=b1 establishes-ifc=no inline-participation=inline-container source-id=3 source=element(\"span\") role=ordinary-element kind=inline display=inline behavior=inline children=[b3] marker=none replaced=none intrinsic=none\n",
            "      b3: parent=b2 cb=b1 establishes-cb=no position=static flow=in-flow positioned-cb=b1 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=inline-level flex-participation=none ifc=b1 establishes-ifc=no inline-participation=text-run source-id=4 source=text(\"x\") role=text-run kind=block display=inline behavior=text-run children=[] marker=none replaced=none intrinsic=none\n",
        )
    );
}

#[test]
fn box_tree_debug_snapshot_covers_generation_and_formatting_foundations() {
    let dom = doc(vec![element(
        2,
        "html",
        Vec::new(),
        vec![element(
            3,
            "body",
            Vec::new(),
            vec![
                element(
                    4,
                    "div",
                    vec![("display", "block")],
                    vec![
                        text(5, "before"),
                        comment(6, "ignored"),
                        element(7, "span", Vec::new(), vec![text(8, "inline")]),
                        element(9, "p", vec![("display", "block")], vec![text(10, "block")]),
                        element(11, "span", Vec::new(), vec![text(12, "after")]),
                    ],
                ),
                element(
                    13,
                    "ul",
                    vec![("display", "block")],
                    vec![element(
                        14,
                        "li",
                        vec![("display", "list-item")],
                        vec![text(15, "item")],
                    )],
                ),
                element(16, "img", vec![("display", "inline-block")], Vec::new()),
                element(
                    17,
                    "span",
                    vec![("display", "inline-block")],
                    vec![text(18, "atomic")],
                ),
                element(
                    19,
                    "div",
                    vec![("display", "none")],
                    vec![text(20, "hidden")],
                ),
            ],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);
    let snapshot = tree.to_debug_snapshot();

    assert!(
        !snapshot.contains("ignored"),
        "comments must not generate box-tree debug output"
    );
    assert!(
        !snapshot.contains("hidden"),
        "display:none subtrees must not generate box-tree debug output"
    );
    assert!(
        !snapshot.contains("source-id=19"),
        "display:none element source nodes must be absent from generated boxes"
    );
    assert!(
        !snapshot.contains("source-id=20"),
        "display:none descendants must be absent from generated boxes"
    );
    assert_eq!(
        snapshot,
        r#"version: 1
box-tree
root: b0
boxes: 20
b0: parent=none cb=none establishes-cb=yes position=static flow=in-flow positioned-cb=none establishes-positioned-cb=yes fc=none establishes-fc=block block-participation=root flex-participation=none ifc=none establishes-ifc=no inline-participation=none source-id=1 source=document role=document-root kind=block display=inline behavior=document-root children=[b1] marker=none replaced=none intrinsic=none
  b1: parent=b0 cb=b0 establishes-cb=yes position=static flow=in-flow positioned-cb=b0 establishes-positioned-cb=no fc=b0 establishes-fc=block block-participation=block-level flex-participation=none ifc=none establishes-ifc=no inline-participation=none source-id=2 source=element("html") role=document-element kind=block display=block behavior=document-element children=[b2] marker=none replaced=none intrinsic=none
    b2: parent=b1 cb=b1 establishes-cb=yes position=static flow=in-flow positioned-cb=b1 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=block-level flex-participation=none ifc=none establishes-ifc=no inline-participation=none source-id=3 source=element("body") role=ordinary-element kind=block display=block behavior=block children=[b3, b13, b16] marker=none replaced=none intrinsic=none
      b3: parent=b2 cb=b2 establishes-cb=yes position=static flow=in-flow positioned-cb=b2 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=block-level flex-participation=none ifc=none establishes-ifc=no inline-participation=none source-id=4 source=element("div") role=ordinary-element kind=block display=block behavior=block children=[b4, b8, b10] marker=none replaced=none intrinsic=none
        b4: parent=b3 cb=b3 establishes-cb=yes position=static flow=in-flow positioned-cb=b3 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=block-level flex-participation=none ifc=none establishes-ifc=yes inline-participation=none source-id=none source=element("div") role=anonymous-block kind=block display=block behavior=anonymous children=[b5, b6] marker=none replaced=none intrinsic=none
          b5: parent=b4 cb=b4 establishes-cb=no position=static flow=in-flow positioned-cb=b4 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=inline-level flex-participation=none ifc=b4 establishes-ifc=no inline-participation=text-run source-id=5 source=text("before") role=text-run kind=block display=block behavior=text-run children=[] marker=none replaced=none intrinsic=none
          b6: parent=b4 cb=b4 establishes-cb=no position=static flow=in-flow positioned-cb=b4 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=inline-level flex-participation=none ifc=b4 establishes-ifc=no inline-participation=inline-container source-id=7 source=element("span") role=ordinary-element kind=inline display=inline behavior=inline children=[b7] marker=none replaced=none intrinsic=none
            b7: parent=b6 cb=b4 establishes-cb=no position=static flow=in-flow positioned-cb=b4 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=inline-level flex-participation=none ifc=b4 establishes-ifc=no inline-participation=text-run source-id=8 source=text("inline") role=text-run kind=block display=inline behavior=text-run children=[] marker=none replaced=none intrinsic=none
        b8: parent=b3 cb=b3 establishes-cb=yes position=static flow=in-flow positioned-cb=b3 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=block-level flex-participation=none ifc=none establishes-ifc=yes inline-participation=none source-id=9 source=element("p") role=ordinary-element kind=block display=block behavior=block children=[b9] marker=none replaced=none intrinsic=none
          b9: parent=b8 cb=b8 establishes-cb=no position=static flow=in-flow positioned-cb=b8 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=inline-level flex-participation=none ifc=b8 establishes-ifc=no inline-participation=text-run source-id=10 source=text("block") role=text-run kind=block display=block behavior=text-run children=[] marker=none replaced=none intrinsic=none
        b10: parent=b3 cb=b3 establishes-cb=yes position=static flow=in-flow positioned-cb=b3 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=block-level flex-participation=none ifc=none establishes-ifc=yes inline-participation=none source-id=none source=element("div") role=anonymous-block kind=block display=block behavior=anonymous children=[b11] marker=none replaced=none intrinsic=none
          b11: parent=b10 cb=b10 establishes-cb=no position=static flow=in-flow positioned-cb=b10 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=inline-level flex-participation=none ifc=b10 establishes-ifc=no inline-participation=inline-container source-id=11 source=element("span") role=ordinary-element kind=inline display=inline behavior=inline children=[b12] marker=none replaced=none intrinsic=none
            b12: parent=b11 cb=b10 establishes-cb=no position=static flow=in-flow positioned-cb=b10 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=inline-level flex-participation=none ifc=b10 establishes-ifc=no inline-participation=text-run source-id=12 source=text("after") role=text-run kind=block display=inline behavior=text-run children=[] marker=none replaced=none intrinsic=none
      b13: parent=b2 cb=b2 establishes-cb=yes position=static flow=in-flow positioned-cb=b2 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=block-level flex-participation=none ifc=none establishes-ifc=no inline-participation=none source-id=13 source=element("ul") role=ordinary-element kind=block display=block behavior=block children=[b14] marker=none replaced=none intrinsic=none
        b14: parent=b13 cb=b13 establishes-cb=yes position=static flow=in-flow positioned-cb=b13 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=block-level flex-participation=none ifc=none establishes-ifc=yes inline-participation=none source-id=14 source=element("li") role=ordinary-element kind=block display=list-item behavior=list-item children=[b15] marker=unordered replaced=none intrinsic=none
          b15: parent=b14 cb=b14 establishes-cb=no position=static flow=in-flow positioned-cb=b14 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=inline-level flex-participation=none ifc=b14 establishes-ifc=no inline-participation=text-run source-id=15 source=text("item") role=text-run kind=block display=list-item behavior=text-run children=[] marker=none replaced=none intrinsic=none
      b16: parent=b2 cb=b2 establishes-cb=yes position=static flow=in-flow positioned-cb=b2 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=block-level flex-participation=none ifc=none establishes-ifc=yes inline-participation=none source-id=none source=element("body") role=anonymous-block kind=block display=block behavior=anonymous children=[b17, b18] marker=none replaced=none intrinsic=none
        b17: parent=b16 cb=b16 establishes-cb=no position=static flow=in-flow positioned-cb=b16 establishes-positioned-cb=no fc=b1 establishes-fc=none block-participation=atomic-inline flex-participation=none ifc=b16 establishes-ifc=no inline-participation=atomic-inline source-id=16 source=element("img") role=ordinary-element kind=replaced-inline display=inline-block behavior=replaced-inline children=[] marker=none replaced=img intrinsic=none
        b18: parent=b16 cb=b16 establishes-cb=yes position=static flow=in-flow positioned-cb=b16 establishes-positioned-cb=no fc=b1 establishes-fc=block block-participation=atomic-inline flex-participation=none ifc=b16 establishes-ifc=yes inline-participation=atomic-inline source-id=17 source=element("span") role=ordinary-element kind=inline-block display=inline-block behavior=inline-block children=[b19] marker=none replaced=none intrinsic=none
          b19: parent=b18 cb=b18 establishes-cb=no position=static flow=in-flow positioned-cb=b18 establishes-positioned-cb=no fc=b18 establishes-fc=none block-participation=inline-level flex-participation=none ifc=b18 establishes-ifc=no inline-participation=text-run source-id=18 source=text("atomic") role=text-run kind=block display=inline-block behavior=text-run children=[] marker=none replaced=none intrinsic=none
"#
    );
}

#[test]
fn box_tree_debug_snapshot_exposes_flex_container_and_item_metadata() {
    let dom = doc(vec![element(
        2,
        "html",
        Vec::new(),
        vec![element(
            3,
            "body",
            Vec::new(),
            vec![element(
                4,
                "section",
                vec![("display", "flex")],
                vec![element(5, "div", Vec::new(), Vec::new())],
            )],
        )],
    )]);
    let styled = css::build_style_tree(&dom, None);
    let tree = BoxTree::generate(&styled, None);
    let snapshot = tree.to_debug_snapshot();

    assert!(snapshot.contains(
        "source-id=4 source=element(\"section\") role=ordinary-element kind=block display=flex behavior=flex-container"
    ));
    assert!(snapshot.contains("establishes-fc=flex"));
    assert!(snapshot.contains(
        "source-id=5 source=element(\"div\") role=ordinary-element kind=block display=block behavior=block"
    ));
    assert!(snapshot.contains("flex-participation=flex-item"));
}
