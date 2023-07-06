mod parse;

use std::{
    fs::{self, set_permissions},
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicUsize, Ordering},
        mpsc::{self, Receiver},
        Arc,
    },
    thread::{self, JoinHandle},
};

use fas_rs_fw::prelude::*;

use super::IgnoreFrameTime;
use parse::{fps_thread, frametime_thread};

pub(crate) const FPSGO: &str = "/sys/kernel/fpsgo";

pub struct MtkFpsGo {
    // 数据量
    target_frametime_count: Arc<AtomicUsize>,
    fps_time_millis: Arc<AtomicU64>,
    // 数据
    frametime_receiver: Receiver<Vec<FrameTime>>,
    avg_fps: Arc<AtomicU32>,
    // 异常FrameTime忽略器
    ignore: IgnoreFrameTime,
    // 控制启停
    thread_handle: [JoinHandle<()>; 2],
    pause: Arc<AtomicBool>,
}

impl VirtualFrameSensor for MtkFpsGo {
    fn support() -> bool
    where
        Self: Sized,
    {
        Path::new(FPSGO).join("fbt/fbt_info").exists()
            && Path::new(FPSGO).join("common/fpsgo_enable").exists() // 检查路径
    }

    fn new() -> Result<Self, Box<dyn Error>>
    where
        Self: Sized,
    {
        // 控制启停的原子bool
        let pause = Arc::new(AtomicBool::new(false));

        let pause_frametime = pause.clone();
        let pause_fps = pause.clone();

        // 数据
        let (frametime_sender, frametime_receiver) = mpsc::sync_channel(1);
        let avg_fps = Arc::new(AtomicU32::new(0));

        // 数据量
        let target_frametime_count = Arc::new(AtomicUsize::new(0));
        let fps_time_millis = Arc::new(AtomicU64::new(0));

        let count_clone = target_frametime_count.clone();
        let time_clone = fps_time_millis.clone();
        let avg_fps_clone = avg_fps.clone();

        let thread_handle = [
            thread::spawn(move || {
                frametime_thread(&frametime_sender, &count_clone, &pause_frametime);
            }),
            thread::spawn(move || fps_thread(&avg_fps_clone, &time_clone, &pause_fps)),
        ];

        Ok(Self {
            frametime_receiver,
            avg_fps,
            target_frametime_count,
            fps_time_millis,
            ignore: IgnoreFrameTime::new(),
            pause,
            thread_handle,
        })
    }

    fn frametimes(&self, target_fps: TargetFps) -> Vec<FrameTime> {
        let data = self.frametime_receiver.recv().unwrap();
        data.into_iter()
            .map(|frametime| self.ignore.ign(frametime, target_fps))
            .collect()
    }

    fn fps(&self) -> Fps {
        self.avg_fps.load(Ordering::Acquire)
    }

    fn pause(&self) -> Result<(), Box<dyn Error>> {
        disable_fpsgo()?;

        self.pause.store(true, Ordering::Release);
        Ok(())
    }

    fn resume(&self, frametime_count: usize, fps_time: Duration) -> Result<(), Box<dyn Error>> {
        enable_fpsgo()?;

        self.pause.store(false, Ordering::Release);
        self.target_frametime_count
            .store(frametime_count, Ordering::Release);
        self.fps_time_millis
            .store(fps_time.as_millis().try_into().unwrap(), Ordering::Release);

        self.thread_handle
            .iter()
            .for_each(|handle| handle.thread().unpark());

        Ok(())
    }
}

pub(crate) fn enable_fpsgo() -> Result<(), std::io::Error> {
    let path = Path::new(FPSGO).join("common/fpsgo_enable");
    set_permissions(&path, PermissionsExt::from_mode(0o644))?;
    fs::write(&path, "1")?;
    set_permissions(&path, PermissionsExt::from_mode(0o444))
}

fn disable_fpsgo() -> Result<(), std::io::Error> {
    let path = Path::new(FPSGO).join("common/fpsgo_enable");
    set_permissions(&path, PermissionsExt::from_mode(0o644))?;
    fs::write(&path, "0")?;
    set_permissions(&path, PermissionsExt::from_mode(0o444))
}
