//! What the process holds, where the platform tells cheaply.

#[derive(Debug, Clone, Copy)]
pub(super) struct Usage {
    pub(super) threads: usize,
    /// Open files on Linux. Windows only counts handles of every kind, each thread
    /// bringing some of its own.
    pub(super) handles: usize,
    pub(super) megabytes: u64,
}

/// Gives nothing where there is no `/proc`.
#[cfg(not(windows))]
pub(super) fn usage() -> Option<Usage> {
    let count = |dir| std::fs::read_dir(dir).ok().map(Iterator::count);
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    let kilobytes: u64 = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))?
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()?;
    Some(Usage {
        threads: count("/proc/self/task")?,
        handles: count("/proc/self/fd")?,
        megabytes: kilobytes / 1024,
    })
}

#[cfg(windows)]
pub(super) use windows::usage;

#[cfg(windows)]
mod windows {
    use std::ffi::c_void;

    use super::Usage;

    type Handle = *mut c_void;

    const INVALID_HANDLE_VALUE: Handle = -1isize as Handle;
    const TH32CS_SNAPTHREAD: u32 = 0x4;

    #[repr(C)]
    struct ThreadEntry {
        size: u32,
        usage: u32,
        thread_id: u32,
        owner_process_id: u32,
        base_priority: i32,
        delta_priority: i32,
        flags: u32,
    }

    #[repr(C)]
    #[derive(Default)]
    struct MemoryCounters {
        size: u32,
        page_fault_count: u32,
        peak_working_set: usize,
        working_set: usize,
        quota_peak_paged_pool: usize,
        quota_paged_pool: usize,
        quota_peak_non_paged_pool: usize,
        quota_non_paged_pool: usize,
        private_bytes: usize,
        peak_private_bytes: usize,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GetCurrentProcess() -> Handle;
        fn GetCurrentProcessId() -> u32;
        fn GetProcessHandleCount(process: Handle, count: *mut u32) -> i32;
        fn K32GetProcessMemoryInfo(
            process: Handle,
            counters: *mut MemoryCounters,
            size: u32,
        ) -> i32;
        fn CreateToolhelp32Snapshot(flags: u32, process_id: u32) -> Handle;
        fn Thread32First(snapshot: Handle, entry: *mut ThreadEntry) -> i32;
        fn Thread32Next(snapshot: Handle, entry: *mut ThreadEntry) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
    }

    struct Snapshot(Handle);

    impl Drop for Snapshot {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateToolhelp32Snapshot and is closed once.
            unsafe { CloseHandle(self.0) };
        }
    }

    pub(in super::super) fn usage() -> Option<Usage> {
        // SAFETY: the pseudo handle of the current process needs no closing.
        let process = unsafe { GetCurrentProcess() };
        let mut handles = 0;
        // SAFETY: `handles` is a valid place for the count to be written.
        if unsafe { GetProcessHandleCount(process, &mut handles) } == 0 {
            return None;
        }
        let size = u32::try_from(size_of::<MemoryCounters>()).ok()?;
        let mut memory = MemoryCounters {
            size,
            ..MemoryCounters::default()
        };
        // SAFETY: `memory` is as large as the size passed, which it also states itself.
        if unsafe { K32GetProcessMemoryInfo(process, &mut memory, size) } == 0 {
            return None;
        }
        Some(Usage {
            threads: threads()?,
            handles: usize::try_from(handles).ok()?,
            // The working set, as Linux's resident size, which leaves out memory committed
            // but never touched.
            megabytes: u64::try_from(memory.working_set).ok()? / (1024 * 1024),
        })
    }

    /// The snapshot lists every thread on the system, so those of other processes are
    /// skipped.
    fn threads() -> Option<usize> {
        // SAFETY: plain calls with no pointers; a failed snapshot is checked below.
        let (snapshot, process_id) = unsafe {
            (
                CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0),
                GetCurrentProcessId(),
            )
        };
        if snapshot == INVALID_HANDLE_VALUE {
            return None;
        }
        let snapshot = Snapshot(snapshot);
        let mut entry = ThreadEntry {
            size: u32::try_from(size_of::<ThreadEntry>()).ok()?,
            usage: 0,
            thread_id: 0,
            owner_process_id: 0,
            base_priority: 0,
            delta_priority: 0,
            flags: 0,
        };
        let mut count = 0;
        // SAFETY: the snapshot is open until `snapshot` drops, and `entry` states its
        // own size as both calls require.
        let mut more = unsafe { Thread32First(snapshot.0, &mut entry) } != 0;
        while more {
            if entry.owner_process_id == process_id {
                count += 1;
            }
            // SAFETY: as for Thread32First.
            more = unsafe { Thread32Next(snapshot.0, &mut entry) } != 0;
        }
        Some(count)
    }
}
