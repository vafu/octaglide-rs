# ARM Toolchain Cheat Sheet

Quick reference for inspecting embedded ARM binaries (Teensy 4.x / Cortex-M7).

## Quick Commands

```bash
# Your binary path
BINARY="target/thumbv7em-none-eabihf/release/octaglide-rs"

# Overall size breakdown
arm-none-eabi-size -A $BINARY

# List all symbols
arm-none-eabi-nm --demangle $BINARY

# Show memory sections
arm-none-eabi-readelf -S $BINARY

# Disassemble code
arm-none-eabi-objdump -d $BINARY

# Find strings
arm-none-eabi-strings $BINARY
```

---

## 1. `arm-none-eabi-size` - Memory Usage

**What it does:** Shows how much Flash/RAM your program uses.

### Basic Usage

```bash
arm-none-eabi-size target/thumbv7em-none-eabihf/release/octaglide-rs
```

**Output:**
```
   text    data     bss     dec     hex filename
 172160     408   39808  212376   33d98 octaglide-rs
```

- `text` = Code + read-only data (lives in Flash)
- `data` = Initialized variables (copied Flash → RAM at boot)
- `bss` = Uninitialized variables (zero-filled RAM)
- `dec/hex` = Total size in decimal/hex

### Detailed View

```bash
arm-none-eabi-size -A target/thumbv7em-none-eabihf/release/octaglide-rs
```

Shows per-section breakdown:
```
section               size      addr
.boot                 8192  1610612736
.stack               16384   536870912
.text               123276           0
.rodata              23612   538968064
.data                  404   536887992
.bss                 23156   536891392
```

**Use this to:**
- Check if you're running out of Flash or RAM
- See which sections are eating memory

---

## 2. `arm-none-eabi-nm` - Symbol List

**What it does:** Lists all symbols (functions, variables, constants) in your binary.

### Basic Usage

```bash
arm-none-eabi-nm target/thumbv7em-none-eabihf/release/octaglide-rs
```

**Output:**
```
0000e106 T ACMP1
20005528 b _ZN12octaglide_rs3app4init7CHANNEL17h2653c35a7cfabd81E
```

**Format:** `ADDRESS TYPE NAME`

### Demangle Rust Names

```bash
arm-none-eabi-nm --demangle target/thumbv7em-none-eabihf/release/octaglide-rs
```

**Output:**
```
20005528 b octaglide_rs::app::init::CHANNEL
```

Much more readable!

### Symbol Types

| Type | Meaning | Where |
|------|---------|-------|
| `T` | **Code** (global function) | Flash (.text) |
| `t` | **Code** (static function) | Flash (.text) |
| `D` | **Initialized data** (global) | RAM (.data) |
| `d` | **Initialized data** (static) | RAM (.data) |
| `B` | **Uninitialized data** (global) | RAM (.bss) |
| `b` | **Uninitialized data** (static) | RAM (.bss) |
| `R` | **Read-only data** (global) | Flash (.rodata) |
| `r` | **Read-only data** (static) | Flash (.rodata) |
| `U` | **Undefined** (external reference) | - |
| `W` | **Weak** (can be overridden) | varies |

**Uppercase = global** (visible to other files)
**Lowercase = local** (static or file-scoped)

### Show Symbol Sizes

```bash
arm-none-eabi-nm --print-size --demangle target/thumbv7em-none-eabihf/release/octaglide-rs
```

**Output:**
```
20005000 00000400 b octaglide_rs::app::init_heap::HEAP_MEM
         ^^^^^^^^
         Size in hex (0x400 = 1024 bytes)
```

### Sort by Size (Largest Last)

```bash
arm-none-eabi-nm --size-sort --print-size --demangle target/thumbv7em-none-eabihf/release/octaglide-rs | tail -20
```

**Use this to:** Find memory hogs!

### Common Filters

```bash
# Find all RTIC channels
arm-none-eabi-nm --demangle $BINARY | grep CHANNEL

# Show only BSS (uninitialized RAM)
arm-none-eabi-nm --demangle $BINARY | grep ' [bB] '

# Show only code (functions)
arm-none-eabi-nm --demangle $BINARY | grep ' [Tt] '

# Find specific function
arm-none-eabi-nm --demangle $BINARY | grep 'AnimationEngine::tick'

# Calculate total BSS usage
arm-none-eabi-nm --print-size $BINARY | grep ' [bB] ' | \
    awk '{sum += strtonum("0x"$2)} END {print sum " bytes"}'
```

### Useful Flags

```bash
-C, --demangle           # Make Rust/C++ names readable
-S, --print-size         # Show symbol sizes
--size-sort              # Sort by size (smallest first)
-n, --numeric-sort       # Sort by address
-u, --undefined-only     # Show only undefined symbols
-g, --extern-only        # Show only global symbols
```

---

## 3. `arm-none-eabi-objdump` - Disassembler

**What it does:** Shows the actual assembly code your CPU executes.

### Disassemble Code

```bash
arm-none-eabi-objdump -d target/thumbv7em-none-eabihf/release/octaglide-rs | less
```

**Output:**
```
60001030 <Reset>:
60001030:   4812        ldr   r0, [pc, #72]
60001032:   f380 8808   msr   MSP, r0
60001036:   4812        ldr   r0, [pc, #72]
```

**Format:** `ADDRESS: OPCODE INSTRUCTION`

### Disassemble Specific Function

```bash
arm-none-eabi-objdump -d target/thumbv7em-none-eabihf/release/octaglide-rs | \
    grep -A 30 '<Reset>:'
```

### Show Source Code (If Debug Symbols Present)

```bash
arm-none-eabi-objdump -S target/thumbv7em-none-eabihf/release/octaglide-rs
```

Interleaves source code with assembly (only works with debug builds).

### Show Section Headers

```bash
arm-none-eabi-objdump -h target/thumbv7em-none-eabihf/release/octaglide-rs
```

**Output:**
```
Idx Name          Size      VMA       LMA       File off  Algn
  0 .boot         00002000  60000000  60000000  00010000  2**2
  4 .text         0001e18c  00000000  00000000  00030000  2**3
  7 .bss          00005a74  20005000  20005000  00065000  2**12
```

### Show All Headers

```bash
arm-none-eabi-objdump -x target/thumbv7em-none-eabihf/release/octaglide-rs
```

Complete dump of all metadata.

### Useful Flags

```bash
-d, --disassemble        # Disassemble executable sections
-S, --source             # Interleave source code
-h, --section-headers    # Show section headers
-x, --all-headers        # Show all headers
-t, --syms               # Show symbol table
```

---

## 4. `arm-none-eabi-readelf` - ELF Inspector

**What it does:** Shows detailed ELF (Executable and Linkable Format) information.

### Show Section Headers

```bash
arm-none-eabi-readelf -S target/thumbv7em-none-eabihf/release/octaglide-rs
```

**Output:**
```
Section Headers:
  [Nr] Name              Type            Addr     Off    Size   ES Flg Lk Inf Al
  [ 1] .boot             PROGBITS        60000000 010000 002000 00  AX  0   0  4
  [ 2] .stack            NOBITS          20000000 020000 004000 00   A  0   0  8
  [ 4] .text             PROGBITS        00000000 030000 01e18c 00  AX  0   0  8
  [ 5] .rodata           PROGBITS        20200000 050000 005c3c 00 AMS  0   0  8
  [ 6] .data             PROGBITS        200042b8 0642b8 000194 00  WA  0   0  4
  [ 7] .bss              NOBITS          20005000 065000 005a74 00  WA  0   0 4096
```

**Columns:**
- `Addr` = Where it lives in memory (VMA = Virtual Memory Address)
- `Off` = Offset in the file
- `Size` = Size in bytes (hex)
- `Flg` = Flags: `A` (alloc), `W` (write), `X` (execute)

### Show Program Headers

```bash
arm-none-eabi-readelf -l target/thumbv7em-none-eabihf/release/octaglide-rs
```

Shows how sections are loaded into memory.

### Show Symbol Table

```bash
arm-none-eabi-readelf -s target/thumbv7em-none-eabihf/release/octaglide-rs
```

Similar to `nm` but in different format.

### Show All Headers

```bash
arm-none-eabi-readelf -a target/thumbv7em-none-eabihf/release/octaglide-rs
```

Everything (very verbose).

### Useful Flags

```bash
-S, --section-headers    # Show section headers (most useful!)
-l, --program-headers    # Show program headers
-s, --symbols            # Show symbol table
-e, --headers            # Show all headers (ELF + section + program)
-a, --all                # Show everything
```

---

## 5. `arm-none-eabi-strings` - String Extractor

**What it does:** Finds readable ASCII strings embedded in the binary.

### Basic Usage

```bash
arm-none-eabi-strings target/thumbv7em-none-eabihf/release/octaglide-rs
```

**Output:**
```
error receiving Animator cmd
panicked at :
WouldBlockParseErrorBufferOverflow
```

### Search for Specific String

```bash
arm-none-eabi-strings target/thumbv7em-none-eabihf/release/octaglide-rs | grep -i panic
```

### Minimum String Length

```bash
arm-none-eabi-strings -n 10 target/thumbv7em-none-eabihf/release/octaglide-rs
```

Only show strings ≥10 characters.

### Useful Flags

```bash
-n <num>                 # Minimum string length (default: 4)
-t x                     # Show address in hex
-t d                     # Show address in decimal
```

---

## Common Debugging Workflows

### 1. "Is my binary too big?"

```bash
# Check overall size
arm-none-eabi-size -A target/thumbv7em-none-eabihf/release/octaglide-rs

# Find largest symbols
arm-none-eabi-nm --size-sort --print-size --demangle $BINARY | tail -20
```

**Teensy 4.1 limits:**
- Flash: 2MB
- DTCM RAM: 320KB
- OCRAM: 512KB

### 2. "Where are my RTIC channels?"

```bash
# Find channel storage
arm-none-eabi-nm --demangle --print-size $BINARY | grep CHANNEL

# Calculate total channel memory
arm-none-eabi-nm --print-size $BINARY | grep CHANNEL | \
    awk '{sum += strtonum("0x"$2)} END {print sum " bytes"}'
```

### 3. "What's eating my BSS?"

```bash
# Show all BSS symbols by size
arm-none-eabi-nm --size-sort --print-size --demangle $BINARY | \
    grep ' [bB] ' | tail -20
```

### 4. "Is a function getting inlined?"

```bash
# Search for function name
arm-none-eabi-nm --demangle $BINARY | grep 'function_name'

# If not found, it was inlined by optimizer
```

### 5. "Where is my function in memory?"

```bash
# Find function address
arm-none-eabi-nm --demangle $BINARY | grep 'function_name'

# Disassemble it
arm-none-eabi-objdump -d $BINARY | grep -A 20 '<function_name>:'
```

### 6. "What panic messages are in my binary?"

```bash
# Find panic strings
arm-none-eabi-strings $BINARY | grep -i panic

# Find all error messages
arm-none-eabi-strings $BINARY | grep -i error
```

---

## Quick Reference Table

| Want to... | Use |
|------------|-----|
| Check overall memory usage | `arm-none-eabi-size -A` |
| Find a function/variable | `arm-none-eabi-nm --demangle \| grep` |
| See largest symbols | `arm-none-eabi-nm --size-sort` |
| Find RTIC channels | `arm-none-eabi-nm \| grep CHANNEL` |
| Calculate BSS usage | `arm-none-eabi-nm \| grep ' [bB] ' \| awk` |
| See memory layout | `arm-none-eabi-readelf -S` |
| Disassemble a function | `arm-none-eabi-objdump -d \| grep -A 20` |
| Find panic strings | `arm-none-eabi-strings \| grep panic` |
| Check section sizes | `arm-none-eabi-readelf -S` |

---

## Aliases (Add to ~/.bashrc or ~/.zshrc)

```bash
# ARM toolchain shortcuts
alias anm='arm-none-eabi-nm --demangle'
alias asize='arm-none-eabi-size -A'
alias aobjdump='arm-none-eabi-objdump'
alias areadelf='arm-none-eabi-readelf'

# Project-specific
alias binsize='arm-none-eabi-size -A target/thumbv7em-none-eabihf/release/octaglide-rs'
alias channels='arm-none-eabi-nm --demangle --print-size target/thumbv7em-none-eabihf/release/octaglide-rs | grep CHANNEL'
```

---

## Further Reading

- **ELF Format**: https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
- **ARM Cortex-M7 Architecture**: https://developer.arm.com/Processors/Cortex-M7
- **GNU Binutils Docs**: https://sourceware.org/binutils/docs/

---

## Pro Tips

1. **Always use `--demangle`** with `nm` for Rust binaries - makes output readable
2. **Pipe to `less`** for long output: `arm-none-eabi-nm ... | less`
3. **Combine with `grep`, `awk`, `sort`** for powerful filtering
4. **Use `--print-size` with `nm`** to find memory hogs
5. **Check both `.bss` and `.data`** - both use RAM!
6. **Release builds inline aggressively** - functions may disappear from symbol table

---

**Created for:** OctaglideRS Embedded Project
**Target:** Teensy 4.1 (ARM Cortex-M7, thumbv7em-none-eabihf)
