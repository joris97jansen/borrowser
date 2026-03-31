use crate::dom_snapshot::{DomSnapshotOptions, assert_dom_eq};
use crate::{HtmlParseOptions, HtmlParser, parse_document};

#[test]
fn chunked_draining_leaves_no_tokens_behind() {
    let input = "<div>ok</div><!--x-->";
    let bytes = input.as_bytes();
    let sizes = [2, 3, 1];
    let mut parser =
        HtmlParser::new(HtmlParseOptions::default()).expect("chunked draining parser init");
    let mut drained_patches = Vec::new();
    let mut offset = 0usize;

    for size in sizes {
        if offset >= bytes.len() {
            break;
        }
        let end = (offset + size).min(bytes.len());
        parser
            .push_bytes(&bytes[offset..end])
            .expect("chunked draining push should succeed");
        parser.pump().expect("chunked draining pump should succeed");
        drained_patches.extend(
            parser
                .take_patches()
                .expect("chunked draining patch drain should succeed"),
        );
        offset = end;
    }
    if offset < bytes.len() {
        parser
            .push_bytes(&bytes[offset..])
            .expect("chunked draining final push should succeed");
    }
    parser
        .finish()
        .expect("chunked draining finish should succeed");
    drained_patches.extend(
        parser
            .take_patches()
            .expect("chunked draining final patch drain should succeed"),
    );

    let output = parser
        .into_output()
        .expect("chunked draining output should materialize");
    let expected = parse_document(input, HtmlParseOptions::default())
        .expect("full draining baseline parse should succeed");
    assert_eq!(
        drained_patches.len() + output.patches.len(),
        expected.patches.len(),
        "expected drained patch count to match one-shot HTML5 patch history"
    );
    assert_dom_eq(
        &output.document,
        &expected.document,
        DomSnapshotOptions::default(),
    );
}
