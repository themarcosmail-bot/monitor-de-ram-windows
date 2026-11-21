use std::{cell::RefCell, mem, ptr};
use windows_sys::Win32::{
    Foundation::{
        CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM,
    },
    Graphics::Gdi::{COLOR_WINDOW, DEFAULT_GUI_FONT, GetStockObject, HDC, SetBkMode, TRANSPARENT},
    System::{
        LibraryLoader::GetModuleHandleW, SystemInformation::GetLocalTime, Threading::CreateMutexW,
    },
    UI::{Controls::EM_SETLIMITTEXT, WindowsAndMessaging::*},
};

use crate::{
    monitor::Monitor,
    settings::{MINUTES_MAX, Settings, THRESHOLD_MAX_MIB, THRESHOLD_MIN_MIB, parse_number},
    windows,
};

const TITLE: &str = "Monitor de RAM";
const AUTHOR: &str = "@themarcosmail-bot";
const TIMER: usize = 1;
const SAVE: usize = 101;
const CHECK: usize = 102;
const REFRESH: usize = 103;
const ADD: usize = 104;
const REMOVE: usize = 105;

struct App {
    window: HWND,
    settings: Settings,
    monitor: Monitor,
    candidates: Vec<String>,
    metrics: HWND,
    interval: HWND,
    threshold: HWND,
    automatic: HWND,
    available_apps: HWND,
    approved: HWND,
    status: HWND,
    action: HWND,
    hint: HWND,
}

pub fn run() {
    let lock = wide("Local\\MonitorDeRam-6a8cee38-e172-4263-a236-8cc7cf73b58a");
    let mutex = unsafe { CreateMutexW(ptr::null(), 0, lock.as_ptr()) };
    if mutex.is_null() {
        return;
    }
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        unsafe {
            CloseHandle(mutex);
            MessageBoxW(
                ptr::null_mut(),
                wide("O Monitor de RAM já está aberto.").as_ptr(),
                wide(TITLE).as_ptr(),
                MB_OK | MB_ICONINFORMATION,
            );
        }
        return;
    }
    run_window();
    unsafe { CloseHandle(mutex) };
}

fn run_window() {
    let instance = unsafe { GetModuleHandleW(ptr::null()) };
    let class = wide("MonitorDeRamWindow");
    let title = wide(TITLE);
    let cursor = unsafe { LoadCursorW(ptr::null_mut(), IDC_ARROW) };
    let icon = unsafe { LoadIconW(ptr::null_mut(), IDI_APPLICATION) };
    let definition = WNDCLASSW {
        lpfnWndProc: Some(window_proc),
        hInstance: instance,
        hCursor: cursor,
        hIcon: icon,
        hbrBackground: (COLOR_WINDOW + 1) as _,
        lpszClassName: class.as_ptr(),
        ..unsafe { mem::zeroed() }
    };
    let window = unsafe {
        if RegisterClassW(&definition) == 0 {
            return;
        }
        CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            654,
            530,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        )
    };
    if window.is_null() {
        return;
    }
    unsafe { ShowWindow(window, SW_SHOW) };
    let mut message: MSG = unsafe { mem::zeroed() };
    while unsafe { GetMessageW(&mut message, ptr::null_mut(), 0, 0) } > 0 {
        unsafe {
            if IsDialogMessageW(window, &message) == 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
}

unsafe extern "system" fn window_proc(
    window: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if message == WM_CTLCOLORSTATIC {
        unsafe { SetBkMode(wparam as HDC, TRANSPARENT as i32) };
        return (COLOR_WINDOW + 1) as LRESULT;
    }
    if message == WM_CREATE {
        let Some(mut app) = App::new(window) else {
            return -1;
        };
        app.refresh_apps();
        app.refresh_approved();
        app.show_reading(true);
        if !app.arm_timer() {
            put(
                app.hint,
                "O Windows não iniciou o timer. Use Verificar agora.",
            );
        }
        let app = Box::into_raw(Box::new(RefCell::new(app)));
        unsafe { SetWindowLongPtrW(window, GWLP_USERDATA, app as isize) };
        return 0;
    }
    let app = unsafe { GetWindowLongPtrW(window, GWLP_USERDATA) as *mut RefCell<App> };
    if message == WM_DESTROY {
        unsafe {
            KillTimer(window, TIMER);
            SetWindowLongPtrW(window, GWLP_USERDATA, 0);
            if !app.is_null() {
                drop(Box::from_raw(app));
            }
            PostQuitMessage(0);
        }
        return 0;
    }
    if !app.is_null()
        && let Ok(mut app) = unsafe { &*app }.try_borrow_mut()
    {
        if message == WM_TIMER && wparam == TIMER {
            app.show_reading(true);
            return 0;
        }
        if message == WM_COMMAND && wparam >> 16 == 0 {
            match wparam & 0xffff {
                SAVE => app.save_controls(),
                CHECK => app.show_reading(false),
                REFRESH => app.refresh_apps(),
                ADD => app.add_selected(),
                REMOVE => app.remove_selected(),
                _ => return unsafe { DefWindowProcW(window, message, wparam, lparam) },
            }
            return 0;
        }
    }
    unsafe { DefWindowProcW(window, message, wparam, lparam) }
}

impl App {
    fn new(window: HWND) -> Option<Self> {
        let settings = Settings::load();
        let monitor = Monitor::new();
        let instance = unsafe { GetModuleHandleW(ptr::null()) };
        let controls = Controls { window, instance };
        controls.label("Memória física", 20, 18, 585)?;
        let metrics = controls.label("", 20, 43, 585)?;
        controls.label("Verificar a cada", 20, 82, 110)?;
        let interval = controls.child(
            "EDIT",
            &settings.minutes.to_string(),
            WS_TABSTOP | ES_NUMBER as u32,
            WS_EX_CLIENTEDGE,
            [133, 78, 66, 25],
            0,
        )?;
        controls.label("minutos", 206, 82, 68)?;
        controls.label("Limite de RAM disponível", 282, 82, 175)?;
        let threshold = controls.child(
            "EDIT",
            &settings.threshold_mib.to_string(),
            WS_TABSTOP | ES_NUMBER as u32,
            WS_EX_CLIENTEDGE,
            [464, 78, 72, 25],
            0,
        )?;
        controls.label("MB", 544, 82, 44)?;
        let automatic = controls.child(
            "BUTTON",
            "Limpar automaticamente abaixo do limite",
            WS_TABSTOP | BS_AUTOCHECKBOX as u32,
            0,
            [20, 112, 320, 24],
            0,
        )?;
        send(automatic, BM_SETCHECK, usize::from(settings.automatic), 0);
        send(interval, EM_SETLIMITTEXT, 4, 0);
        send(threshold, EM_SETLIMITTEXT, 4, 0);
        controls.button("Aplicar", [375, 110, 100, 28], SAVE)?;
        controls.button("Verificar agora", [484, 110, 135, 28], CHECK)?;
        controls.label("Programas autorizados para limpeza", 20, 157, 552)?;
        let available_apps = controls.child(
            "COMBOBOX",
            "",
            WS_TABSTOP | WS_VSCROLL | CBS_DROPDOWNLIST as u32,
            0,
            [20, 183, 391, 172],
            0,
        )?;
        controls.button("Atualizar lista", [419, 181, 104, 27], REFRESH)?;
        controls.button("Autorizar", [532, 181, 87, 27], ADD)?;
        let approved = controls.child(
            "LISTBOX",
            "",
            WS_TABSTOP | WS_VSCROLL | LBS_NOINTEGRALHEIGHT as u32,
            WS_EX_CLIENTEDGE,
            [20, 218, 479, 89],
            0,
        )?;
        controls.button("Remover", [510, 219, 109, 27], REMOVE)?;
        controls.child(
            "STATIC",
            concat!(
                "A limpeza só atua em programas autorizados, inativos e sem janela visível em uso.\n",
                "Ao retomar o programa, o Windows pode recarregar páginas e causar uma breve demora."
            ),
            0, 0, [20, 316, 610, 42], 0,
        )?;
        let status = controls.child("STATIC", "", 0, 0, [20, 364, 610, 40], 0)?;
        let action = controls.child("STATIC", "", 0, 0, [20, 404, 610, 21], 0)?;
        let hint = controls.child("STATIC", "", 0, 0, [20, 428, 610, 23], 0)?;
        controls.label(&format!("Criado por {AUTHOR} · Licença MIT"), 20, 462, 610)?;
        Some(Self {
            window,
            settings,
            monitor,
            candidates: Vec::new(),
            metrics,
            interval,
            threshold,
            automatic,
            available_apps,
            approved,
            status,
            action,
            hint,
        })
    }

    fn arm_timer(&self) -> bool {
        unsafe { SetTimer(self.window, TIMER, self.settings.minutes * 60_000, None) != 0 }
    }

    fn show_reading(&mut self, scheduled: bool) {
        let reading = self.monitor.check(&self.settings, scheduled);
        if let Some(ram) = reading.ram {
            put(
                self.metrics,
                &format!(
                    "{} MB disponíveis de {} MB · {}% em uso",
                    ram.available_mib, ram.total_mib, ram.usage_pct
                ),
            );
        } else {
            put(self.metrics, "Leitura de memória indisponível");
        }
        let mut time = unsafe { mem::zeroed() };
        unsafe { GetLocalTime(&mut time) };
        put(
            self.status,
            &format!(
                "Última leitura às {:02}:{:02}:{:02} · {}",
                time.wHour, time.wMinute, time.wSecond, reading.detail
            ),
        );
        if let Some(action) = reading.action {
            put(self.action, &format!("Última limpeza · {}", action));
        }
    }

    fn save_controls(&mut self) {
        let Some(minutes) = parse_number(&get(self.interval), 1, MINUTES_MAX) else {
            put(self.hint, "Use um intervalo de 1 a 1440 minutos.");
            return;
        };
        let Some(threshold) =
            parse_number(&get(self.threshold), THRESHOLD_MIN_MIB, THRESHOLD_MAX_MIB)
        else {
            put(self.hint, "Use um limite de 256 a 4096 MB.");
            return;
        };
        let mut next = self.settings.clone();
        next.minutes = minutes;
        next.threshold_mib = threshold;
        next.automatic = send(self.automatic, BM_GETCHECK, 0, 0) == 1;
        if self.apply(next) {
            self.show_reading(true);
            if !self.arm_timer() {
                put(
                    self.hint,
                    "O Windows não atualizou o timer. Use Verificar agora.",
                );
            }
        }
    }

    fn apply(&mut self, settings: Settings) -> bool {
        if settings.save().is_err() {
            put(
                self.hint,
                "Não foi possível salvar. A configuração anterior segue em uso.",
            );
            return false;
        }
        self.settings = settings;
        self.monitor.reset_samples();
        put(self.hint, "Configuração salva.");
        true
    }

    fn refresh_apps(&mut self) {
        self.candidates = self
            .monitor
            .user
            .as_ref()
            .map(windows::selectable_apps)
            .unwrap_or_default();
        self.candidates
            .retain(|path| !self.settings.targets.contains(path));
        send(self.available_apps, CB_RESETCONTENT, 0, 0);
        for app in &self.candidates {
            add_item(
                self.available_apps,
                CB_ADDSTRING,
                &format!("{} — {}", windows::basename(app), app),
            );
        }
        if !self.candidates.is_empty() {
            send(self.available_apps, CB_SETCURSEL, 0, 0);
        }
    }

    fn refresh_approved(&self) {
        send(self.approved, LB_RESETCONTENT, 0, 0);
        for path in &self.settings.targets {
            add_item(
                self.approved,
                LB_ADDSTRING,
                &format!("{} — {}", windows::basename(path), path),
            );
        }
    }

    fn add_selected(&mut self) {
        let index = send(self.available_apps, CB_GETCURSEL, 0, 0);
        let Some(path) = usize::try_from(index)
            .ok()
            .and_then(|index| self.candidates.get(index))
        else {
            put(
                self.hint,
                "Abra o programa desejado e clique em Atualizar lista.",
            );
            return;
        };
        let mut next = self.settings.clone();
        if next.add_target(path) && self.apply(next) {
            self.refresh_approved();
            self.refresh_apps();
            self.show_reading(true);
        }
    }

    fn remove_selected(&mut self) {
        let index = send(self.approved, LB_GETCURSEL, 0, 0);
        let Some(index) = usize::try_from(index)
            .ok()
            .filter(|index| *index < self.settings.targets.len())
        else {
            put(self.hint, "Selecione um programa na lista de autorizados.");
            return;
        };
        let mut next = self.settings.clone();
        next.targets.remove(index);
        if self.apply(next) {
            self.refresh_approved();
            self.refresh_apps();
            self.show_reading(true);
        }
    }
}

struct Controls {
    window: HWND,
    instance: HINSTANCE,
}

impl Controls {
    fn child(
        &self,
        class: &str,
        label: &str,
        style: u32,
        extra: u32,
        rect: [i32; 4],
        id: usize,
    ) -> Option<HWND> {
        let class = wide(class);
        let label = wide(label);
        let [x, y, width, height] = rect;
        let control = unsafe {
            CreateWindowExW(
                extra,
                class.as_ptr(),
                label.as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                x,
                y,
                width,
                height,
                self.window,
                id as HMENU,
                self.instance,
                ptr::null(),
            )
        };
        if control.is_null() {
            return None;
        }
        let font = unsafe { GetStockObject(DEFAULT_GUI_FONT) };
        send(control, WM_SETFONT, font as usize, 1);
        Some(control)
    }

    fn label(&self, label: &str, x: i32, y: i32, width: i32) -> Option<HWND> {
        self.child("STATIC", label, 0, 0, [x, y, width, 21], 0)
    }

    fn button(&self, label: &str, rect: [i32; 4], id: usize) -> Option<HWND> {
        self.child(
            "BUTTON",
            label,
            WS_TABSTOP | BS_PUSHBUTTON as u32,
            0,
            rect,
            id,
        )
    }
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn send(window: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { SendMessageW(window, msg, wparam, lparam) }
}

fn add_item(window: HWND, message: u32, text: &str) {
    send(window, message, 0, wide(text).as_ptr() as LPARAM);
}

fn put(window: HWND, value: &str) {
    unsafe { SetWindowTextW(window, wide(value).as_ptr()) };
}

fn get(window: HWND) -> String {
    let mut buffer = [0u16; 16];
    let count = unsafe { GetWindowTextW(window, buffer.as_mut_ptr(), buffer.len() as i32) };
    String::from_utf16_lossy(&buffer[..count as usize])
}
