use super::InputValueStore;
use crate::{InputId, SelectionRange, caret_from_x};

#[test]
fn insert_text_keeps_caret_on_char_boundary() {
    let mut store = InputValueStore::new();
    let id = InputId::from_raw(1);

    store.ensure_initial(id, String::new());
    store.focus(id);

    store.insert_text(id, "€"); // 3-byte UTF-8
    let value = store.get(id).unwrap();
    let caret = store.caret(id).unwrap();
    assert_eq!(value, "€");
    assert_eq!(caret, value.len());
    assert!(value.is_char_boundary(caret));
}

#[test]
fn backspace_removes_a_full_unicode_scalar_value() {
    let mut store = InputValueStore::new();
    let id = InputId::from_raw(1);

    store.set(id, "a€".to_string());
    store.focus(id);

    store.backspace(id);
    assert_eq!(store.get(id), Some("a"));
    let value = store.get(id).unwrap();
    let caret = store.caret(id).unwrap();
    assert_eq!(caret, value.len());
    assert!(value.is_char_boundary(caret));
}

#[test]
fn invalid_caret_is_clamped_before_insert() {
    let mut store = InputValueStore::new();
    let id = InputId::from_raw(1);

    store.set(id, "€".to_string());
    // Manually corrupt the caret to an invalid boundary
    store.values.get_mut(&id).unwrap().caret = 1;

    store.insert_text(id, "x");
    assert_eq!(store.get(id), Some("x€"));
    let value = store.get(id).unwrap();
    let caret = store.caret(id).unwrap();
    assert!(value.is_char_boundary(caret));
}

#[test]
fn move_caret_left_right_moves_by_unicode_scalar_value() {
    let mut store = InputValueStore::new();
    let id = InputId::from_raw(1);

    store.set(id, "a€b".to_string());
    store.focus(id);

    assert_eq!(store.caret(id), Some("a€b".len()));

    store.move_caret_left(id, false);
    assert_eq!(store.caret(id), Some("a€".len()));

    store.move_caret_left(id, false);
    assert_eq!(store.caret(id), Some("a".len()));

    store.move_caret_right(id, false);
    assert_eq!(store.caret(id), Some("a€".len()));
}

#[test]
fn shift_arrow_creates_selection_and_backspace_deletes_it() {
    let mut store = InputValueStore::new();
    let id = InputId::from_raw(1);

    store.set(id, "hello".to_string());
    store.focus(id);

    store.move_caret_left(id, true);
    let (_value, _caret, selection, _scroll_x, _scroll_y) = store.get_state(id).unwrap();
    assert_eq!(selection, Some(SelectionRange { start: 4, end: 5 }));

    store.backspace(id);
    assert_eq!(store.get(id), Some("hell"));
    assert_eq!(store.caret(id), Some(4));

    let (_value, _caret, selection, _scroll_x, _scroll_y) = store.get_state(id).unwrap();
    assert_eq!(selection, None);
}

#[test]
fn typing_replaces_selection() {
    let mut store = InputValueStore::new();
    let id = InputId::from_raw(1);

    store.set(id, "hello".to_string());
    store.focus(id);
    store.move_caret_left(id, true);
    store.insert_text(id, "X");

    assert_eq!(store.get(id), Some("hellX"));
    assert_eq!(store.caret(id), Some("hellX".len()));
}

#[test]
fn delete_removes_next_char() {
    let mut store = InputValueStore::new();
    let id = InputId::from_raw(1);

    store.set(id, "abc".to_string());
    store.focus(id);
    store.move_caret_left(id, false);
    assert_eq!(store.caret(id), Some(2));

    store.delete(id);
    assert_eq!(store.get(id), Some("ab"));
    assert_eq!(store.caret(id), Some(2));
}

#[test]
fn delete_selection_wins_over_single_char_delete() {
    let mut store = InputValueStore::new();
    let id = InputId::from_raw(1);

    store.set(id, "abcd".to_string());
    store.focus(id);

    store.move_caret_left(id, true);
    store.move_caret_left(id, true);
    store.delete(id);

    assert_eq!(store.get(id), Some("ab"));
    assert_eq!(store.caret(id), Some(2));
}

#[test]
fn set_caret_supports_shift_extend_selection() {
    let mut store = InputValueStore::new();
    let id = InputId::from_raw(1);

    store.set(id, "hello".to_string());
    store.focus(id);

    store.set_caret(id, 2, false);
    let (_value, caret, selection, _scroll_x, _scroll_y) = store.get_state(id).unwrap();
    assert_eq!(caret, 2);
    assert_eq!(selection, None);

    store.set_caret(id, 4, true);
    let (_value, caret, selection, _scroll_x, _scroll_y) = store.get_state(id).unwrap();
    assert_eq!(caret, 4);
    assert_eq!(selection, Some(SelectionRange { start: 2, end: 4 }));

    store.set_caret(id, 1, false);
    let (_value, caret, selection, _scroll_x, _scroll_y) = store.get_state(id).unwrap();
    assert_eq!(caret, 1);
    assert_eq!(selection, None);
}

#[test]
fn caret_from_x_picks_nearest_boundary() {
    let value = "hello";
    let measure = |s: &str| s.chars().count() as f32 * 10.0;

    assert_eq!(caret_from_x(value, 0.0, measure), 0);
    assert_eq!(caret_from_x(value, 4.0, measure), 0);
    assert_eq!(caret_from_x(value, 6.0, measure), 1);
    assert_eq!(caret_from_x(value, 19.0, measure), 2);
    assert_eq!(caret_from_x(value, 999.0, measure), value.len());
}

#[test]
fn scroll_x_updates_only_when_caret_leaves_viewport() {
    let mut store = InputValueStore::new();
    let id = InputId::from_raw(1);

    let value = "x".repeat(100);
    store.set(id, value);
    store.focus(id);

    let available_w = 50.0;
    let text_w = 1000.0;

    store.set_caret(id, 0, false);
    store.update_scroll_for_caret(id, 0.0, text_w, available_w);
    let (_value, _caret, _selection, scroll, _scroll_y) = store.get_state(id).unwrap();
    assert_eq!(scroll, 0.0);

    store.set_caret(id, 100, false);
    store.update_scroll_for_caret(id, 1000.0, text_w, available_w);
    let (_value, _caret, _selection, scroll, _scroll_y) = store.get_state(id).unwrap();
    assert_eq!(scroll, 950.0);

    store.set_caret(id, 99, false);
    store.update_scroll_for_caret(id, 990.0, text_w, available_w);
    let (_value, _caret, _selection, scroll, _scroll_y) = store.get_state(id).unwrap();
    assert_eq!(scroll, 950.0);
}

#[test]
fn checked_mutators_return_changed() {
    let mut store = InputValueStore::new();
    let id = InputId::from_raw(1);

    assert!(!store.set_checked(id, false));
    assert!(!store.is_checked(id));

    assert!(store.set_checked(id, true));
    assert!(store.is_checked(id));

    assert!(!store.set_checked(id, true));
    assert!(store.is_checked(id));

    assert!(store.toggle_checked(id));
    assert!(!store.is_checked(id));
}
