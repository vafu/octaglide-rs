// --- 1. MACRO CLOAKING ---
// Rename the Core's Serial objects to avoid conflict with our FakeSerial
#define Serial Serial_Real
#define Serial7 Serial7_Real

#include <stddef.h>
#include <stdint.h>

// Expose private ISR (The Hack)
#define private public
#include "Arduino.h" // Pulls in Print, Stream, HardwareSerial
#include "USBHost_t36.h"
#undef private

// Remove the cloaking so we can define the names we want
#undef Serial
#undef Serial7

// --- 2. FAKE SERIAL ---
// Must inherit from Print to work with USBHost functions taking Print&
class FakeSerial : public Print {
public:
  // Required by Print class
  virtual size_t write(uint8_t b) { return 1; }
  virtual size_t write(const uint8_t *buffer, size_t size) { return size; }

  // Extra methods used by libraries
  void begin(unsigned long baud) {}
  operator bool() { return true; }
};

// Define the instances USBHost is looking for
FakeSerial Serial;
FakeSerial Serial7;

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
extern "C" volatile uint32_t systick_millis_count = 0;
extern "C" void cpp_tick() { systick_millis_count++; }

// --- 5. EXPOSED API ---
// 1. Static Allocation (No Heap required, memory reserved at compile time)
USBHost myusb_static;
USBHub hub1_static(myusb_static);
MIDIDevice midi_static(myusb_static);

// 2. Global Pointers (If you really want to use pointers)
USBHost *myusb = &myusb_static;
MIDIDevice *midi = &midi_static;
extern "C" {

void cpp_init() { myusb->begin(); }
void cpp_task() {
  if (myusb)
    myusb->Task();
}
void cpp_usb_isr() {
  if (myusb)
    myusb->isr();
}

int cpp_read_midi(uint8_t *type, uint8_t *d1, uint8_t *d2) {
  if (midi && midi->read()) {
    *type = midi->getType();
    *d1 = midi->getData1();
    *d2 = midi->getData2();
    return 1;
  }
  return 0;
}
}

#define DWT_CYCCNT (*(volatile uint32_t *)0xE0001004)

extern "C" {

// B. IMPLEMENT TIMEKEEPING
// ehci.cpp uses this for USB timeouts.
uint32_t micros() {
  // 600 MHz clock = 600 cycles per microsecond
  return DWT_CYCCNT / 600;
}

void delay(uint32_t ms) {
  uint32_t start = micros();
  while (micros() - start < (ms * 1000)) {
    // Busy wait
    asm("nop");
  }
}

// C. MOCK INTERRUPT VECTOR TABLE
// ehci.cpp tries to attach the USB Interrupt by writing directly to this array.
// Since we handle the Interrupt in Rust (#[task(binds = USB_OTG2)]),
// we just give C++ a dummy array to write to so it doesn't crash or fail
// linking. Size 256 is standard for Cortex-M vector tables.
void (*volatile _VectorsRam[NVIC_NUM_INTERRUPTS + 16])(void);

// D. RUNTIME STUBS
// In case we missed a pure virtual function somewhere
void __cxa_pure_virtual() {
  while (1)
    ;
}
}
