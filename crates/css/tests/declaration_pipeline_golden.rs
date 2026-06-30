use css::declaration_list_pipeline_debug_snapshot;

fn fixture_input(text: &str) -> &str {
    text.strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text)
}

#[test]
fn declaration_pipeline_snapshot_golden_ad8_declarations() {
    assert_eq!(
        declaration_list_pipeline_debug_snapshot(fixture_input(include_str!(
            "fixtures/declarations/ad8_declaration_pipeline.css"
        ))),
        include_str!("fixtures/declarations/ad8_declaration_pipeline.snap"),
    );
}
