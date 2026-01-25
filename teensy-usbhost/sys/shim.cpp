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

// Wrapper class to track claim() calls
class LoggingMIDIDevice : public MIDIDevice {
public:
  LoggingMIDIDevice(USBHost &host) : MIDIDevice(host) {}

  virtual bool claim(Device_t *dev, int type, const uint8_t *descriptors,
                     uint32_t len) {
    bool result = MIDIDevice::claim(dev, type, descriptors, len);
    if (result) {
      rust_log_info("MIDI driver claimed device");
    }
    return result;
  }
};

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
// 1. Static Allocation (No Heap required, memory reserved at compile time)
USBHost myusb_static;
USBHub hub1_static(myusb_static);
// Don't construct MIDI device globally - do it explicitly after logging is
// ready
LoggingMIDIDevice *midi_static_ptr = nullptr;

// 2. Global Pointers (If you really want to use pointers)
USBHost *myusb = &myusb_static;
LoggingMIDIDevice *midi = nullptr;

extern "C" {

void cpp_init() {
  rust_log_info("USB Host: Initializing");

  // Ensure GPIO8 peripheral clock is enabled (required for USB host power on T4.1)
  // The Rust BSP only initializes GPIO1-4, not GPIO6-9
  CCM_CCGR3 |= 0xFFFFFFFF;

  // Create MIDI device driver (placement new into static storage)
  static uint8_t midi_storage[sizeof(LoggingMIDIDevice)]
      __attribute__((aligned(16)));
  midi_static_ptr = new (midi_storage) LoggingMIDIDevice(myusb_static);
  midi = midi_static_ptr;

  if (!midi) {
    rust_log_info("USB Host: ERROR - Failed to create MIDI driver!");
    return;
  }

  myusb->begin();
  rust_log_info("USB Host: Ready");
}
void cpp_task() {
  if (myusb) 
    myusb->Task();  
}
void cpp_usb_isr() {
  if (myusb)
    myusb->isr();
}

void cpp_send_note_on(uint8_t note, uint8_t velocity, uint8_t channel) {
  if (midi) {
    midi->sendNoteOn(note, velocity, channel);
  }
}

void cpp_send_note_off(uint8_t note, uint8_t velocity, uint8_t channel) {
  if (midi) {
    midi->sendNoteOff(note, velocity, channel);
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
  while ((rust_micros() - start) < ms * 1000) {
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
