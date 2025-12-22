#!/bin/sh
set -e

ELF_FILE=$1
HEX_FILE="${ELF_FILE%.elf}.hex"

flash() {
    teensy_loader_cli --mcu=TEENSY41 -w -v "${HEX_FILE}"
}

cargo objcopy -- -O ihex -R .dmamem -R .bss -R .sbss -R .heap -R .stack -R .init_array "${HEX_FILE}"

flash || flash
