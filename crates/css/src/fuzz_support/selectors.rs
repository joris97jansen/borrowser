use super::cursor::ByteCursor;

pub(crate) fn synthesize_selector_source(bytes: &[u8]) -> String {
    let mut cursor = ByteCursor::new(bytes);

    let selector = match cursor.choose_index(9) {
        0 => "div#hero.alpha",
        1 => "[data-kind=\"promo\"]",
        2 => "section > span.label",
        3 => "section + aside.note",
        4 => "body div.alpha",
        5 => "main article.card[data-state=\"open\"]",
        6 => ":ROOT, section:first-child > span:empty",
        7 => "li:only-child:last-child",
        _ => ":BEFORE, :IS(.alpha)",
    };

    selector.to_string()
}
