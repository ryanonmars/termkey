pub fn char_count(value: &str) -> usize {
    value.chars().count()
}

pub fn move_left(cursor: &mut usize) {
    *cursor = cursor.saturating_sub(1);
}

pub fn move_right(cursor: &mut usize, value: &str) {
    *cursor = (*cursor + 1).min(char_count(value));
}

pub fn move_home(cursor: &mut usize) {
    *cursor = 0;
}

pub fn move_end(cursor: &mut usize, value: &str) {
    *cursor = char_count(value);
}

pub fn move_word_left(cursor: &mut usize, value: &str) {
    *cursor = previous_word_boundary(value, *cursor);
}

pub fn move_word_right(cursor: &mut usize, value: &str) {
    *cursor = next_word_boundary(value, *cursor);
}

pub fn insert_char(value: &mut String, cursor: &mut usize, c: char) {
    let byte_idx = char_to_byte_idx(value, *cursor);
    value.insert(byte_idx, c);
    *cursor += 1;
}

pub fn backspace(value: &mut String, cursor: &mut usize) {
    if *cursor == 0 {
        return;
    }

    let end = char_to_byte_idx(value, *cursor);
    let start = char_to_byte_idx(value, *cursor - 1);
    value.drain(start..end);
    *cursor -= 1;
}

pub fn delete(value: &mut String, cursor: &mut usize) {
    if *cursor >= char_count(value) {
        return;
    }

    let start = char_to_byte_idx(value, *cursor);
    let end = char_to_byte_idx(value, *cursor + 1);
    value.drain(start..end);
}

pub fn backspace_word(value: &mut String, cursor: &mut usize) {
    let end_char = (*cursor).min(char_count(value));
    let start_char = previous_word_boundary(value, end_char);
    if start_char == end_char {
        return;
    }

    let start = char_to_byte_idx(value, start_char);
    let end = char_to_byte_idx(value, end_char);
    value.drain(start..end);
    *cursor = start_char;
}

pub fn delete_word(value: &mut String, cursor: &mut usize) {
    let start_char = (*cursor).min(char_count(value));
    let end_char = next_word_boundary(value, start_char);
    if start_char == end_char {
        return;
    }

    let start = char_to_byte_idx(value, start_char);
    let end = char_to_byte_idx(value, end_char);
    value.drain(start..end);
}

pub fn cursor_segments(value: &str, cursor: usize) -> (&str, &str) {
    let byte_idx = char_to_byte_idx(value, cursor.min(char_count(value)));
    value.split_at(byte_idx)
}

fn char_to_byte_idx(value: &str, char_idx: usize) -> usize {
    value
        .char_indices()
        .map(|(idx, _)| idx)
        .nth(char_idx)
        .unwrap_or(value.len())
}

fn previous_word_boundary(value: &str, cursor: usize) -> usize {
    let chars: Vec<char> = value.chars().collect();
    let mut idx = cursor.min(chars.len());

    while idx > 0 && !is_word_char(chars[idx - 1]) {
        idx -= 1;
    }
    while idx > 0 && is_word_char(chars[idx - 1]) {
        idx -= 1;
    }

    idx
}

fn next_word_boundary(value: &str, cursor: usize) -> usize {
    let chars: Vec<char> = value.chars().collect();
    let mut idx = cursor.min(chars.len());

    while idx < chars.len() && !is_word_char(chars[idx]) {
        idx += 1;
    }
    while idx < chars.len() && is_word_char(chars[idx]) {
        idx += 1;
    }

    idx
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edits_middle_of_string() {
        let mut value = String::from("abcd");
        let mut cursor = 2;

        insert_char(&mut value, &mut cursor, 'X');
        assert_eq!(value, "abXcd");
        assert_eq!(cursor, 3);

        backspace(&mut value, &mut cursor);
        assert_eq!(value, "abcd");
        assert_eq!(cursor, 2);

        delete(&mut value, &mut cursor);
        assert_eq!(value, "abd");
        assert_eq!(cursor, 2);
    }

    #[test]
    fn splits_value_at_cursor() {
        let (before, after) = cursor_segments("hello", 2);
        assert_eq!(before, "he");
        assert_eq!(after, "llo");
    }

    #[test]
    fn moves_by_word_boundaries() {
        let value = "alpha beta gamma";
        let mut cursor = char_count(value);

        move_word_left(&mut cursor, value);
        assert_eq!(cursor, 11);

        move_word_left(&mut cursor, value);
        assert_eq!(cursor, 6);

        move_word_right(&mut cursor, value);
        assert_eq!(cursor, 10);
    }

    #[test]
    fn deletes_by_word_boundaries() {
        let mut value = String::from("alpha beta gamma");
        let mut cursor = 10;

        backspace_word(&mut value, &mut cursor);
        assert_eq!(value, "alpha  gamma");
        assert_eq!(cursor, 6);

        delete_word(&mut value, &mut cursor);
        assert_eq!(value, "alpha ");
        assert_eq!(cursor, 6);
    }
}
