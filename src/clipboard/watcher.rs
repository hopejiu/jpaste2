//! Clipboard watcher using clipboard-rs.

use crate::clipboard::capture::{capture_current, CapturedData};
use clipboard_rs::{
    ClipboardContext, ClipboardHandler, ClipboardWatcher, ClipboardWatcherContext,
};
use crossbeam_channel::Sender;
use std::thread;
use std::time::Duration;

#[derive(Debug)]
pub enum ClipboardEvent {
    Captured(CapturedData),
}

pub struct WatcherContext {
    pub _ctx: ClipboardWatcherContext<ClipHandler>,
}

/// Start watching clipboard changes.
pub fn start_watcher(event_tx: Sender<ClipboardEvent>) -> WatcherContext {
    let mut ctx: ClipboardWatcherContext<ClipHandler> =
        ClipboardWatcherContext::new().expect("create clipboard watcher");

    let tx = event_tx.clone();
    ctx.add_handler(ClipHandler { tx });

    thread::Builder::new()
        .name("clipboard-watcher".into())
        .spawn(move || {
            ctx.start_watch();
        })
        .expect("spawn clipboard watcher thread");

    let keep_alive_ctx: ClipboardWatcherContext<ClipHandler> =
        ClipboardWatcherContext::new().expect("create clipboard watcher keepalive");
    WatcherContext { _ctx: keep_alive_ctx }
}

pub struct ClipHandler {
    pub tx: Sender<ClipboardEvent>,
}

impl ClipboardHandler for ClipHandler {
    fn on_clipboard_change(&mut self) {
        thread::sleep(Duration::from_millis(50));
        let clip = ClipboardContext::new().expect("create clipboard");
        if let Some(data) = capture_current(&clip) {
            let _ = self.tx.try_send(ClipboardEvent::Captured(data));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_event_send() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let data = CapturedData {
            primary_text: Some("test".into()),
            image_png: None,
            file_paths: vec![],
            content_hash: "abc".into(),
            tag_mask: 1,
            content_length: 4,
        };
        tx.send(ClipboardEvent::Captured(data)).unwrap();
        match rx.recv().unwrap() {
            ClipboardEvent::Captured(d) => assert_eq!(d.primary_text, Some("test".into())),
        }
    }
}
