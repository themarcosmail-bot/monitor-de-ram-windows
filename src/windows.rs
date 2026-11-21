use std::{collections::HashSet, mem, ptr, sync::OnceLock};
use windows_sys::{
    Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM},
        Security::{EqualSid, GetTokenInformation, TOKEN_QUERY, TOKEN_USER, TokenUser},
        System::{
            Diagnostics::ToolHelp::{
                CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW,
                TH32CS_SNAPPROCESS,
            },
            ProcessStatus::{K32EmptyWorkingSet, K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS},
            RemoteDesktop::ProcessIdToSessionId,
            SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
            Threading::{
                GetCurrentProcess, GetProcessTimes, OpenProcess, OpenProcessToken,
                PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SET_QUOTA, QueryFullProcessImageNameW,
            },
        },
        UI::WindowsAndMessaging::{
            EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowThreadProcessId,
            IsIconic, IsWindowVisible,
        },
    },
    core::BOOL,
};

pub const MIB: u64 = 1024 * 1024;

pub struct Ram {
    pub total_mib: u64,
    pub available_mib: u64,
    pub usage_pct: u32,
}

pub fn ram() -> Option<Ram> {
    let mut data: MEMORYSTATUSEX = unsafe { mem::zeroed() };
    data.dwLength = mem::size_of::<MEMORYSTATUSEX>() as u32;
    (unsafe { GlobalMemoryStatusEx(&mut data) } != 0).then_some(Ram {
        total_mib: data.ullTotalPhys / MIB,
        available_mib: data.ullAvailPhys / MIB,
        usage_pct: data.dwMemoryLoad,
    })
}

struct Handle(HANDLE);

impl Handle {
    fn new(raw: HANDLE) -> Option<Self> {
        (!raw.is_null() && raw != INVALID_HANDLE_VALUE).then_some(Self(raw))
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        unsafe { CloseHandle(self.0) };
    }
}

pub struct User(Vec<usize>);

impl User {
    pub fn current() -> Option<Self> {
        Self::for_process(unsafe { GetCurrentProcess() })
    }

    fn for_process(process: HANDLE) -> Option<Self> {
        let mut token = ptr::null_mut();
        if unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) } == 0 {
            return None;
        }
        let token = Handle::new(token)?;
        let mut required = 0;
        unsafe { GetTokenInformation(token.0, TokenUser, ptr::null_mut(), 0, &mut required) };
        if (required as usize) < mem::size_of::<TOKEN_USER>() {
            return None;
        }
        let mut data = vec![0usize; (required as usize).div_ceil(mem::size_of::<usize>())];
        let result = unsafe {
            GetTokenInformation(
                token.0,
                TokenUser,
                data.as_mut_ptr().cast(),
                required,
                &mut required,
            )
        };
        (result != 0).then_some(Self(data))
    }

    fn owns(&self, process: HANDLE) -> bool {
        let Some(other) = Self::for_process(process) else {
            return false;
        };
        let first = unsafe { &*self.0.as_ptr().cast::<TOKEN_USER>() };
        let second = unsafe { &*other.0.as_ptr().cast::<TOKEN_USER>() };
        unsafe { EqualSid(first.User.Sid, second.User.Sid) != 0 }
    }
}

pub struct Process(Handle);

impl Process {
    pub fn open(pid: u32, allow_trim: bool) -> Option<Self> {
        static CURRENT_SESSION: OnceLock<Option<u32>> = OnceLock::new();
        let own = (*CURRENT_SESSION.get_or_init(|| session(std::process::id())))?;
        if session(pid) != Some(own) {
            return None;
        }
        let quota = if allow_trim { PROCESS_SET_QUOTA } else { 0 };
        Handle::new(unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | quota, 0, pid) })
            .map(Self)
    }

    pub fn image(&self) -> Option<String> {
        let mut buffer = [0u16; 512];
        let read = |path: &mut [u16]| {
            let mut length = path.len() as u32;
            (unsafe { QueryFullProcessImageNameW(self.0.0, 0, path.as_mut_ptr(), &mut length) }
                != 0)
                .then_some(length as usize)
        };
        if let Some(length) = read(&mut buffer) {
            return String::from_utf16(&buffer[..length])
                .ok()
                .map(|image| image.to_lowercase());
        }
        let mut path = vec![0u16; 32768];
        let length = read(&mut path)?;
        String::from_utf16(&path[..length])
            .ok()
            .map(|image| image.to_lowercase())
    }

    pub fn is_owned_by(&self, user: &User) -> bool {
        user.owns(self.0.0)
    }

    pub fn times(&self) -> Option<Times> {
        let mut creation: FILETIME = unsafe { mem::zeroed() };
        let mut exit = creation;
        let mut kernel = creation;
        let mut user = creation;
        if unsafe { GetProcessTimes(self.0.0, &mut creation, &mut exit, &mut kernel, &mut user) }
            == 0
        {
            return None;
        }
        Some(Times {
            birth: ticks(creation),
            cpu: ticks(kernel) + ticks(user),
        })
    }

    pub fn resident_bytes(&self) -> Option<u64> {
        let mut data: PROCESS_MEMORY_COUNTERS = unsafe { mem::zeroed() };
        let size = mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
        data.cb = size;
        (unsafe { K32GetProcessMemoryInfo(self.0.0, &mut data, size) } != 0)
            .then_some(data.WorkingSetSize as u64)
    }

    pub fn trim(&self) -> bool {
        unsafe { K32EmptyWorkingSet(self.0.0) != 0 }
    }
}

fn session(pid: u32) -> Option<u32> {
    let mut id = 0;
    (unsafe { ProcessIdToSessionId(pid, &mut id) } != 0).then_some(id)
}

#[derive(Clone, Copy)]
pub struct Times {
    pub birth: u64,
    pub cpu: u64,
}

fn ticks(time: FILETIME) -> u64 {
    (u64::from(time.dwHighDateTime) << 32) | u64::from(time.dwLowDateTime)
}

pub fn processes() -> Vec<(u32, String)> {
    let Some(snapshot) = Handle::new(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) })
    else {
        return Vec::new();
    };
    let mut entry: PROCESSENTRY32W = unsafe { mem::zeroed() };
    entry.dwSize = mem::size_of::<PROCESSENTRY32W>() as u32;
    let mut entries = Vec::new();
    let mut ok = unsafe { Process32FirstW(snapshot.0, &mut entry) };
    while ok != 0 {
        let end = entry
            .szExeFile
            .iter()
            .position(|digit| *digit == 0)
            .unwrap_or(entry.szExeFile.len());
        if let Ok(name) = String::from_utf16(&entry.szExeFile[..end]) {
            entries.push((entry.th32ProcessID, name.to_lowercase()));
        }
        ok = unsafe { Process32NextW(snapshot.0, &mut entry) };
    }
    entries
}

struct Windows {
    all: HashSet<u32>,
    active: HashSet<u32>,
}

unsafe extern "system" fn read_window(window: HWND, data: LPARAM) -> BOOL {
    if unsafe { IsWindowVisible(window) == 0 } {
        return 1;
    }
    let windows = unsafe { &mut *(data as *mut Windows) };
    let mut pid = 0;
    unsafe { GetWindowThreadProcessId(window, &mut pid) };
    if pid != 0 {
        if unsafe { GetWindowTextLengthW(window) } > 0 {
            windows.all.insert(pid);
        }
        if unsafe { IsIconic(window) } == 0 {
            windows.active.insert(pid);
        }
    }
    1
}

fn windows() -> Option<Windows> {
    let mut windows = Windows {
        all: HashSet::new(),
        active: HashSet::new(),
    };
    if unsafe { EnumWindows(Some(read_window), (&mut windows as *mut Windows) as LPARAM) } == 0 {
        return None;
    }
    let foreground = unsafe { GetForegroundWindow() };
    if !foreground.is_null() {
        let mut pid = 0;
        unsafe { GetWindowThreadProcessId(foreground, &mut pid) };
        if pid != 0 {
            windows.active.insert(pid);
        }
    }
    Some(windows)
}

pub fn active_windows() -> Option<HashSet<u32>> {
    windows().map(|windows| windows.active)
}

pub fn basename(image: &str) -> &str {
    image.rsplit(['\\', '/']).next().unwrap_or(image)
}

pub fn allowed_path(image: &str) -> bool {
    let Ok(root) = std::env::var("SystemRoot") else {
        return false;
    };
    let root = root.trim_end_matches(['\\', '/']).to_lowercase() + "\\";
    let path = image.strip_prefix(r"\\?\").unwrap_or(image);
    std::path::Path::new(image).is_absolute() && image.ends_with(".exe") && !path.starts_with(&root)
}

pub fn selectable_apps(user: &User) -> Vec<String> {
    let Some(windows) = windows() else {
        return Vec::new();
    };
    let current = std::process::id();
    let mut apps = Vec::new();
    for pid in windows.all.into_iter().filter(|pid| *pid != current) {
        let Some(process) = Process::open(pid, false) else {
            continue;
        };
        if let Some(image) = process.image().filter(|image| allowed_path(image))
            && process.is_owned_by(user)
        {
            apps.push(image);
        }
    }
    apps.sort_unstable_by(|first, second| {
        basename(first)
            .cmp(basename(second))
            .then(first.cmp(second))
    });
    apps.dedup();
    apps
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn working_set_api_reduces_resident_pages_in_a_test_process() {
        let user = User::current().expect("current Windows user");
        let process = Process::open(std::process::id(), true).expect("own test process");
        assert!(process.is_owned_by(&user));
        let memory = ram().expect("physical memory statistics");
        assert!(memory.total_mib >= memory.available_mib);

        let mut payload = vec![0u8; 32 * MIB as usize];
        for page in payload.chunks_mut(4096) {
            page[0] = 1;
        }
        std::hint::black_box(&payload);
        let before = process.resident_bytes().expect("working set before");
        assert!(process.trim());
        let after = process.resident_bytes().expect("working set after");
        assert!(before.saturating_sub(after) >= 8 * MIB);
        assert_eq!(payload[0], 1);
    }
}
