use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::{process, thread, time::Duration};

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, mouse_event, MOUSE_EVENT_FLAGS, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    ClipCursor, GetCursorPos, GetSystemMetrics, SM_CXVIRTUALSCREEN, SM_XVIRTUALSCREEN,
};
use windows::Win32::Foundation::RECT;
use windows::Win32::System::Console::SetConsoleCtrlHandler;

// ── Supported hotkeys ──────────────────────────────────────────────────────
struct HotkeyEntry {
    name: &'static str,
    vk: i32,
}

const HOTKEYS: &[HotkeyEntry] = &[
    HotkeyEntry { name: "F1",       vk: 0x70 },
    HotkeyEntry { name: "F2",       vk: 0x71 },
    HotkeyEntry { name: "F3",       vk: 0x72 },
    HotkeyEntry { name: "F4",       vk: 0x73 },
    HotkeyEntry { name: "F5",       vk: 0x74 },
    HotkeyEntry { name: "F6",       vk: 0x75 },
    HotkeyEntry { name: "F7",       vk: 0x76 },
    HotkeyEntry { name: "F8",       vk: 0x77 },
    HotkeyEntry { name: "F9",       vk: 0x78 },
    HotkeyEntry { name: "F11",      vk: 0x7A },
    HotkeyEntry { name: "F12",      vk: 0x7B },
    HotkeyEntry { name: "CTRL",     vk: 0x11 },
    HotkeyEntry { name: "ALT",      vk: 0x12 },
    HotkeyEntry { name: "SHIFT",    vk: 0x10 },
    HotkeyEntry { name: "CAPSLOCK", vk: 0x14 },
    HotkeyEntry { name: "TAB",      vk: 0x09 },
    HotkeyEntry { name: "LCTRL",    vk: 0xA2 },
    HotkeyEntry { name: "RCTRL",    vk: 0xA3 },
    HotkeyEntry { name: "LALT",     vk: 0xA4 },
    HotkeyEntry { name: "RALT",     vk: 0xA5 },
    HotkeyEntry { name: "LSHIFT",   vk: 0xA0 },
    HotkeyEntry { name: "RSHIFT",   vk: 0xA1 },
    HotkeyEntry { name: "MOUSE4",   vk: 0x05 }, // XButton1
    HotkeyEntry { name: "MOUSE5",   vk: 0x06 }, // XButton2
    HotkeyEntry { name: "\\",       vk: 0xDC }, // OEM_5 (backslash)
];

// ── Helpers ────────────────────────────────────────────────────────────────

fn is_key_pressed(vk: i32) -> bool {
    // GetAsyncKeyState returns i16; bit 15 (0x8000) means currently pressed
    let state = unsafe { GetAsyncKeyState(vk) };
    state & (0x8000_u16 as i16) != 0
}

fn get_cursor_y() -> i32 {
    let mut pt = POINT { x: 0, y: 0 };
    unsafe { let _ = GetCursorPos(&mut pt); }
    pt.y
}

fn lock_y(y: i32) {
    let left  = unsafe { GetSystemMetrics(SM_XVIRTUALSCREEN) };
    let width = unsafe { GetSystemMetrics(SM_CXVIRTUALSCREEN) };
    let rect = RECT {
        left,
        top:    y,
        right:  left + width,
        bottom: y + 1, // 1-pixel strip
    };
    unsafe { let _ = ClipCursor(Some(&rect)); }
}

fn unlock_cursor() {
    unsafe { let _ = ClipCursor(None); }
}

fn mouse_down() {
    unsafe {
        mouse_event(MOUSEEVENTF_LEFTDOWN, 0, 0, 0, 0);
    }
}

fn mouse_up() {
    unsafe {
        mouse_event(MOUSEEVENTF_LEFTUP, 0, 0, 0, 0);
    }
}

// ── Main ───────────────────────────────────────────────────────────────────

fn main() {
    // Nice header
    println!();
    println!("  ╔══════════════════════════════════════╗");
    println!("  ║        🖱️  MOUSE Y-LOCK TOOL         ║");
    println!("  ╠══════════════════════════════════════╣");
    println!("  ║  Hold a hotkey to lock the mouse Y.  ║");
    println!("  ║  Release to unlock.  Press F10 to    ║");
    println!("  ║  exit the program.                   ║");
    println!("  ╚══════════════════════════════════════╝");
    println!();

    // Show available hotkeys
    println!("  Hotkey disponibili (F10 è riservato per Uscire):");
    println!("  ─────────────────────────────────────────");
    println!("   Funzione : F1 F2 F3 F4 F5 F6 F7 F8 F9 F11 F12");
    println!("   Modifier : CTRL  ALT  SHIFT  CAPSLOCK  TAB");
    println!("   Specifici: LCTRL RCTRL LALT RALT LSHIFT RSHIFT");
    println!("   Mouse    : MOUSE4  MOUSE5");
    println!("   Altro    : \\");
    println!("  ─────────────────────────────────────────");
    println!();

    // Ask for hotkey
    print!("  ▸ Inserisci hotkey: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let input = input.trim().to_uppercase();

    let hotkey = match HOTKEYS.iter().find(|h| h.name == input) {
        Some(h) => h,
        None => {
            eprintln!("  ✖ Hotkey \"{}\" non riconosciuto!", input);
            eprintln!("    Usa uno dei nomi elencati sopra.");
            process::exit(1);
        }
    };

    println!();
    println!("  ✔ Hotkey impostato: {}", hotkey.name);
    println!("  ℹ Tieni premuto [{}] per bloccare la Y del mouse.", hotkey.name);
    println!("  ℹ Premi F10 in qualsiasi momento per uscire dal programma.");
    println!();

    // Disable standard Ctrl+C
    unsafe {
        let _ = SetConsoleCtrlHandler(None, true);
    }

    // Handle F10 exit: make sure we unlock the cursor before exiting
    let running = Arc::new(AtomicBool::new(true));
    let r = running.clone();
    exit_handler(r);

    // Main loop
    let mut locked = false;

    while running.load(Ordering::Relaxed) {
        let pressed = is_key_pressed(hotkey.vk);

        if pressed && !locked {
            let y = get_cursor_y();
            lock_y(y);
            mouse_down();
            locked = true;
            println!("  🔒 Y bloccata a {} & Click Sinistro PREMUTO", y);
        } else if !pressed && locked {
            unlock_cursor();
            mouse_up();
            locked = false;
            println!("  🔓 Y sbloccata & Click Sinistro RILASCIATO");
        }

        // Poll at ~1 ms for fast response
        thread::sleep(Duration::from_millis(1));
    }

    // Cleanup
    unlock_cursor();
    mouse_up();
    println!("  👋 Uscita. Cursore sbloccato.");
}

fn exit_handler(running: Arc<AtomicBool>) {
    let _ = exit_setup(running);
}

/// We spawn a thread that watches for F10 via Windows API.
fn exit_setup(running: Arc<AtomicBool>) -> Result<(), ()> {
    // Unlock cursor and release mouse on any panic
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        unlock_cursor();
        mouse_up();
        original_hook(info);
    }));

    // Spawn a thread that waits for F10 (VK_F10)
    std::thread::spawn(move || {
        loop {
            // Check for F10 key (0x79)
            if is_key_pressed(0x79) {
                unlock_cursor();
                mouse_up();
                running.store(false, Ordering::Relaxed);
                println!("\n  👋 F10 premuto. Uscita...");
                std::thread::sleep(Duration::from_millis(100));
                process::exit(0);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });

    Ok(())
}
