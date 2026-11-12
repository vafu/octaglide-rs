//! Retained RAM log for the most recent panic.
//!
//! The buffer lives in the BSP's `.uninit` section, which is not zeroed at
//! startup. A warm reset therefore preserves the panic text until the next
//! boot can flush it over USB logging.

use core::{fmt::Write, mem::MaybeUninit};

// Keep the dump within imxrt-log's default 1024-byte transport buffer.
const TEXT_CAPACITY: usize = 768;
const MAGIC: u32 = 0x4f43_524c;

#[unsafe(link_section = ".uninit.crashlog")]
static mut STORAGE: MaybeUninit<Storage> = MaybeUninit::uninit();

#[repr(C)]
struct Storage {
    magic: u32,
    len: usize,
    text: [u8; TEXT_CAPACITY],
}

/// Clears the retained log after it has been flushed.
pub fn clear() {
    // SAFETY: this runs on the main thread before the log is shared.
    unsafe {
        let storage = core::ptr::addr_of_mut!(STORAGE).cast::<Storage>();
        (*storage).magic = 0;
    }
}

/// Writes the retained panic text, if present, through `log::error!`.
///
/// Returns whether a retained record was found.
pub fn flush() -> bool {
    let Some(text) = previous() else {
        return false;
    };
    log::error!("previous panic:\r\n{}", text);
    true
}

fn previous() -> Option<&'static str> {
    // SAFETY: the panic handler is the only writer, and it has finished before
    // this boot starts. The contents are validated before being exposed.
    unsafe {
        let storage = core::ptr::addr_of!(STORAGE).cast::<Storage>();
        if (*storage).magic != MAGIC || (*storage).len > TEXT_CAPACITY {
            return None;
        }
        let text = core::ptr::addr_of!((*storage).text) as *const u8;
        let bytes = core::slice::from_raw_parts(text, (*storage).len);
        core::str::from_utf8(bytes).ok()
    }
}

struct Writer;

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        // SAFETY: called only from the panic handler, after interrupts are off.
        unsafe {
            let storage = core::ptr::addr_of_mut!(STORAGE).cast::<Storage>();
            let len = if (*storage).magic == MAGIC {
                (*storage).len
            } else {
                (*storage).magic = MAGIC;
                (*storage).len = 0;
                0
            };
            let bytes = s.as_bytes();
            let copied = bytes.len().min(TEXT_CAPACITY - len);
            let text = core::ptr::addr_of_mut!((*storage).text) as *mut u8;
            core::ptr::copy_nonoverlapping(bytes.as_ptr(), text.add(len), copied);
            (*storage).len = len + copied;
        }
        Ok(())
    }
}

/// Records a panic message before the USB log has a chance to drain.
pub fn panic(info: &core::panic::PanicInfo<'_>) {
    let mut writer = Writer;
    if let Some(location) = info.location() {
        let _ = write!(
            writer,
            "panicked at {}:{}:{}: {}\r\n",
            location.file(),
            location.line(),
            location.column(),
            info.message()
        );
    } else {
        let _ = write!(writer, "panicked: {}\r\n", info.message());
    }
}
