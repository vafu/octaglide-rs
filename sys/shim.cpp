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

void cpp_configure_usb_power() {
  rust_log_info("EARLY: Configuring USB host power...");

  // Enable all necessary clocks
  // CCM_CCGR4 |= CCM_CCGR4_IOMUXC(CCM_CCGR_ON);
  // CCM_CCGR1 |= 0xFFFFFFFF;
  // CCM_CCGR2 |= 0xFFFFFFFF;
  // CCM_CCGR3 |= 0xFFFFFFFF;

  // Configure IOMUXC
  // IOMUXC_SW_MUX_CTL_PAD_GPIO_EMC_40 = 5;           // ALT5 = GPIO mode
  // IOMUXC_SW_PAD_CTL_PAD_GPIO_EMC_40 = 0x0008;      // Drive strength

  // Configure GPIO8
  GPIO8_GDIR |= 1 << 26;  // Output
  GPIO8_DR_SET = 1 << 26; // High

  // Small delay to let pin stabilize
  for (volatile int i = 0; i < 10000; i++) {
  }

  // Verify
  if ((GPIO8_GDIR & (1 << 26)) && (GPIO8_DR & (1 << 26))) {
    rust_log_info("EARLY: USB power configured and verified");
    delay(10000);
  } else {
    rust_log_info("EARLY: ERROR - USB power config failed");
  }
}

void cpp_verify_clocks() {
  uint32_t ccgr3 = CCM_CCGR3;
  rust_log_info("Pre-init: Verifying clock gates and pin config...");

  // Check specific GPIO8 clock gate field (bits 16-17 of CCGR3)
  uint32_t gpio8_cg = (ccgr3 >> 16) & 0x3; // CG8 = GPIO8

  if (gpio8_cg == 0x3) {
    rust_log_info("Pre-init: GPIO8 clock is ENABLED (0x3) - OK");
  } else if (gpio8_cg == 0x0) {
    rust_log_info("Pre-init: ERROR - GPIO8 clock is OFF (0x0)!");
  } else {
    rust_log_info("Pre-init: WARNING - GPIO8 clock in partial state!");
  }

  // Check IOMUXC configuration for GPIO_EMC_40
  uint32_t mux = IOMUXC_SW_MUX_CTL_PAD_GPIO_EMC_40;
  uint32_t pad = IOMUXC_SW_PAD_CTL_PAD_GPIO_EMC_40;

  if ((mux & 0x7) == 5) {
    rust_log_info("Pre-init: IOMUXC MUX = ALT5 (GPIO mode) - OK");
  } else {
    rust_log_info("Pre-init: ERROR - IOMUXC MUX NOT in GPIO mode!");
  }

  // Check PAD configuration
  if (pad == 0x0008) {
    rust_log_info("Pre-init: IOMUXC PAD = 0x0008 - OK");
  } else if (pad == 0) {
    rust_log_info("Pre-init: WARNING - IOMUXC PAD = 0x0000 (reset default)");
  } else {
    rust_log_info("Pre-init: WARNING - IOMUXC PAD has unexpected value");
  }

  // Check GPIO8 direction and data registers
  uint32_t gdir = GPIO8_GDIR;
  uint32_t dr = GPIO8_DR;

  if (gdir & (1 << 26)) {
    rust_log_info("Pre-init: GPIO8 direction = OUTPUT - OK");
  } else {
    rust_log_info("Pre-init: ERROR - GPIO8 direction = INPUT!");
  }

  if (dr & (1 << 26)) {
    rust_log_info("Pre-init: GPIO8 data register = HIGH");
  } else {
    rust_log_info("Pre-init: GPIO8 data register = LOW");
  }
}

void cpp_init() {
  rust_log_info("USB Host: Initializing");

  // CRITICAL: Ensure GPIO8 peripheral clock is enabled
  // The Rust BSP only initializes GPIO1-4, not GPIO6-9
  rust_log_info("USB Host: Pre-enabling GPIO8 clock");
  CCM_CCGR3 |= 0xFFFFFFFF; // Ensure GPIO6-9 clocks are ON

  // Create MIDI device driver NOW (after logging is ready)
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
  if (myusb) {
    myusb->Task();
  }
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

void cpp_debug_status() {
  uint32_t portsc = USBHS_PORTSC1;

  if (portsc & USBHS_PORTSC_CCS) {
    if (portsc & USBHS_PORTSC_PE) {
      rust_log_info("USB: Port enabled, device present");
    } else {
      rust_log_info("USB: Device present but port not enabled");
    }
  } else {
    rust_log_info("USB: No device connected");
  }
}

void cpp_recheck_power() {
// Re-verify and re-assert USB host power pin
#ifdef ARDUINO_TEENSY41
  // CRITICAL: Check if GPIO8 clock is still enabled
  uint32_t ccgr3 = CCM_CCGR3;
  if ((ccgr3 & 0xFFFF) != 0xFFFF) {
    rust_log_info("USB Power: CCGR3 clock gates disabled! Re-enabling...");
    CCM_CCGR3 |= 0xFFFFFFFF;
  }

  // Re-verify IOMUXC configuration hasn't been lost
  if ((IOMUXC_SW_MUX_CTL_PAD_GPIO_EMC_40 & 0x7) != 5) {
    rust_log_info("USB Power: IOMUXC mux lost, restoring...");
    IOMUXC_SW_MUX_CTL_PAD_GPIO_EMC_40 = 5;
    IOMUXC_SW_PAD_CTL_PAD_GPIO_EMC_40 = 0x0008;
  }

  // Re-verify GPIO direction
  if (!(GPIO8_GDIR & (1 << 26))) {
    rust_log_info("USB Power: GPIO direction lost, restoring...");
    GPIO8_GDIR |= 1 << 26;
  }

  // Re-assert power pin
  GPIO8_DR_SET = 1 << 26;

  // Read back to verify
  uint32_t dr = GPIO8_DR;
  if (dr & (1 << 26)) {
    rust_log_info("USB Power: GPIO still HIGH - OK");
  } else {
    rust_log_info("USB Power: ERROR - GPIO is LOW after re-assert!");
  }
#endif
}
}

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
