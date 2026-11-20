//! LMU's own shared memory "LMU_Data" (LMU 1.2+), opened read-only. Never created by us: if the game
//! isn't running there is nothing to open, and we try again every 2 seconds.
#![cfg(windows)]

use std::time::{Duration, Instant};

use windows::core::w;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Memory::{MapViewOfFile, OpenFileMappingW, UnmapViewOfFile, FILE_MAP_READ, MEMORY_MAPPED_VIEW_ADDRESS};

use super::layout::{LMUObjectOut, LAYOUT_SIZE};
use crate::source::{zeroed, Source};

pub struct SharedMemory {
    handle: Option<HANDLE>,
    view: Option<MEMORY_MAPPED_VIEW_ADDRESS>,
    next_try: Instant,
    last_et: f64,
    since: Instant,
}

impl SharedMemory {
    pub fn new() -> Self {
        SharedMemory { handle: None, view: None, next_try: Instant::now(), last_et: f64::NAN, since: Instant::now() }
    }
    fn open(&mut self) -> bool {
        if self.view.is_some() {
            return true;
        }
        if Instant::now() < self.next_try {
            return false;
        }
        self.next_try = Instant::now() + Duration::from_secs(2);
        unsafe {
            let Ok(h) = OpenFileMappingW(FILE_MAP_READ.0, false, w!("LMU_Data")) else { return false };
            let v = MapViewOfFile(h, FILE_MAP_READ, 0, 0, LAYOUT_SIZE);
            if v.Value.is_null() {
                let _ = CloseHandle(h);
                return false;
            }
            self.handle = Some(h);
            self.view = Some(v);
        }
        true
    }
    fn close(&mut self) {
        unsafe {
            if let Some(v) = self.view.take() {
                let _ = UnmapViewOfFile(v);
            }
            if let Some(h) = self.handle.take() {
                let _ = CloseHandle(h);
            }
        }
    }
}

impl Default for SharedMemory {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for SharedMemory {
    fn drop(&mut self) {
        self.close();
    }
}

// The mapping is read-only memory owned by Windows; we only copy out of it.
unsafe impl Send for SharedMemory {}

impl Source for SharedMemory {
    fn read(&mut self) -> Option<Box<LMUObjectOut>> {
        if !self.open() {
            return None;
        }
        let v = self.view?;
        let mut o = zeroed();
        unsafe {
            std::ptr::copy_nonoverlapping(v.Value as *const u8, (&mut *o) as *mut LMUObjectOut as *mut u8, LAYOUT_SIZE);
        }
        // When the game closes, our open view would keep its last data forever. If the game clock
        // hasn't moved for 5 s, let go of the view; if the game is still there, it reopens at once.
        let et = o.scoring.scoringInfo.mCurrentET;
        if et != self.last_et {
            self.last_et = et;
            self.since = Instant::now();
        } else if self.since.elapsed() > Duration::from_secs(5) {
            self.close();
            self.since = Instant::now();
            self.next_try = Instant::now();
            return None;
        }
        Some(o)
    }
}
