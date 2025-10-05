#!/bin/sh
set -e

ELF_FILE=$1
HEX_FILE="${ELF_FILE%.elf}.hex"

flash() {
    teensy_loader_cli --mcu=TEENSY40 -w -v "${HEX_FILE}"
}

cargo objcopy -- -O ihex "${HEX_FILE}"

flash || flash
