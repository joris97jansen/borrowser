pub fn escape_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch < ' ' => {
                use std::fmt::Write;
                let _ = write!(&mut out, "\\u{{{:02X}}}", ch as u32);
            }
            _ => out.push(ch),
        }
    }
    out
}

pub fn diff_lines(expected: &[String], actual: &[String]) -> String {
    let max = expected.len().max(actual.len());
    let mut out = String::new();
    use std::fmt::Write;
    let mut mismatch = None;
    let missing = "<missing>";
    for i in 0..max {
        let left = expected.get(i).map(String::as_str).unwrap_or(missing);
        let right = actual.get(i).map(String::as_str).unwrap_or(missing);
        if left != right {
            mismatch = Some(i);
            break;
        }
    }
    if let Some(i) = mismatch {
        let start = i.saturating_sub(2);
        let end = (i + 3).min(max);
        let _ = writeln!(
            &mut out,
            "first mismatch at line {} (showing {}..={}):",
            i + 1,
            start + 1,
            end
        );
        for line_idx in start..end {
            let left = expected
                .get(line_idx)
                .map(String::as_str)
                .unwrap_or(missing);
            let right = actual.get(line_idx).map(String::as_str).unwrap_or(missing);
            let marker = if line_idx == i { ">" } else { " " };
            let _ = writeln!(&mut out, "{marker} {:>4}  expected: {left}", line_idx + 1);
            let _ = writeln!(&mut out, "{marker} {:>4}    actual: {right}", line_idx + 1);
        }
    }
    if expected.len() != actual.len() && mismatch.is_none() {
        let _ = writeln!(
            &mut out,
            "prefix matched but lengths differ (expected {} lines, actual {} lines)",
            expected.len(),
            actual.len()
        );
    }
    let _ = writeln!(
        &mut out,
        "expected {} lines, actual {} lines",
        expected.len(),
        actual.len()
    );
    out
}

#[cfg(feature = "html5")]
pub mod token_snapshot;

#[cfg(feature = "html5")]
pub mod wpt_tokenizer;

#[cfg(feature = "html5")]
pub mod wpt_expected;

#[cfg(feature = "html5")]
pub mod wpt_formats;
