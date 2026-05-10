use super::cursor::ByteCursor;
use crate::{PropertyId, PropertyLengthSignPolicy, PropertySpecifiedValueKind};

const DISPLAY_VALUES: &[&str] = &[
    "block",
    "inline",
    "inline-block",
    "list-item",
    "none",
    "grid",
];

const OVERFLOW_VALUES: &[&str] = &["visible", "hidden", "clip", "scroll", "auto", "overlay"];

const COLOR_VALUES: &[&str] = &[
    "red",
    "#112233",
    "#00ff00",
    "transparent",
    "#12",
    "rgb(1,2,3)",
];

pub(crate) fn synthesized_supported_stylesheet_suite(bytes: &[u8], raw_css: &str) -> Vec<String> {
    let mut cursor = ByteCursor::new(bytes);

    vec![
        raw_css.to_string(),
        format!(
            "body {{ color: {}; font-size: {}; }}\n\
             div#hero.alpha {{ width: {}; display: {}; overflow: {}; }}\n\
             section > span.label {{ background-color: {}; padding-left: {}; }}\n\
             section + aside.note {{ max-width: {}; margin-top: {}; }}\n\
             [data-kind=\"promo\"] {{ min-width: {}; }}",
            cursor.choose_str(&COLOR_VALUES[..4]),
            supported_absolute_length_value(&mut cursor, false),
            supported_auto_length_value(&mut cursor),
            cursor.choose_str(&DISPLAY_VALUES[..5]),
            cursor.choose_str(&OVERFLOW_VALUES[..5]),
            cursor.choose_str(&COLOR_VALUES[..4]),
            supported_absolute_length_value(&mut cursor, false),
            supported_none_length_value(&mut cursor),
            supported_absolute_length_value(&mut cursor, true),
            supported_auto_length_value(&mut cursor),
        ),
    ]
}

pub(crate) fn synthesized_value_cases(
    property: PropertyId,
    raw_value: &str,
    seed: u64,
) -> Vec<String> {
    let seed_bytes = seed.to_le_bytes();
    let mut cursor = ByteCursor::new(&seed_bytes);

    vec![
        raw_value.to_string(),
        synthesized_value_for_property(property, &mut cursor, true),
        synthesized_value_for_property(property, &mut cursor, false),
    ]
}

fn synthesized_value_for_property(
    property: PropertyId,
    cursor: &mut ByteCursor<'_>,
    valid_bias: bool,
) -> String {
    match property.metadata().specified_value {
        PropertySpecifiedValueKind::Color => {
            if valid_bias {
                cursor.choose_str(&COLOR_VALUES[..4]).to_string()
            } else {
                cursor.choose_str(&COLOR_VALUES[3..]).to_string()
            }
        }
        PropertySpecifiedValueKind::DisplayKeyword => {
            if valid_bias {
                cursor.choose_str(&DISPLAY_VALUES[..5]).to_string()
            } else {
                cursor.choose_str(&DISPLAY_VALUES[4..]).to_string()
            }
        }
        PropertySpecifiedValueKind::OverflowKeyword => {
            if valid_bias {
                cursor.choose_str(&OVERFLOW_VALUES[..5]).to_string()
            } else {
                cursor.choose_str(&OVERFLOW_VALUES[5..]).to_string()
            }
        }
        PropertySpecifiedValueKind::AbsoluteLength => absolute_length_value(
            cursor,
            property.metadata().length_sign == PropertyLengthSignPolicy::AllowNegative,
        ),
        PropertySpecifiedValueKind::LengthPercentageOrAuto => {
            if valid_bias && cursor.next_bool() {
                "auto".to_string()
            } else if valid_bias {
                length_percentage_value(
                    cursor,
                    property.metadata().length_sign == PropertyLengthSignPolicy::AllowNegative,
                )
            } else {
                cursor
                    .choose_str(&["1em", "1e39px", "-1px", "bogus"])
                    .to_string()
            }
        }
        PropertySpecifiedValueKind::LengthPercentageOrNone => {
            if valid_bias && cursor.next_bool() {
                "none".to_string()
            } else if valid_bias {
                length_percentage_value(
                    cursor,
                    property.metadata().length_sign == PropertyLengthSignPolicy::AllowNegative,
                )
            } else {
                cursor
                    .choose_str(&["1em", "1e39px", "-1px", "bogus"])
                    .to_string()
            }
        }
    }
}

fn length_percentage_value(cursor: &mut ByteCursor<'_>, allow_negative: bool) -> String {
    if cursor.next_bool() {
        return absolute_length_value(cursor, allow_negative);
    }

    let value = if allow_negative {
        cursor.choose_str(&["0%", "12.5%", "-5%", "100%"])
    } else {
        cursor.choose_str(&["0%", "12.5%", "50%", "100%"])
    };
    value.to_string()
}

fn absolute_length_value(cursor: &mut ByteCursor<'_>, allow_negative: bool) -> String {
    let magnitude = 1 + cursor.next_usize(64);

    match cursor.choose_index(4) {
        0 => format!("{magnitude}px"),
        1 if allow_negative => format!("-{magnitude}px"),
        1 => "0".to_string(),
        2 => "1em".to_string(),
        _ => "1e39px".to_string(),
    }
}

fn supported_absolute_length_value(cursor: &mut ByteCursor<'_>, allow_negative: bool) -> String {
    let magnitude = 1 + cursor.next_usize(64);

    match cursor.choose_index(3) {
        0 => format!("{magnitude}px"),
        1 if allow_negative => format!("-{magnitude}px"),
        1 => "0".to_string(),
        _ => format!("{}px", magnitude / 2 + 1),
    }
}

fn supported_auto_length_value(cursor: &mut ByteCursor<'_>) -> String {
    if cursor.next_bool() {
        "auto".to_string()
    } else {
        supported_absolute_length_value(cursor, false)
    }
}

fn supported_none_length_value(cursor: &mut ByteCursor<'_>) -> String {
    if cursor.next_bool() {
        "none".to_string()
    } else {
        supported_absolute_length_value(cursor, false)
    }
}
