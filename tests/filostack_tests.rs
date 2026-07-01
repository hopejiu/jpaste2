//! FiloStack integration tests.
//!
//! Tests queue push/pop, self-write guard, self-paste guard, auto-exit.

mod common;

use jpastev2::filostack::service::FiloStackService;
use jpastev2::util::hash::sha256_hex;

#[test]
fn test_queue_push_pop_cycle() {
    let mut svc = FiloStackService::new();
    svc.set_mode("queue");

    svc.push("first");
    svc.push("second");
    svc.push("third");

    assert_eq!(svc.queue_len(), 3);
    assert_eq!(svc.pop(), Some("first".into()));
    assert_eq!(svc.pop(), Some("second".into()));
    assert_eq!(svc.pop(), Some("third".into()));
    assert_eq!(svc.pop(), None);
}

#[test]
fn test_queue_items_returns_clone() {
    let mut svc = FiloStackService::new();
    svc.set_mode("queue");
    svc.push("a");
    svc.push("b");

    let items = svc.queue_items();
    assert_eq!(items, vec!["a".to_string(), "b".to_string()]);
}

#[test]
fn test_mode_change_clears_queue() {
    let mut svc = FiloStackService::new();
    svc.set_mode("queue");
    svc.push("a");
    svc.push("b");
    assert_eq!(svc.queue_len(), 2);

    svc.set_mode("normal");
    assert_eq!(svc.queue_len(), 0);

    svc.set_mode("queue");
    assert_eq!(svc.queue_len(), 0, "queue should still be empty");
}

#[test]
fn test_self_write_skipped() {
    let mut svc = FiloStackService::new();
    svc.set_mode("queue");

    let text = "secret";
    let hash = sha256_hex(text.trim());
    svc.mark_self_write(&hash);

    assert!(!svc.push(text), "self-write should be skipped");
    assert_eq!(svc.queue_len(), 0);
}

#[test]
fn test_self_paste_guard() {
    let mut svc = FiloStackService::new();
    svc.set_mode("queue");
    svc.push("guarded item");

    svc.mark_self_paste();
    assert_eq!(svc.pop(), None, "pop should return None during self-paste guard");

    // Normal pop should work after guard expires (we can't test real time,
    // but we can verify the logic by checking that pop works normally)
}

#[test]
fn test_auto_exit_on_image_tag() {
    let mut svc = FiloStackService::new();
    svc.set_mode("queue");
    svc.push("a");

    svc.auto_exit_if_needed(jpastev2::clipboard::capture::tag::IMAGE);
    assert_eq!(svc.mode(), "normal");
    assert_eq!(svc.queue_len(), 0);
}

#[test]
fn test_auto_exit_on_file_tag() {
    let mut svc = FiloStackService::new();
    svc.set_mode("queue");
    svc.push("a");

    svc.auto_exit_if_needed(jpastev2::clipboard::capture::tag::FILE);
    assert_eq!(svc.mode(), "normal");
}

#[test]
fn test_text_only_does_not_auto_exit() {
    let mut svc = FiloStackService::new();
    svc.set_mode("queue");
    svc.push("a");

    svc.auto_exit_if_needed(jpastev2::clipboard::capture::tag::TEXT);
    assert_eq!(svc.mode(), "queue");
    assert_eq!(svc.queue_len(), 1);
}

#[test]
fn test_normal_mode_no_op() {
    let mut svc = FiloStackService::new();
    // default mode is "normal"
    assert!(!svc.push("x"));
    assert_eq!(svc.pop(), None);
    assert_eq!(svc.handle_ctrl_v(), false);
}
