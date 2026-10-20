//! Spike S1/S2 (006 research): motherboard fans through PawnIO's LpcIO module (Nuvoton NCT67xx)
//! and the GPU fan through D3DKMT perf data. Read-only: it never writes a chip setting (it does not
//! clear the I/O space lock; it only reports it). Run elevated; prints to stdout.
#![allow(clippy::unwrap_used, clippy::expect_used)]

use std::ffi::{CStr, c_char, c_void};
use std::time::Duration;

use windows::Wdk::Graphics::Direct3D::{
    D3DKMT_ADAPTER_PERFDATA, D3DKMT_ADAPTERINFO, D3DKMT_ENUMADAPTERS2, D3DKMT_QUERYADAPTERINFO,
    D3DKMTEnumAdapters2, D3DKMTQueryAdapterInfo, KMTQAITYPE_ADAPTERPERFDATA,
};
use windows::Win32::Foundation::{CloseHandle, WAIT_ABANDONED, WAIT_OBJECT_0};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Threading::{CreateMutexW, ReleaseMutex, WaitForSingleObject};
use windows::core::{s, w};

type OpenFn = unsafe extern "system" fn(*mut *mut c_void) -> i32;
type LoadFn = unsafe extern "system" fn(*mut c_void, *const u8, usize) -> i32;
type ExecuteFn = unsafe extern "system" fn(
    *mut c_void,
    *const c_char,
    *const u64,
    usize,
    *mut u64,
    usize,
    *mut usize,
) -> i32;

struct Pawn {
    h: *mut c_void,
    exec: ExecuteFn,
}

impl Pawn {
    fn call(&self, f: &CStr, input: &[u64], out_len: usize) -> Result<Vec<u64>, i32> {
        let mut out = vec![0u64; out_len];
        let mut ret = 0usize;
        let hr = unsafe {
            (self.exec)(
                self.h,
                f.as_ptr(),
                input.as_ptr(),
                input.len(),
                out.as_mut_ptr(),
                out.len(),
                &mut ret,
            )
        };
        if hr < 0 { Err(hr) } else { Ok(out) }
    }
    fn outb(&self, port: u16, v: u8) {
        self.call(c"ioctl_pio_outb", &[port as u64, v as u64], 0)
            .unwrap();
    }
    fn inb(&self, port: u16) -> u8 {
        self.call(c"ioctl_pio_inb", &[port as u64], 1).unwrap()[0] as u8
    }
    fn sio_in(&self, reg: u8) -> u8 {
        self.call(c"ioctl_superio_inb", &[reg as u64], 1).unwrap()[0] as u8
    }
    fn sio_inw(&self, reg: u8) -> u16 {
        self.call(c"ioctl_superio_inw", &[reg as u64], 1).unwrap()[0] as u16
    }
}

fn main() {
    let module = std::env::args().nth(1).expect("path to LpcIO.bin");
    let blob = std::fs::read(&module).unwrap();
    let pawn = unsafe {
        let lib = LoadLibraryW(w!(r"C:\Program Files\PawnIO\PawnIOLib.dll")).unwrap();
        let open: OpenFn = std::mem::transmute(GetProcAddress(lib, s!("pawnio_open")).unwrap());
        let load: LoadFn = std::mem::transmute(GetProcAddress(lib, s!("pawnio_load")).unwrap());
        let exec: ExecuteFn =
            std::mem::transmute(GetProcAddress(lib, s!("pawnio_execute")).unwrap());
        let mut h = std::ptr::null_mut();
        assert!(open(&mut h) >= 0, "pawnio_open failed");
        let hr = load(h, blob.as_ptr(), blob.len());
        assert!(hr >= 0, "pawnio_load failed {hr:#x}");
        Pawn { h, exec }
    };

    let mutex =
        unsafe { CreateMutexW(None, false, w!(r"Global\Access_ISABUS.HTP.Method")) }.unwrap();
    let lock = || {
        let r = unsafe { WaitForSingleObject(mutex, 100) };
        assert!(
            r == WAIT_OBJECT_0 || r == WAIT_ABANDONED,
            "ISA bus mutex busy"
        );
    };
    let unlock = || unsafe { ReleaseMutex(mutex).unwrap() };

    // --- Detect ---
    let mut found = None;
    lock();
    for slot in 0u64..2 {
        pawn.call(c"ioctl_select_slot", &[slot], 0).unwrap();
        let port: u16 = if slot == 0 { 0x2e } else { 0x4e };
        pawn.outb(port, 0x87);
        pawn.outb(port, 0x87);
        let id = pawn.sio_in(0x20);
        let rev = pawn.sio_in(0x21);
        println!("slot {slot} (port {port:#x}): chip id {id:#04x} rev {rev:#04x}");
        if id == 0xD8 || id == 0xD4 || id == 0xD3 || id == 0xC8 {
            pawn.call(c"ioctl_find_bars", &[], 0).unwrap();
            // Select the hardware monitor logical device (0x0B).
            pawn.call(c"ioctl_superio_outb", &[0x07, 0x0B], 0).unwrap();
            let base = pawn.sio_inw(0x60);
            std::thread::sleep(Duration::from_millis(1));
            let verify = pawn.sio_inw(0x60);
            let io_lock = pawn.sio_in(0x28);
            println!(
                "  HWM base {base:#06x} (verify {verify:#06x}), reg 0x28 = {io_lock:#04x} (I/O space lock bit 4: {})",
                io_lock & 0x10 != 0
            );
            pawn.outb(port, 0xAA);
            found = Some(base);
            break;
        }
        pawn.outb(port, 0xAA);
    }
    unlock();

    // --- Fans ---
    if let Some(base) = found {
        let read = |addr: u16| -> u8 {
            pawn.outb(base + 5, 0x4E);
            pawn.outb(base + 6, (addr >> 8) as u8);
            pawn.outb(base + 5, (addr & 0xFF) as u8);
            pawn.inb(base + 6)
        };
        let count_regs = [0x4B0u16, 0x4B2, 0x4B4, 0x4B6, 0x4B8, 0x4BA, 0x4CC];
        let rpm_regs = [0x4C0u16, 0x4C2, 0x4C4, 0x4C6, 0x4C8, 0x4CA, 0x4CE];
        for second in 0..5 {
            lock();
            let counts: Vec<String> = count_regs
                .iter()
                .map(|&r| {
                    let (hi, lo) = (read(r) as u32, read(r + 1) as u32);
                    let count = (hi << 5) | (lo & 0x1F);
                    if (0x15..0x1FFF).contains(&count) {
                        format!("{:5.0}", 1.35e6 / count as f64)
                    } else {
                        format!("{:>5}", "0")
                    }
                })
                .collect();
            let rpms: Vec<String> = rpm_regs
                .iter()
                .map(|&r| format!("{:5}", ((read(r) as u16) << 8) | read(r + 1) as u16))
                .collect();
            unlock();
            println!(
                "t={second}s fans from count: [{}]  16-bit RPM regs: [{}]",
                counts.join(" "),
                rpms.join(" ")
            );
            std::thread::sleep(Duration::from_secs(1));
        }
    } else {
        println!("no Nuvoton chip found");
    }
    unsafe { CloseHandle(mutex).unwrap() };

    // --- GPU fan ---
    unsafe {
        let mut q = D3DKMT_ENUMADAPTERS2::default();
        D3DKMTEnumAdapters2(&mut q).unwrap();
        let mut a = vec![D3DKMT_ADAPTERINFO::default(); q.NumAdapters as usize];
        q.pAdapters = a.as_mut_ptr();
        D3DKMTEnumAdapters2(&mut q).unwrap();
        for info in &a[..q.NumAdapters as usize] {
            let mut p = D3DKMT_ADAPTER_PERFDATA::default();
            let mut qi = D3DKMT_QUERYADAPTERINFO {
                hAdapter: info.hAdapter,
                Type: KMTQAITYPE_ADAPTERPERFDATA,
                pPrivateDriverData: &mut p as *mut _ as *mut c_void,
                PrivateDriverDataSize: size_of::<D3DKMT_ADAPTER_PERFDATA>() as u32,
            };
            let ok = D3DKMTQueryAdapterInfo(&mut qi).is_ok();
            println!(
                "GPU adapter luid {:#x}: perfdata ok={ok} FanRPM={} Temperature={} (deci-C) Power={}",
                info.AdapterLuid.LowPart, p.FanRPM, p.Temperature, p.Power
            );
        }
    }
}
