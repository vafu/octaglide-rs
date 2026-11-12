#!/bin/sh
set -e

ELF_FILE=$1
HEX_FILE="${ELF_FILE%.elf}.hex"

flash() {
    teensy_loader_cli --mcu=TEENSY41 -w -v "${HEX_FILE}"
}

arm-none-eabi-objcopy -O ihex -R .init_array "${ELF_FILE}" "${HEX_FILE}"

flash || flash
