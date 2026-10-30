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
pub(super) use on_windows::usage;

#[cfg(windows)]
mod on_windows {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, GetProcessHandleCount,
    };

    use super::Usage;

    struct Snapshot(HANDLE);

    impl Drop for Snapshot {
        fn drop(&mut self) {
            // SAFETY: the handle came from CreateToolhelp32Snapshot and is closed once.
            let _ = unsafe { CloseHandle(self.0) };
        }
    }

    pub(in super::super) fn usage() -> Option<Usage> {
        // SAFETY: the pseudo handle of the current process needs no closing.
        let process = unsafe { GetCurrentProcess() };
        let mut handles = 0;
        // SAFETY: `handles` is a valid place for the count to be written.
        unsafe { GetProcessHandleCount(process, &mut handles) }.ok()?;
        let size = u32::try_from(size_of::<PROCESS_MEMORY_COUNTERS>()).ok()?;
        let mut memory = PROCESS_MEMORY_COUNTERS {
            cb: size,
            ..PROCESS_MEMORY_COUNTERS::default()
        };
        // SAFETY: `memory` is as large as the size passed, which it also states itself.
        if !unsafe { K32GetProcessMemoryInfo(process, &mut memory, size) }.as_bool() {
            return None;
        }
        Some(Usage {
            threads: threads()?,
            handles: usize::try_from(handles).ok()?,
            // The working set, as Linux's resident size, which leaves out memory committed
            // but never touched.
            megabytes: u64::try_from(memory.WorkingSetSize).ok()? / (1024 * 1024),
        })
    }

    /// The snapshot lists every thread on the system, so those of other processes are
    /// skipped.
    fn threads() -> Option<usize> {
        // SAFETY: plain calls with no pointers; a failed snapshot is an error.
        let (snapshot, process_id) = unsafe {
            (
                CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0).ok()?,
                GetCurrentProcessId(),
            )
        };
        let snapshot = Snapshot(snapshot);
        let mut entry = THREADENTRY32 {
            dwSize: u32::try_from(size_of::<THREADENTRY32>()).ok()?,
            ..THREADENTRY32::default()
        };
        let mut count = 0;
        // SAFETY: the snapshot is open until `snapshot` drops, and `entry` states its
        // own size as both calls require.
        let mut more = unsafe { Thread32First(snapshot.0, &mut entry) }.is_ok();
        while more {
            if entry.th32OwnerProcessID == process_id {
                count += 1;
            }
            // SAFETY: as for Thread32First.
            more = unsafe { Thread32Next(snapshot.0, &mut entry) }.is_ok();
        }
        Some(count)
    }
}
