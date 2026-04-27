use super::cursor::ByteCursor;

pub(crate) fn synthesize_selector_source(bytes: &[u8]) -> String {
    let mut cursor = ByteCursor::new(bytes);

    let selector = match cursor.choose_index(6) {
        0 => "div#hero.alpha",
        1 => "[data-kind=\"promo\"]",
        2 => "section > span.label",
        3 => "section + aside.note",
        4 => "body div.alpha",
        _ => "main article.card[data-state=\"open\"]",
    };

    selector.to_string()
}
