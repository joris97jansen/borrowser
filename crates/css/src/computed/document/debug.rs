//! Stable debug snapshot formatting for computed document styles.

use std::fmt::Write;

use super::model::ComputedDocumentStyle;

impl ComputedDocumentStyle {
    pub fn to_debug_snapshot(&self) -> String {
        let mut out = String::new();
        writeln!(&mut out, "version: 1").expect("write snapshot");
        writeln!(&mut out, "computed-document-style").expect("write snapshot");
        for (index, entry) in self.entries.iter().enumerate() {
            writeln!(
                &mut out,
                "element[{index}]: selector-id={} name=\"{}\"",
                entry.selector_element_id.get(),
                entry.element_name
            )
            .expect("write snapshot");
            for line in entry.style.to_debug_snapshot().lines().skip(2) {
                let line = line.strip_prefix("  ").unwrap_or(line);
                writeln!(&mut out, "  {line}").expect("write snapshot");
            }
        }
        out
    }
}
