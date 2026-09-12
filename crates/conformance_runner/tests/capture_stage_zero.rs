#![cfg(feature = "external-capture")]

#[test]
fn future_modes_are_unavailable_without_opening_inputs_or_launching() {
    for args in [vec!["capture"], vec!["qualify", "--purpose", "admission"]] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_conformance-capture"))
            .args(args)
            .output()
            .expect("collector executable");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("NOT ESTABLISHED")
        );
    }
}
