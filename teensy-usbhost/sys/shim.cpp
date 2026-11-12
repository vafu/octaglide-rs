// --- 1. MACRO CLOAKING ---
// Stub out interrupt management BEFORE including headers
// RTIC handles interrupts, not the C++ library
#define NVIC_ENABLE_IRQ(n)                                                     \
  do { /* RTIC handles this */                                                 \
  } while (0)
#define NVIC_DISABLE_IRQ(n)                                                    \
  do { /* RTIC handles this */                                                 \
  } while (0)

#include "core_pins.h"
#include <stddef.h>
#include <stdint.h>

// Expose private ISR (The Hack)
#define private public
#include "USBHost_t36.h"
#undef private

// Forward declare Rust logging function
extern "C" void rust_log_info(const char *msg);

// --- 3. MEMORY MANAGEMENT ---
static uint8_t my_heap[16384];
static size_t heap_idx = 0;

void *operator new(size_t size) {
  if (size % 8 != 0)
    size += 8 - (size % 8);
  if (heap_idx + size > sizeof(my_heap))
    return nullptr;
  void *ptr = &my_heap[heap_idx];
  heap_idx += size;
  return ptr;
}
void *operator new[](size_t size) { return operator new(size); }
void *operator new(size_t size, std::align_val_t al) {
  return operator new(size);
}
void operator delete(void *ptr) noexcept {}
void operator delete[](void *ptr) noexcept {}
void operator delete(void *ptr, size_t size) noexcept {}
void operator delete[](void *ptr, size_t size) noexcept {}

// --- 4. TIMEKEEPING ---
// Forward declare Rust time function
extern "C" uint32_t rust_micros();

// --- 5. EXPOSED API ---

// --- STATIC ALLOCATION ---
USBHost myusb_static;
// Don't construct MIDI device globally - do it explicitly in cpp_init()
MIDIDevice *midi_static_ptr = nullptr;

// Global pointers
USBHost *myusb = &myusb_static;
MIDIDevice *midi = nullptr;

extern "C" {

void cpp_init() {
  rust_log_info("USB Host: Initializing");
  // Rust startup does not run C++ global constructors. Construct each driver
  // explicitly, respecting the alignment required by its DMA descriptors.
  alignas(USBHub) static uint8_t hub_storage[sizeof(USBHub)];
  new (hub_storage) USBHub(myusb_static);
  alignas(MIDIDevice) static uint8_t midi_storage[sizeof(MIDIDevice)];
  midi_static_ptr = new (midi_storage) MIDIDevice(myusb_static);
  midi = midi_static_ptr;

  if (!midi) {
    rust_log_info("USB Host: ERROR - Failed to create MIDI driver!");
    return;
  }

  // The host power switch is GPIO_EMC_40. Arduino uses fast GPIO8, but
  // the Rust BSP uses the normal GPIO ports. Drive the currently selected bank.
  constexpr uint32_t power_bit = 1u << 26;
  IOMUXC_SW_MUX_CTL_PAD_GPIO_EMC_40 = 5;
  IOMUXC_SW_PAD_CTL_PAD_GPIO_EMC_40 = 0x0008;
  if (IOMUXC_GPR_GPR28 & power_bit) {
    GPIO8_DR_SET = power_bit;
    GPIO8_GDIR |= power_bit;
  } else {
    GPIO3_DR_SET = power_bit;
    GPIO3_GDIR |= power_bit;
  }
  rust_log_info("USB Host: VBUS power switch enabled");
  myusb->begin();
  rust_log_info("USB Host: Ready");
}
void cpp_task() {
  if (myusb)
    myusb->Task();
}
uint32_t cpp_host_port_status() { return USBHS_PORTSC1; }

void cpp_usb_isr() {
  if (myusb)
    myusb->isr();
}

void cpp_send_midi(const uint8_t *data, uint8_t len) {
  if (!midi || len == 0)
    return;

  // Parse status byte to determine MIDI message type
  uint8_t status = data[0];
  uint8_t msg_type = status & 0xF0;
  uint8_t channel = (status & 0x0F) + 1; // MIDIDevice uses 1-indexed channels

  // System real-time messages are single-byte and bypass running status.
  if (status >= 0xF8) {
    midi->sendRealTime(status);
    // Flush the driver's pending packet so clock is not held for its batch timer.
    midi->send_now();
    return;
  }

  switch (status) {
  case 0xF1: // Time Code Quarter Frame
    if (len >= 2) {
      midi->sendTimeCodeQuarterFrame((data[1] >> 4) & 0x07, data[1] & 0x0F);
    }
    return;
  case 0xF2: // Song Position (14-bit, LSB first)
    if (len >= 3) {
      midi->sendSongPosition(data[1] | (data[2] << 7));
    }
    return;
  case 0xF3: // Song Select
    if (len >= 2) {
      midi->sendSongSelect(data[1]);
    }
    return;
  case 0xF6: // Tune Request
    midi->sendTuneRequest();
    return;
  default:
    break;
  }

  switch (msg_type) {
  case 0x80: // Note Off
    if (len >= 3) {
      midi->sendNoteOff(data[1], data[2], channel);
    }
    break;
  case 0x90: // Note On
    if (len >= 3)
      midi->sendNoteOn(data[1], data[2], channel);
    break;
  case 0xA0: // Polyphonic Aftertouch
    if (len >= 3)
      midi->sendPolyPressure(data[1], data[2], channel);
    break;
  case 0xB0: // Control Change
    if (len >= 3)
      midi->sendControlChange(data[1], data[2], channel);
    break;
  case 0xC0: // Program Change
    if (len >= 2)
      midi->sendProgramChange(data[1], channel);
    break;
  case 0xD0: // Channel Aftertouch
    if (len >= 2)
      midi->sendAfterTouch(data[1], channel);
    break;
  case 0xE0: // Pitch Bend (14-bit value, LSB first)
    if (len >= 3) {
      int bend = (data[1] | (data[2] << 7)) - 8192;
      midi->sendPitchBend(bend, channel);
    }
    break;
  default:
    // Unsupported message type - ignore
    break;
  }
}

int cpp_midi_connected() {
  bool connected = (midi && *midi);
  static bool last_state = false;
  if (connected != last_state) {
    if (connected) {
      rust_log_info("USB MIDI: Device connected");
    } else {
      rust_log_info("USB MIDI: Device disconnected");
    }
    last_state = connected;
  }
  return connected ? 1 : 0;
}

void cpp_midi_get_device_info(uint16_t *vendor, uint16_t *product) {
  if (midi && *midi) {
    *vendor = midi->idVendor();
    *product = midi->idProduct();
  } else {
    *vendor = 0;
    *product = 0;
  }
}

} // extern "C"

// --- C RUNTIME STUBS ---

extern "C" {

uint32_t micros() { return rust_micros(); }

void delay(uint32_t ms) {
  uint64_t start = micros();
  while ((micros() - start) < ms * 1000) {
    asm("nop");
  }
}

// C. MOCK INTERRUPT VECTOR TABLE
// ehci.cpp tries to attach the USB Interrupt by writing directly to this array.
// Since we handle the Interrupt in Rust (#[task(binds = USB_OTG2)]),
// we just give C++ a dummy array to write to so it doesn't crash or fail
// linking. Size 256 is standard for Cortex-M vector tables.
static void (*volatile _VectorsRam[NVIC_NUM_INTERRUPTS + 16])(void) = {};

// D. RUNTIME STUBS
// In case we missed a pure virtual function somewhere
void __cxa_pure_virtual() {
  while (1)
    ;
}
}
