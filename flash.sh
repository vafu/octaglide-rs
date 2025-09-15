#!/bin/sh
set -e

ELF_FILE=$1
HEX_FILE="${ELF_FILE%.elf}.hex"

cargo objcopy  -- -O ihex  "${HEX_FILE}"
teensy_loader_cli --mcu=TEENSY40 -w -v "${HEX_FILE}"

