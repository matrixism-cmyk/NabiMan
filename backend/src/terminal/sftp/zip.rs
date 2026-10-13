//! A ZIP archive written front to back, so a folder can be sent while it is
//! still being read: no temporary file, no seeking back.
//!
//! Each file's sizes and checksum follow its data in a data descriptor, and
//! the central directory comes at the end as usual. ZIP64 records are used
//! where a file, an offset or the entry count outgrows the classic 32-bit
//! fields, so archives and files past 4 GiB open everywhere. Names are UTF-8
//! and flagged as such, which current Windows, macOS and Linux tools honour.

use chrono::{Datelike, Local, TimeZone, Timelike};

/// A file at least this large gets ZIP64 sizes from the start. It is under
/// 4 GiB by more than deflate could ever add to a file's size.
pub(crate) const BIG: u64 = 0xFF00_0000;
const MAX32: u64 = 0xFFFF_FFFF;
const UTF8: u16 = 1 << 11;
const DESCRIPTOR: u16 = 1 << 3;

struct Entry {
    name: String,
    offset: u64,
    crc: u32,
    compressed: u64,
    size: u64,
    method: u16,
    flags: u16,
    time: u16,
    date: u16,
    external: u32,
    big: bool,
}

#[derive(Default)]
pub(crate) struct ZipWriter {
    offset: u64,
    entries: Vec<Entry>,
}

fn put16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// MS-DOS time and date, as ZIP stores them: local time, two-second steps,
/// nothing before 1980.
pub(crate) fn dos_time(mtime: Option<u32>) -> (u16, u16) {
    let at = mtime
        .and_then(|t| Local.timestamp_opt(i64::from(t), 0).single())
        .filter(|t| t.year() >= 1980 && t.year() <= 2107);
    match at {
        Some(t) => (
            ((t.hour() as u16) << 11) | ((t.minute() as u16) << 5) | (t.second() as u16 / 2),
            (((t.year() - 1980) as u16) << 9) | ((t.month() as u16) << 5) | t.day() as u16,
        ),
        None => (0, (1 << 5) | 1),
    }
}

impl ZipWriter {
    /// Counts bytes of file data the caller sent after `begin_file`.
    pub fn wrote(&mut self, bytes: usize) {
        self.offset += bytes as u64;
    }

    fn local_header(&mut self, entry: Entry) -> Vec<u8> {
        let mut out = Vec::with_capacity(30 + entry.name.len() + 20);
        put32(&mut out, 0x0403_4b50);
        put16(&mut out, if entry.big { 45 } else { 20 });
        put16(&mut out, entry.flags);
        put16(&mut out, entry.method);
        put16(&mut out, entry.time);
        put16(&mut out, entry.date);
        put32(&mut out, 0); // checksum: in the descriptor, or none for a folder
        let unknown = if entry.big { MAX32 as u32 } else { 0 };
        put32(&mut out, unknown);
        put32(&mut out, unknown);
        put16(&mut out, entry.name.len() as u16);
        put16(&mut out, if entry.big { 20 } else { 0 });
        out.extend_from_slice(entry.name.as_bytes());
        if entry.big {
            put16(&mut out, 0x0001);
            put16(&mut out, 16);
            put64(&mut out, 0);
            put64(&mut out, 0);
        }
        self.offset += out.len() as u64;
        self.entries.push(entry);
        out
    }

    /// A folder entry, which keeps empty folders in the archive.
    pub fn dir(&mut self, name: &str, mtime: Option<u32>, mode: u32) -> Vec<u8> {
        let (time, date) = dos_time(mtime);
        let entry = Entry {
            name: format!("{}/", name.trim_end_matches('/')),
            offset: self.offset,
            crc: 0,
            compressed: 0,
            size: 0,
            method: 0,
            flags: UTF8,
            time,
            date,
            external: ((0o040000 | (mode & 0o7777)) << 16) | 0x10,
            big: false,
        };
        self.local_header(entry)
    }

    /// The header of a file whose data the caller sends next. `deflate`
    /// chooses compression; `big` must be set when the file may reach
    /// [`BIG`] bytes.
    pub fn begin_file(&mut self, name: &str, mtime: Option<u32>, mode: u32, deflate: bool, big: bool) -> Vec<u8> {
        let (time, date) = dos_time(mtime);
        let entry = Entry {
            name: name.to_string(),
            offset: self.offset,
            crc: 0,
            compressed: 0,
            size: 0,
            method: if deflate { 8 } else { 0 },
            flags: UTF8 | DESCRIPTOR,
            time,
            date,
            external: (0o100000 | (mode & 0o7777)) << 16,
            big,
        };
        self.local_header(entry)
    }

    /// The data descriptor that closes the current file.
    pub fn end_file(&mut self, crc: u32, compressed: u64, size: u64) -> std::io::Result<Vec<u8>> {
        let entry = self.entries.last_mut().ok_or_else(|| std::io::Error::other("no open file"))?;
        if !entry.big && (compressed > MAX32 || size > MAX32) {
            return Err(std::io::Error::other("a file grew past 4 GiB while it was being read"));
        }
        entry.crc = crc;
        entry.compressed = compressed;
        entry.size = size;
        let mut out = Vec::with_capacity(24);
        put32(&mut out, 0x0807_4b50);
        put32(&mut out, crc);
        if entry.big {
            put64(&mut out, compressed);
            put64(&mut out, size);
        } else {
            put32(&mut out, compressed as u32);
            put32(&mut out, size as u32);
        }
        self.offset += out.len() as u64;
        Ok(out)
    }

    /// The central directory and its end records.
    pub fn finish(self) -> Vec<u8> {
        let start = self.offset;
        let mut out = Vec::new();
        for e in &self.entries {
            let far = e.offset >= MAX32;
            let mut extra = Vec::new();
            if e.big {
                put64(&mut extra, e.size);
                put64(&mut extra, e.compressed);
            }
            if far {
                put64(&mut extra, e.offset);
            }
            put32(&mut out, 0x0201_4b50);
            put16(&mut out, (3 << 8) | 45); // made on Unix, so modes are kept
            put16(&mut out, if e.big || far { 45 } else { 20 });
            put16(&mut out, e.flags);
            put16(&mut out, e.method);
            put16(&mut out, e.time);
            put16(&mut out, e.date);
            put32(&mut out, e.crc);
            put32(&mut out, if e.big { MAX32 as u32 } else { e.compressed as u32 });
            put32(&mut out, if e.big { MAX32 as u32 } else { e.size as u32 });
            put16(&mut out, e.name.len() as u16);
            put16(&mut out, if extra.is_empty() { 0 } else { extra.len() as u16 + 4 });
            put16(&mut out, 0);
            put16(&mut out, 0);
            put16(&mut out, 0);
            put32(&mut out, e.external);
            put32(&mut out, if far { MAX32 as u32 } else { e.offset as u32 });
            out.extend_from_slice(e.name.as_bytes());
            if !extra.is_empty() {
                put16(&mut out, 0x0001);
                put16(&mut out, extra.len() as u16);
                out.extend_from_slice(&extra);
            }
        }
        let size = out.len() as u64;
        let count = self.entries.len() as u64;
        if count >= 0xFFFF || start >= MAX32 || size >= MAX32 {
            let record = start + size;
            put32(&mut out, 0x0606_4b50);
            put64(&mut out, 44);
            put16(&mut out, (3 << 8) | 45);
            put16(&mut out, 45);
            put32(&mut out, 0);
            put32(&mut out, 0);
            put64(&mut out, count);
            put64(&mut out, count);
            put64(&mut out, size);
            put64(&mut out, start);
            put32(&mut out, 0x0706_4b50);
            put32(&mut out, 0);
            put64(&mut out, record);
            put32(&mut out, 1);
        }
        put32(&mut out, 0x0605_4b50);
        put16(&mut out, 0);
        put16(&mut out, 0);
        put16(&mut out, count.min(0xFFFF) as u16);
        put16(&mut out, count.min(0xFFFF) as u16);
        put32(&mut out, size.min(MAX32) as u32);
        put32(&mut out, start.min(MAX32) as u32);
        put16(&mut out, 0);
        out
    }
}

/// Formats that are compressed already; deflating them again only costs time.
pub(crate) fn worth_compressing(name: &str) -> bool {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_ascii_lowercase()).unwrap_or_default();
    !matches!(
        ext.as_str(),
        "zip" | "gz" | "tgz" | "bz2" | "xz" | "zst" | "7z" | "rar" | "lz4" | "jpg" | "jpeg" | "png"
            | "gif" | "webp" | "avif" | "heic" | "mp4" | "mkv" | "mov" | "avi" | "webm" | "mp3" | "aac"
            | "ogg" | "flac" | "m4a" | "iso" | "pdf" | "docx" | "xlsx" | "pptx" | "jar" | "war" | "apk"
            | "deb" | "rpm" | "whl" | "woff" | "woff2" | "qcow2" | "vmdk"
    )
}

#[cfg(test)]
#[path = "zip_tests.rs"]
mod tests;
