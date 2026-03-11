use crate::selection::SelectionRange;
use crate::state::InputState;
use crate::text::clamp_to_char_boundary;

pub(super) fn selection_range(
    value: &str,
    anchor: Option<usize>,
    caret: usize,
) -> Option<SelectionRange> {
    let anchor = anchor?;

    let anchor = clamp_to_char_boundary(value, anchor);
    let caret = clamp_to_char_boundary(value, caret);
    if anchor == caret {
        return None;
    }

    Some(SelectionRange {
        start: anchor.min(caret),
        end: anchor.max(caret),
    })
}

pub(super) fn set_caret_in_state(state: &mut InputState, caret: usize, selecting: bool) {
    let caret = clamp_to_char_boundary(&state.value, caret);

    if selecting {
        if state.selection_anchor.is_none() {
            state.selection_anchor = Some(state.caret);
        }
        state.caret = caret;
        normalize_selection_anchor(state);
    } else {
        state.caret = caret;
        clear_selection(state);
    }
}

pub(super) fn normalize_selection_anchor(state: &mut InputState) {
    let Some(anchor) = state.selection_anchor else {
        return;
    };
    let anchor = clamp_to_char_boundary(&state.value, anchor);
    state.selection_anchor = Some(anchor);

    // If selection collapsed, clear anchor to avoid "sticky" selection.
    if anchor == state.caret {
        state.selection_anchor = None;
    }
}

pub(super) fn delete_selection_if_any(state: &mut InputState) -> bool {
    let Some(selection) = selection_range(&state.value, state.selection_anchor, state.caret) else {
        state.selection_anchor = None;
        state.caret = clamp_to_char_boundary(&state.value, state.caret);
        return false;
    };

    state.value.drain(selection.start..selection.end);
    state.caret = clamp_to_char_boundary(&state.value, selection.start);
    state.selection_anchor = None;
    mark_text_dirty(state);
    true
}

pub(super) fn clamp_state(state: &mut InputState) {
    state.caret = clamp_to_char_boundary(&state.value, state.caret);
    if let Some(anchor) = state.selection_anchor {
        state.selection_anchor = Some(clamp_to_char_boundary(&state.value, anchor));
    }
    state.scroll_x = state.scroll_x.max(0.0);
    state.scroll_y = state.scroll_y.max(0.0);
}

pub(super) fn clear_selection(state: &mut InputState) {
    state.selection_anchor = None;
}

pub(super) fn mark_text_dirty(state: &mut InputState) {
    state.value_rev = state.value_rev.wrapping_add(1);
}
