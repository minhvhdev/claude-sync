use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::time::Duration;
use std::thread;
use std::sync::Arc;
use log::{info, warn};

pub struct FileWatcher {
    watcher: Option<RecommendedWatcher>,
    stop_sender: Option<Sender<()>>,
}

impl FileWatcher {
    pub fn new() -> Self {
        Self {
            watcher: None,
            stop_sender: None,
        }
    }

    pub fn start<F>(&mut self, path: PathBuf, on_change: F) -> Result<(), String>
    where
        F: Fn() + Send + Sync + 'static,
    {
        if self.watcher.is_some() {
            warn!("Watcher already running");
            return Ok(());
        }

        let (tx, rx) = channel();
        let (stop_tx, stop_rx) = channel::<()>();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<notify::Event, notify::Error>| {
                if res.is_ok() {
                    let _ = tx.send(());
                }
            },
            notify::Config::default(),
        ).map_err(|e| e.to_string())?;

        watcher.watch(&path, RecursiveMode::Recursive)
            .map_err(|e| e.to_string())?;

        info!("File watcher started for {:?}", path);

        let on_change = Arc::new(on_change);
        let on_change_clone = on_change.clone();

        thread::spawn(move || {
            let mut last_change: Option<std::time::Instant> = None;
            let debounce_duration = Duration::from_secs(30);

            loop {
                if stop_rx.try_recv().is_ok() {
                    info!("File watcher stopped");
                    break;
                }

                match rx.recv_timeout(Duration::from_millis(500)) {
                    Ok(()) => {
                        last_change = Some(std::time::Instant::now());
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if let Some(last) = last_change {
                            if last.elapsed() > debounce_duration {
                                last_change = None;
                                info!("Debounce period elapsed, triggering sync");
                                on_change_clone();
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        break;
                    }
                }
            }
        });

        self.watcher = Some(watcher);
        self.stop_sender = Some(stop_tx);
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(sender) = self.stop_sender.take() {
            let _ = sender.send(());
        }
        self.watcher = None;
        info!("File watcher stopped");
    }

    pub fn _is_running(&self) -> bool {
        self.watcher.is_some()
    }
}

impl Drop for FileWatcher {
    fn drop(&mut self) {
        self.stop();
    }
}
