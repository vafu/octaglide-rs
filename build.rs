use std::env;

fn main() {
    let mut build = cc::Build::new();

    build
        .cpp(true)
        .include("sys") // Local mocks
        .include("sys/core") // Teensy Core
        .include("sys/USBHost_t36") // Library
        // Files
        .file("sys/shim.cpp")
        .file("sys/USBHost_t36/ehci.cpp") // Hardware Driver (Required)
        .file("sys/USBHost_t36/enumeration.cpp") // Core Logic (Required)
        .file("sys/USBHost_t36/hub.cpp") // Hub Support (Required)
        .file("sys/USBHost_t36/memory.cpp") // Memory Pipe Management (Required)
        .file("sys/USBHost_t36/print.cpp") // Debug Printing (Required)
        .file("sys/USBHost_t36/midi.cpp") // MIDI Driver (Required)
        // CRITICAL MACROS for Hardware Definitions
        .define("ARDUINO_TEENSY41", None) // T4.1 Board ID
        .define("TEENSYDUINO", "159")
        .define("ARDUINO", "10810")
        .define("F_CPU", "600000000")
        .flag("-D__IMXRT1062__") // Chip ID
        .flag("-DUSB_MIDI") // Enable MIDI mode
        // Compiler Flags
        .flag("-w") // Suppress all C++ warnings (Clean output)
        .flag("-fpermissive") // Allow loose typing (Essential for Arduino libs)
        .flag("-fno-rtti")
        .flag("-fno-exceptions")
        .flag("-mthumb")
        .flag("-mfloat-abi=hard")
        .flag("-mfpu=fpv5-d16")
        .cpp_link_stdlib(None)
        .compiler("arm-none-eabi-gcc");

    build.compile("usbhost");

    println!("cargo:rerun-if-changed=sys/shim.cpp");
}
