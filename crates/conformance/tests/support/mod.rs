use std::process::Output;

// The local summary is authoritative; authored reasons can contain newlines.
// Keep this assertion specific to the existing local CLI summary and exits.
pub fn assert_local_summary_exit(output: &Output) {
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = std::str::from_utf8(&output.stdout).unwrap();
    let summary = text.lines().last().expect("local report summary");
    let mut fields = summary
        .strip_prefix("SUMMARY ")
        .expect("local report summary")
        .split_ascii_whitespace();
    let mut total = 0;
    for status in [
        "PASS",
        "FAIL",
        "XFAIL",
        "XPASS",
        "UNSUPPORTED",
        "SKIP",
        "ERROR",
    ] {
        let (name, count) = fields
            .next()
            .expect("summary count")
            .split_once('=')
            .unwrap();
        assert_eq!(name, status, "{summary}");
        let count: usize = count.parse().unwrap();
        if matches!(status, "FAIL" | "XPASS" | "ERROR") {
            assert_eq!(count, 0, "{summary}");
        }
        total += count;
    }
    let incomplete: usize = fields
        .next()
        .map(|field| field.strip_prefix("INCOMPLETE=").unwrap().parse().unwrap())
        .unwrap_or(0);
    assert!(fields.next().is_none(), "{summary}");
    assert!(total + incomplete > 0, "{summary}");
    // Higher-precedence failures were rejected above. No other nonzero exit
    // satisfies this assertion, and exit 3 requires reported incomplete cases.
    assert_eq!(
        output.status.code(),
        Some(if incomplete > 0 { 3 } else { 0 }),
        "{summary}"
    );
}
