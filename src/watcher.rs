use anyhow::{Context, Result};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher as _};
use std::{
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, RecvTimeoutError},
    time::Duration,
};

pub struct Watcher {
    _watcher: RecommendedWatcher,
    events: Receiver<notify::Result<Event>>,
}

impl Watcher {
    pub fn new(root: &Path) -> Result<Self> {
        let (tx, rx) = mpsc::channel();

        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = tx.send(event);
        })
        .context("Failed to create filesystem watcher")?;

        watcher
            .watch(root, RecursiveMode::Recursive)
            .with_context(|| format!("Failed to watch {}", root.display()))?;

        Ok(Self {
            _watcher: watcher,
            events: rx,
        })
    }

    pub fn next_batch(&self, debounce: Duration) -> Option<Vec<PathBuf>> {
        let mut paths = Vec::new();
        collect(self.events.recv().ok()?, &mut paths);

        loop {
            match self.events.recv_timeout(debounce) {
                Ok(event) => collect(event, &mut paths),
                Err(RecvTimeoutError::Timeout) => break,
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }

        paths.sort();
        paths.dedup();
        Some(paths)
    }
}

fn collect(event: notify::Result<Event>, paths: &mut Vec<PathBuf>) {
    if let Ok(event) = event {
        paths.extend(event.paths);
    }
}
