use core::sync::atomic::{AtomicU32, Ordering::Relaxed};

static HELD: [AtomicU32; 4] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];

pub fn press(note: u8) {
    HELD[note as usize / 32].fetch_or(1 << (note % 32), Relaxed);
}

pub fn release(note: u8) {
    HELD[note as usize / 32].fetch_and(!(1 << (note % 32)), Relaxed);
}

pub fn is_held(note: u8) -> bool {
    HELD[note as usize / 32].load(Relaxed) & (1 << (note % 32)) != 0
}
