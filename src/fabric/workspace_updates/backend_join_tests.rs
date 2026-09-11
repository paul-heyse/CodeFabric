//! Exercise real backend thread exit, including thread-local destruction after the loop returns.

use std::cell::RefCell;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use std::time::Duration;

use notify_debouncer_full::notify::{
    Config, PollWatcher, RecommendedWatcher, RecursiveMode, Watcher,
};

struct ExitProbe {
    entered: mpsc::Sender<()>,
    release: mpsc::Receiver<()>,
    finished: Arc<AtomicBool>,
}

impl Drop for ExitProbe {
    fn drop(&mut self) {
        let _ = self.entered.send(());
        let _ = self.release.recv();
        self.finished.store(true, Ordering::Release);
    }
}

thread_local! {
    static EXIT: RefCell<Option<ExitProbe>> = const { RefCell::new(None) };
}

#[test]
fn joined_backend_stop_waits_for_real_thread_exit_for_manual_timed_and_inotify_workers() {
    for backend in ["manual", "timed", "inotify"] {
        exercise(backend);
    }
}

fn exercise(backend: &str) {
    let fixture = tempfile::tempdir().unwrap();
    let (armed_tx, armed_rx) = mpsc::channel();
    let (exit_tx, exit_rx) = mpsc::channel();
    let (release_tx, release_rx) = mpsc::channel();
    let finished = Arc::new(AtomicBool::new(false));
    let mut gates = Some((exit_tx, release_rx, Arc::clone(&finished)));
    let handler = move |event: notify_debouncer_full::notify::Result<_>| {
        if event.is_ok()
            && let Some((entered, release, finished)) = gates.take()
        {
            EXIT.with(|slot| {
                *slot.borrow_mut() = Some(ExitProbe {
                    entered,
                    release,
                    finished,
                });
            });
            let _ = armed_tx.send(std::thread::current().id());
        }
    };
    let config = Config::default()
        .with_follow_symlinks(false)
        .with_join_on_drop(true);
    let watcher: Box<dyn Watcher + Send> = if backend == "inotify" {
        let mut watcher = RecommendedWatcher::new(handler, config).unwrap();
        watcher
            .watch(fixture.path(), RecursiveMode::NonRecursive)
            .unwrap();
        std::fs::write(fixture.path().join("created"), b"event").unwrap();
        Box::new(watcher)
    } else {
        let config = if backend == "manual" {
            config.with_manual_polling()
        } else {
            config.with_poll_interval(Duration::from_secs(3600))
        };
        let mut watcher = PollWatcher::new(handler, config).unwrap();
        watcher
            .watch(fixture.path(), RecursiveMode::NonRecursive)
            .unwrap();
        std::fs::write(fixture.path().join("created"), b"event").unwrap();
        watcher.poll().unwrap();
        Box::new(watcher)
    };
    let armed = armed_rx.recv_timeout(Duration::from_secs(5));
    if armed.is_err() {
        let _ = release_tx.send(());
        drop(watcher);
        panic!("{backend} callback did not arm exit probe: {armed:?}");
    }
    let (stopped_tx, stopped_rx) = mpsc::channel();
    let owner = std::thread::spawn(move || {
        drop(watcher);
        let _ = stopped_tx.send(());
    });
    let exiting = exit_rx.recv_timeout(Duration::from_secs(5));
    // The exit destructor is held, so joined shutdown cannot report completion yet.
    let premature = stopped_rx.recv_timeout(Duration::from_millis(50));
    // Always release before asserting, so a failing assertion cannot strand the native worker.
    let _ = release_tx.send(());
    owner.join().unwrap();
    assert_ne!(armed.unwrap(), std::thread::current().id());
    assert!(
        exiting.is_ok(),
        "{backend} stop must wake the worker's wait"
    );
    assert_eq!(
        premature,
        Err(mpsc::RecvTimeoutError::Timeout),
        "{backend} returned before actual thread exit"
    );
    assert!(
        finished.load(Ordering::Acquire),
        "{backend} exit destructor completed before owned stop returned"
    );
    stopped_rx.recv_timeout(Duration::from_secs(5)).unwrap();
}
