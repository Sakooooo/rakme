use super::*;

fn editor() -> Editor {
    let font = Font::load(14.0).expect("a monospace font is needed for editor tests");
    Editor::new(font, 1024, 768, &[])
}

/// Pixel position just inside the first cell of `needle` in text `id`.
fn point_at(ed: &Editor, id: TextId, needle: &str) -> (i32, i32) {
    let t = ed.text(id).unwrap();
    let s = t.contents();
    let byte = s.find(needle).unwrap_or_else(|| panic!("{needle:?} not in {s:?}"));
    let idx = s[..byte].chars().count();
    let (x, y) = ed.xy_of(id, idx).expect("needle is visible");
    (x + 2, y + 2)
}

fn click(ed: &mut Editor, btn: u8, (x, y): (i32, i32)) {
    ed.mouse_press(btn, x, y);
    ed.mouse_release(btn, x, y);
}

fn only_win_in(ed: &Editor, ci: usize) -> usize {
    assert_eq!(ed.cols[ci].wins.len(), 1);
    ed.cols[ci].wins[0].id
}

#[test]
fn starts_with_two_columns_and_a_directory_window() {
    let ed = editor();
    assert_eq!(ed.cols.len(), 2);
    assert!(ed.cols[0].wins.is_empty());
    let w = &ed.cols[1].wins[0];
    assert!(w.is_dir);
    assert!(w.tag.contents().starts_with(&w.name));
    assert!(w.tag.contents().contains("| Look"));
}

#[test]
fn button2_on_column_tag_runs_new() {
    let mut ed = editor();
    let col = TextId::ColTag(ed.cols[0].id);
    let p = point_at(&ed, col, "New");
    click(&mut ed, 2, p);
    let id = only_win_in(&ed, 0);
    assert_eq!(ed.focus, TextId::Body(id));
    assert!(ed.win(id).unwrap().name.is_empty());
}

#[test]
fn typing_cut_paste_and_undo() {
    let mut ed = editor();
    let col = TextId::ColTag(ed.cols[0].id);
    let p = point_at(&ed, col, "New");
    click(&mut ed, 2, p);
    let id = only_win_in(&ed, 0);
    let body = TextId::Body(id);
    for c in "hello world".chars() {
        ed.key(Key::Char(c));
    }
    assert_eq!(ed.text(body).unwrap().contents(), "hello world");
    assert!(ed.win(id).unwrap().dirty());
    assert!(ed.win(id).unwrap().tag.contents().contains(" Put "));

    // Sweep "world" with button 1, then chord button 2 to cut it.
    let (x0, y0) = point_at(&ed, body, "world");
    let (x1, _) = point_at(&ed, body, "d");
    ed.mouse_press(1, x0, y0);
    ed.mouse_move(x1 + ed.font.adv, y0);
    assert_eq!(ed.text(body).unwrap().selection(), "world");
    ed.mouse_press(2, x1 + ed.font.adv, y0);
    ed.mouse_release(2, x1 + ed.font.adv, y0);
    ed.mouse_release(1, x1 + ed.font.adv, y0);
    assert_eq!(ed.text(body).unwrap().contents(), "hello ");
    assert_eq!(ed.snarf, "world");

    // Button 1 then button 3 pastes.
    let (x, y) = point_at(&ed, body, "hello");
    ed.mouse_press(1, x, y);
    ed.mouse_press(3, x, y);
    ed.mouse_release(3, x, y);
    ed.mouse_release(1, x, y);
    assert_eq!(ed.text(body).unwrap().contents(), "worldhello ");

    // Undo via the tag.
    let tag = TextId::Tag(id);
    let p = point_at(&ed, tag, "Undo");
    click(&mut ed, 2, p);
    assert_eq!(ed.text(body).unwrap().contents(), "hello ");
}

#[test]
fn button3_searches_forward_and_wraps() {
    let mut ed = editor();
    let col = TextId::ColTag(ed.cols[0].id);
    let p = point_at(&ed, col, "New");
    click(&mut ed, 2, p);
    let id = only_win_in(&ed, 0);
    let body = TextId::Body(id);
    ed.text_mut(body).unwrap().set_contents("foo bar\nfoo baz\n");
    let p = point_at(&ed, body, "foo");
    click(&mut ed, 3, p);
    let t = ed.text(body).unwrap();
    assert_eq!((t.q0, t.q1), (8, 11));
    // The pointer is warped onto the match; clicking there continues
    // the search, wrapping around to the first occurrence.
    let (wx, wy) = ed.warp.take().unwrap();
    click(&mut ed, 3, (wx, wy));
    let t = ed.text(body).unwrap();
    assert_eq!((t.q0, t.q1), (0, 3));
}

#[test]
fn put_get_and_del_warning() {
    let mut ed = editor();
    let dir = std::env::temp_dir().join(format!("rakme-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("note.txt");
    std::fs::write(&path, "one\n").unwrap();

    let id = ed.open_file(&path, 0);
    let body = TextId::Body(id);
    assert_eq!(ed.text(body).unwrap().contents(), "one\n");
    ed.focus = body;
    ed.text_mut(body).unwrap().set_select(4, 4);
    for c in "two\n".chars() {
        ed.key(Key::Char(c));
    }
    // Del on a dirty window is refused once and reported in +Errors.
    ed.execute(TextId::Tag(id), "Del");
    assert!(ed.find_win(id).is_some());
    let errs = ed.cols.iter().flat_map(|c| c.wins.iter()).find(|w| w.name.ends_with("+Errors")).unwrap();
    assert!(errs.body.contents().contains("file modified"));

    ed.execute(TextId::Tag(id), "Put");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "one\ntwo\n");
    assert!(!ed.win(id).unwrap().dirty());
    ed.execute(TextId::Tag(id), "Del");
    assert!(ed.find_win(id).is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
