#pragma once
#include <stddef.h>
#include <stdint.h>

// --- Fix Missing Types for USBHost_t36 ---
typedef int oflag_t;
struct MbrSector_t {};
class FsBlockDeviceInterface {};

// --- Constants ---
#define O_READ 0
#define O_RDWR 1
#define O_CREAT 2
#define O_AT_END 4
#define FILE_WRITE 5
#define FILE_WRITE_BEGIN 6
#define T_CREATE 0
#define T_WRITE 1

// --- Macros ---
#define FS_SECOND(t) (t & 0)
#define FS_MINUTE(t) (t & 0)
#define FS_HOUR(t) (t & 0)
#define FS_DAY(d) (d & 0)
#define FS_MONTH(d) (d & 0)
#define FS_YEAR(d) (d & 0)

class FsFile {
public:
  operator bool() const { return false; }

  // Print/Stream methods
  size_t write(const void *buf, size_t size) { return 0; }
  int peek() { return -1; }
  int available() { return 0; }
  void flush() {}
  size_t read(void *buf, size_t nbyte) { return 0; }

  // Filesystem methods
  bool truncate(uint64_t size) { return false; }
  bool seekSet(uint64_t pos) { return false; }
  bool seekCur(uint64_t pos) { return false; }
  bool seekEnd(uint64_t pos) { return false; }
  uint64_t curPosition() { return 0; }
  uint64_t size() { return 0; }
  void close() {}
  bool isOpen() { return false; }
  bool getName(char *name, size_t len) { return false; }
  bool isDirectory() { return false; }

  // Directory iteration
  FsFile openNextFile() { return *this; }
  void rewindDirectory() {}

  // Time
  bool getCreateDateTime(uint16_t *pdate, uint16_t *ptime) { return false; }
  bool getModifyDateTime(uint16_t *pdate, uint16_t *ptime) { return false; }
  bool timestamp(uint8_t flags, uint16_t year, uint8_t month, uint8_t day,
                 uint8_t hour, uint8_t minute, uint8_t second) {
    return false;
  }
};

class FsVolume {
public:
  bool exists(const char *filepath) { return false; }
  bool mkdir(const char *filepath) { return false; }
  bool rename(const char *oldpath, const char *newpath) { return false; }
  bool remove(const char *filepath) { return false; }
  bool rmdir(const char *filepath) { return false; }
  bool getVolumeLabel(char *volume_label, size_t cb) { return false; }

  FsFile open(const char *filepath, uint8_t mode = 0) { return FsFile(); }

  uint32_t bytesPerCluster() { return 0; }
  uint32_t clusterCount() { return 0; }
  uint32_t freeClusterCount() { return 0; }
};

typedef FsVolume SdFileSystem;
