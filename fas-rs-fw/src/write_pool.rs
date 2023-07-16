//! 并发写入池
//! 采用简化的线程池和负载设计，并且缓存重复路径
//! 适合控制器用来向节点写入频率

use std::{
    collections::HashMap,
    error::Error,
    fs::{self, set_permissions},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicUsize, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};

use likely_stable::{if_likely, LikelyOption};

use crate::{debug, this_unwrap::ThisResult};

pub struct WritePool {
    workers: Vec<(Sender<Command>, Arc<AtomicUsize>)>,
    cache_map: HashMap<PathBuf, (String, Instant)>,
}

enum Command {
    Write(PathBuf, String),
    Exit,
}

impl Drop for WritePool {
    fn drop(&mut self) {
        self.workers.iter().for_each(|(sender, _)| {
            let _ = sender.send(Command::Exit);
        });
    }
}

impl WritePool {
    /// 构造一个写入线程池
    ///
    #[must_use]
    pub fn new(worker_count: usize) -> Self {
        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let (sender, receiver) = mpsc::channel();
            let heavy = Arc::new(AtomicUsize::new(0));
            let heavy_clone = heavy.clone();

            thread::Builder::new()
                .name("WritePoolThread".into())
                .spawn(move || write_thread(&receiver, &heavy_clone))
                .this_unwrap();

            workers.push((sender, heavy));
        }

        Self {
            workers,
            cache_map: HashMap::with_capacity(worker_count),
        }
    }

    /// 异步写入一个值到指定路径
    ///
    /// # Errors
    /// 向线程池发送写入请求失败
    ///
    /// # Panics
    /// 线程池量为0
    pub fn write(&mut self, path: &Path, value: &str) -> Result<(), Box<dyn Error>> {
        debug! {
            println!("WritePool: write {} to {}", &value, &path.display());
        }

        if Some(value) == self.cache_map.get(path).map_likely(|(x, _)| x.as_str()) {
            return Ok(());
        }

        let (best_worker, heavy) = self
            .workers
            .iter()
            .min_by_key(|(_, heavy)| heavy.load(Ordering::Acquire))
            .unwrap();

        let new_heavy = heavy.load(Ordering::Acquire) + 1;
        heavy.store(new_heavy, Ordering::Release); // 完成一个任务负载计数加一

        best_worker.send(Command::Write(path.to_owned(), value.to_owned()))?;

        self.cache_map
            .insert(path.to_owned(), (value.to_owned(), Instant::now()));
        self.map_gc();

        Ok(())
    }

    fn map_gc(&mut self) {
        self.cache_map
            .retain(|_, (_, time)| (*time).elapsed() <= Duration::from_secs(5));
    }
}

#[allow(unused_variables)]
fn write_thread(receiver: &Receiver<Command>, heavy: &Arc<AtomicUsize>) {
    loop {
        if_likely! {
            let Ok(command) = receiver.recv() => {
                match command {
                    Command::Write(path, value) => {
                        set_permissions(&path, PermissionsExt::from_mode(0o644)).unwrap();
                        let _ = fs::write(&path, value);
                        set_permissions(&path, PermissionsExt::from_mode(0o444)).unwrap();
                    }
                    Command::Exit => return,
                }
            } else {
                return;
            }
        }
        let new_heavy = heavy.load(Ordering::Acquire).saturating_sub(1);
        heavy.store(new_heavy, Ordering::Release); // 完成一个任务负载计数器减一
    }
}
