//! 后台解码服务：主图预览与缩略图。
//!
//! 主图复用 `iv-image-decode::Loader`（单解码线程 + latest-wins 合并）；
//! 缩略图使用独立的小线程池，避免大量小图拖住当前图片的打开。

use std::collections::{HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use iced::widget::image::Handle;
use iv_image_decode::{Caps, LoadResponse, Loader, decode_thumbnail};

/// 缩略图最长边（逻辑 64 px，按 2x DPI 预留）。
pub const MAX_THUMB_SIDE: u32 = 128;
const THUMB_WORKERS: usize = 2;

pub struct PreviewService {
    loader: Loader,
}

impl Default for PreviewService {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewService {
    pub fn new() -> Self {
        Self {
            loader: Loader::new(|| {}),
        }
    }

    /// 请求打开图片；返回的代号用于丢弃过期结果。
    pub fn request(&mut self, path: PathBuf) -> u64 {
        self.loader.request(path, Caps::default())
    }

    pub fn cancel(&mut self) {
        self.loader.cancel_pending();
    }

    pub fn try_recv(&self) -> Option<LoadResponse> {
        self.loader.try_recv()
    }
}

pub struct ThumbResult {
    pub path: PathBuf,
    pub result: Result<Handle, String>,
}

#[derive(Debug)]
struct Queue {
    items: VecDeque<PathBuf>,
    closed: bool,
}

#[derive(Debug)]
pub struct ThumbnailService {
    queue: Arc<(Mutex<Queue>, Condvar)>,
    results: Receiver<ThumbResult>,
    inflight: HashSet<PathBuf>,
}

impl Default for ThumbnailService {
    fn default() -> Self {
        Self::new()
    }
}

impl ThumbnailService {
    pub fn new() -> Self {
        let queue = Arc::new((
            Mutex::new(Queue {
                items: VecDeque::new(),
                closed: false,
            }),
            Condvar::new(),
        ));
        let (sender, results) = mpsc::channel();

        for index in 0..THUMB_WORKERS {
            let queue = Arc::clone(&queue);
            let sender = sender.clone();
            std::thread::Builder::new()
                .name(format!("thumbnail-decoder-{index}"))
                .spawn(move || run_worker(&queue, &sender))
                .expect("无法启动缩略图解码线程");
        }

        Self {
            queue,
            results,
            inflight: HashSet::new(),
        }
    }

    /// 排入一个缩略图请求；已在队列中时返回 `false`。
    pub fn request(&mut self, path: PathBuf) -> bool {
        if !self.inflight.insert(path.clone()) {
            return false;
        }

        let (mutex, ready) = &*self.queue;
        let mut queue = lock_queue(mutex);
        queue.items.push_back(path);
        ready.notify_one();
        true
    }

    pub fn pending(&self) -> bool {
        !self.inflight.is_empty()
    }

    /// 某个路径是否已有请求在队列中或正在解码；视图层据此避免重复排队。
    pub fn is_inflight(&self, path: &Path) -> bool {
        self.inflight.contains(path)
    }

    pub fn try_recv(&mut self) -> Option<ThumbResult> {
        let result = self.results.try_recv().ok()?;
        self.inflight.remove(&result.path);
        Some(result)
    }
}

impl Drop for ThumbnailService {
    fn drop(&mut self) {
        let (mutex, ready) = &*self.queue;
        let mut queue = lock_queue(mutex);
        queue.closed = true;
        queue.items.clear();
        ready.notify_all();
    }
}

fn run_worker(queue: &Arc<(Mutex<Queue>, Condvar)>, sender: &Sender<ThumbResult>) {
    loop {
        let Some(path) = next_path(queue) else {
            return;
        };

        let result = decode_thumbnail(&path, MAX_THUMB_SIDE)
            .map(|decoded| {
                Handle::from_rgba(decoded.size[0] as u32, decoded.size[1] as u32, decoded.rgba)
            })
            .map_err(|error| error.to_string());

        if sender.send(ThumbResult { path, result }).is_err() {
            return;
        }
    }
}

fn next_path(queue: &Arc<(Mutex<Queue>, Condvar)>) -> Option<PathBuf> {
    let (mutex, ready) = &**queue;
    let mut queue = lock_queue(mutex);
    loop {
        if let Some(path) = queue.items.pop_front() {
            return Some(path);
        }
        if queue.closed {
            return None;
        }
        queue = ready
            .wait(queue)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
    }
}

fn lock_queue(mutex: &Mutex<Queue>) -> MutexGuard<'_, Queue> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
