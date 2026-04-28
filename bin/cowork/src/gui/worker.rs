use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use winit::event_loop::EventLoopProxy;

use crate::gui::events::UiEvent;

pub struct WorkerPool {
    handles: Arc<Mutex<Vec<JoinHandle<()>>>>,
}

impl WorkerPool {
    pub fn new() -> Self {
        Self {
            handles: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn spawn_with_proxy<F>(&self, proxy: EventLoopProxy<UiEvent>, f: F)
    where
        F: FnOnce(EventLoopProxy<UiEvent>) + Send + 'static,
    {
        let handle = std::thread::spawn(move || f(proxy));
        self.track(handle);
    }

    pub fn spawn<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        let handle = std::thread::spawn(f);
        self.track(handle);
    }

    fn track(&self, handle: JoinHandle<()>) {
        let mut guard = self.handles.lock().expect("WorkerPool poisoned");
        guard.retain(|h| !h.is_finished());
        guard.push(handle);
    }
}

impl Default for WorkerPool {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WorkerPool {
    fn drop(&mut self) {
        let drained: Vec<JoinHandle<()>> = {
            let mut guard = self.handles.lock().expect("WorkerPool poisoned");
            std::mem::take(&mut *guard)
        };
        for handle in drained {
            let _ = handle.join();
        }
    }
}
