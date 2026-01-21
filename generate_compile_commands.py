#!/usr/bin/env python3
import json
import os

# Base configuration from build.rs
project_root = os.getcwd()
compiler = "arm-none-eabi-g++"

cpp_files = [
    "sys/shim.cpp",
    "sys/USBHost_t36/ehci.cpp",
    "sys/USBHost_t36/enumeration.cpp",
    "sys/USBHost_t36/hub.cpp",
    "sys/USBHost_t36/memory.cpp",
    "sys/USBHost_t36/print.cpp",
    "sys/USBHost_t36/midi.cpp",
]

includes = [
    "-Isys",
    "-Isys/core",
    "-Isys/USBHost_t36",
]

defines = [
    "-DARDUINO_TEENSY41",
    "-DTEENSYDUINO=159",
    "-DARDUINO=10810",
    "-DF_CPU=600000000",
    "-D__IMXRT1062__",
    "-DUSB_MIDI",
]

flags = [
    "-w",
    "-fpermissive",
    "-fno-rtti",
    "-fno-exceptions",
    "-mthumb",
    "-mfloat-abi=hard",
    "-mfpu=fpv5-d16",
    "-std=c++11",
]

compile_commands = []

for source_file in cpp_files:
    file_path = os.path.join(project_root, source_file)
    output_file = source_file.replace(".cpp", ".o")
    
    command_parts = [compiler, "-c"] + defines + flags + includes + ["-o", output_file, source_file]
    command = " ".join(command_parts)
    
    entry = {
        "directory": project_root,
        "command": command,
        "file": source_file,
        "output": output_file
    }
    compile_commands.append(entry)

# Write to compile_commands.json
with open("compile_commands.json", "w") as f:
    json.dump(compile_commands, f, indent=2)

print(f"Generated compile_commands.json with {len(compile_commands)} entries")
