use super::*;

fn editor() -> Editor {
    let font = Font::load(14.0).expect("a monospace font is needed for editor tests");
    Editor::new(font, 1024, 768, &[])
}

/// Pixel position just inside the first cell of `needle` in text `id`.
fn point_at(editor: &Editor, id: TextId, needle: &str) -> (i32, i32) {
    let text = editor.text(id).unwrap();
    let contents = text.contents();
    let byte_idx = contents
        .find(needle)
        .unwrap_or_else(|| panic!("{needle:?} not in {contents:?}"));
    let char_idx = contents[..byte_idx].chars().count();
    let (x, y) = editor.xy_of(id, char_idx).expect("needle is visible");
    (x + 2, y + 2)
}

fn click(editor: &mut Editor, button: u8, (x, y): (i32, i32)) {
    editor.mouse_press(button, x, y);
    editor.mouse_release(button, x, y);
}

fn only_win_in(editor: &Editor, col_idx: usize) -> usize {
    assert_eq!(editor.columns[col_idx].windows.len(), 1);
    editor.columns[col_idx].windows[0].id
}

#[test]
fn starts_with_two_columns_and_a_directory_window() {
    let editor = editor();
    assert_eq!(editor.columns.len(), 2);
    assert!(editor.columns[0].windows.is_empty());
    let win = &editor.columns[1].windows[0];
    assert!(win.is_dir);
    assert!(win.tag.contents().starts_with(&win.name));
    assert!(win.tag.contents().contains("| Look"));
}

#[test]
fn button2_on_column_tag_runs_new() {
    let mut editor = editor();
    let col_tag = TextId::ColTag(editor.columns[0].id);
    let point = point_at(&editor, col_tag, "New");
    click(&mut editor, 2, point);
    let win_id = only_win_in(&editor, 0);
    assert_eq!(editor.focus, TextId::Body(win_id));
    assert!(editor.win(win_id).unwrap().name.is_empty());
}

#[test]
fn typing_cut_paste_and_undo() {
    let mut editor = editor();
    let col_tag = TextId::ColTag(editor.columns[0].id);
    let point = point_at(&editor, col_tag, "New");
    click(&mut editor, 2, point);
    let win_id = only_win_in(&editor, 0);
    let body = TextId::Body(win_id);
    for ch in "hello world".chars() {
        editor.key(Key::Char(ch));
    }
    assert_eq!(editor.text(body).unwrap().contents(), "hello world");
    assert!(editor.win(win_id).unwrap().dirty());
    assert!(editor.win(win_id).unwrap().tag.contents().contains(" Put "));

    // Sweep "world" with button 1, then chord button 2 to cut it.
    let (start_x, start_y) = point_at(&editor, body, "world");
    let (end_x, _) = point_at(&editor, body, "d");
    let end_x = end_x + editor.font.cell_width;
    editor.mouse_press(1, start_x, start_y);
    editor.mouse_move(end_x, start_y);
    assert_eq!(editor.text(body).unwrap().selection(), "world");
    editor.mouse_press(2, end_x, start_y);
    editor.mouse_release(2, end_x, start_y);
    editor.mouse_release(1, end_x, start_y);
    assert_eq!(editor.text(body).unwrap().contents(), "hello ");
    assert_eq!(editor.snarf, "world");

    // Button 1 then button 3 pastes.
    let (x, y) = point_at(&editor, body, "hello");
    editor.mouse_press(1, x, y);
    editor.mouse_press(3, x, y);
    editor.mouse_release(3, x, y);
    editor.mouse_release(1, x, y);
    assert_eq!(editor.text(body).unwrap().contents(), "worldhello ");

    // Undo via the tag.
    let tag = TextId::Tag(win_id);
    let point = point_at(&editor, tag, "Undo");
    click(&mut editor, 2, point);
    assert_eq!(editor.text(body).unwrap().contents(), "hello ");
}

#[test]
fn button3_searches_forward_and_wraps() {
    let mut editor = editor();
    let col_tag = TextId::ColTag(editor.columns[0].id);
    let point = point_at(&editor, col_tag, "New");
    click(&mut editor, 2, point);
    let win_id = only_win_in(&editor, 0);
    let body = TextId::Body(win_id);
    editor
        .text_mut(body)
        .unwrap()
        .set_contents("foo bar\nfoo baz\n");
    let point = point_at(&editor, body, "foo");
    click(&mut editor, 3, point);
    let text = editor.text(body).unwrap();
    assert_eq!((text.sel_start, text.sel_end), (8, 11));
    // The pointer is warped onto the match; clicking there continues
    // the search, wrapping around to the first occurrence.
    let (warp_x, warp_y) = editor.warp.take().unwrap();
    click(&mut editor, 3, (warp_x, warp_y));
    let text = editor.text(body).unwrap();
    assert_eq!((text.sel_start, text.sel_end), (0, 3));
}

#[test]
fn put_get_and_del_warning() {
    let mut editor = editor();
    let dir = std::env::temp_dir().join(format!("rakme-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("note.txt");
    std::fs::write(&path, "one\n").unwrap();

    let win_id = editor.open_file(&path, 0);
    let body = TextId::Body(win_id);
    assert_eq!(editor.text(body).unwrap().contents(), "one\n");
    editor.focus = body;
    editor.text_mut(body).unwrap().set_select(4, 4);
    for ch in "two\n".chars() {
        editor.key(Key::Char(ch));
    }
    // Del on a dirty window is refused once and reported in +Errors.
    editor.execute(TextId::Tag(win_id), "Del");
    assert!(editor.find_win(win_id).is_some());
    let errors_win = editor
        .columns
        .iter()
        .flat_map(|col| col.windows.iter())
        .find(|win| win.name.ends_with("+Errors"))
        .unwrap();
    assert!(errors_win.body.contents().contains("file modified"));

    editor.execute(TextId::Tag(win_id), "Put");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\ntwo\n");
    assert!(!editor.win(win_id).unwrap().dirty());
    editor.execute(TextId::Tag(win_id), "Del");
    assert!(editor.find_win(win_id).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
