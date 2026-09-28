//! 1:1 Unit QA tests for high-level ELF crash metadata extraction.

use sentry_driver::coredump::elf_extractor::{
    extract_elf_crash_headers, extract_elf_crash_headers_from_file,
};
use sentry_driver::coredump::lz4_flex;
use std::fs;
use std::io::{Cursor, Write};
use tempfile::tempdir;

const SYNTHETIC_ZSTD_CORE: &[u8] = &[
    40, 181, 47, 253, 4, 88, 149, 6, 0, 130, 136, 30, 41, 128, 197, 173, 6, 115, 9, 77, 44,
    118, 64, 163, 52, 172, 49, 134, 167, 126, 115, 116, 129, 183, 70, 21, 133, 182, 116, 5,
    165, 144, 204, 224, 18, 224, 66, 40, 221, 182, 108, 55, 153, 2, 255, 131, 232, 194, 162,
    170, 90, 43, 86, 107, 101, 48, 147, 16, 222, 72, 116, 154, 191, 253, 31, 0, 38, 19, 240,
    11, 122, 160, 127, 229, 127, 142, 75, 65, 144, 32, 70, 108, 141, 23, 147, 196, 7, 70,
    221, 138, 113, 27, 144, 123, 77, 156, 223, 242, 63, 120, 121, 254, 249, 159, 10, 132,
    225, 150, 248, 159, 115, 2, 20, 71, 88, 46, 255, 194, 238, 214, 26, 105, 154, 89, 42,
    32, 32, 131, 153, 217, 180, 1, 57, 3, 236, 248, 192, 193, 195, 130, 37, 131, 21, 229,
    144, 224, 202, 236, 146, 160, 52, 45, 147, 112, 224, 111, 173, 118, 64, 114, 169, 43, 1,
    71, 185, 112, 62, 96, 229, 23, 4, 2, 128, 51, 138, 47, 164, 10, 0, 100, 225, 1, 186,
    187, 189, 250, 101, 179, 126, 54, 44, 0, 99, 18, 8, 72, 190, 113, 102, 71, 113, 69, 132,
    121, 143, 184, 9, 1, 184, 227, 227, 178, 101,
];

fn build_synthetic_core_elf() -> Vec<u8> {
    let mut elf = Vec::new();

    // 1. ELF Header (64 bytes)
    elf.extend_from_slice(&[0x7F, b'E', b'L', b'F', 2, 1, 1, 0]); // ident
    elf.extend_from_slice(&[0u8; 8]); // ident padding
    elf.extend_from_slice(&4_u16.to_le_bytes()); // e_type = ET_CORE (4)
    elf.extend_from_slice(&0x3E_u16.to_le_bytes()); // e_machine = x86_64 (62)
    elf.extend_from_slice(&1_u32.to_le_bytes()); // e_version = 1
    elf.extend_from_slice(&0_u64.to_le_bytes()); // e_entry
    elf.extend_from_slice(&64_u64.to_le_bytes()); // e_phoff = 64
    elf.extend_from_slice(&0_u64.to_le_bytes()); // e_shoff = 0
    elf.extend_from_slice(&0_u32.to_le_bytes()); // e_flags
    elf.extend_from_slice(&64_u16.to_le_bytes()); // e_ehsize
    elf.extend_from_slice(&56_u16.to_le_bytes()); // e_phentsize
    elf.extend_from_slice(&1_u16.to_le_bytes()); // e_phnum = 1
    elf.extend_from_slice(&0_u16.to_le_bytes()); // e_shentsize
    elf.extend_from_slice(&0_u16.to_le_bytes()); // e_shnum
    elf.extend_from_slice(&0_u16.to_le_bytes()); // e_shstrndx
    assert_eq!(elf.len(), 64);

    // Build notes
    let mut notes = Vec::new();
    let add_note = |buf: &mut Vec<u8>, ntype: u32, name: &[u8], desc: &[u8]| {
        buf.extend_from_slice(&(name.len() as u32).to_le_bytes());
        buf.extend_from_slice(&(desc.len() as u32).to_le_bytes());
        buf.extend_from_slice(&ntype.to_le_bytes());
        buf.extend_from_slice(name);
        let name_pad = (name.len() + 3) & !3;
        buf.resize(buf.len() + (name_pad - name.len()), 0);
        buf.extend_from_slice(desc);
        let desc_pad = (desc.len() + 3) & !3;
        buf.resize(buf.len() + (desc_pad - desc.len()), 0);
    };

    // Note 1: NT_PRSTATUS
    let mut desc_status = vec![0u8; 336];
    desc_status[0..4].copy_from_slice(&11_i32.to_le_bytes());
    desc_status[12..14].copy_from_slice(&11_i16.to_le_bytes());
    desc_status[32..36].copy_from_slice(&4242_u32.to_le_bytes());
    add_note(&mut notes, 1, b"CORE\0", &desc_status);

    // Note 2: NT_PRPSINFO
    let mut desc_psinfo = vec![0u8; 136];
    desc_psinfo[24..28].copy_from_slice(&4242_u32.to_le_bytes());
    let comm = b"worker_crash\0";
    desc_psinfo[40..40 + comm.len()].copy_from_slice(comm);
    let cmdline = b"/usr/bin/worker_crash --arg\0";
    desc_psinfo[56..56 + cmdline.len()].copy_from_slice(cmdline);
    add_note(&mut notes, 3, b"CORE\0", &desc_psinfo);

    // Note 3: NT_SIGINFO
    let mut desc_siginfo = vec![0u8; 128];
    desc_siginfo[0..4].copy_from_slice(&11_i32.to_le_bytes());
    desc_siginfo[8..12].copy_from_slice(&1_i32.to_le_bytes());
    desc_siginfo[16..24].copy_from_slice(&0xDEAD_BEEF_0000_u64.to_le_bytes());
    add_note(&mut notes, 0x53494749, b"CORE\0", &desc_siginfo);

    // Note 4: NT_FILE
    let mut desc_file = Vec::new();
    desc_file.extend_from_slice(&2_u64.to_le_bytes()); // count
    desc_file.extend_from_slice(&4096_u64.to_le_bytes()); // page_size
    desc_file.extend_from_slice(&0x400000_u64.to_le_bytes());
    desc_file.extend_from_slice(&0x401000_u64.to_le_bytes());
    desc_file.extend_from_slice(&0_u64.to_le_bytes());
    desc_file.extend_from_slice(&0x7F0000_u64.to_le_bytes());
    desc_file.extend_from_slice(&0x7F1000_u64.to_le_bytes());
    desc_file.extend_from_slice(&0_u64.to_le_bytes());
    desc_file.extend_from_slice(b"/usr/bin/worker_crash\0/usr/lib/libc.so.6\0");
    add_note(&mut notes, 0x46494c45, b"CORE\0", &desc_file);

    // 2. Program Header 0: PT_NOTE (56 bytes)
    elf.extend_from_slice(&4_u32.to_le_bytes()); // p_type = PT_NOTE (4)
    elf.extend_from_slice(&0_u32.to_le_bytes()); // p_flags
    elf.extend_from_slice(&120_u64.to_le_bytes()); // p_offset = 64 + 56 = 120
    elf.extend_from_slice(&0_u64.to_le_bytes()); // p_vaddr
    elf.extend_from_slice(&0_u64.to_le_bytes()); // p_paddr
    elf.extend_from_slice(&(notes.len() as u64).to_le_bytes()); // p_filesz
    elf.extend_from_slice(&0_u64.to_le_bytes()); // p_memsz
    elf.extend_from_slice(&4_u64.to_le_bytes()); // p_align
    assert_eq!(elf.len(), 120);

    // 3. Append Notes
    elf.extend_from_slice(&notes);
    elf
}

#[test]
fn test_extract_elf_crash_headers_from_raw_elf() {
    let raw = build_synthetic_core_elf();
    let mut cursor = Cursor::new(raw);
    let header = extract_elf_crash_headers(&mut cursor).unwrap();

    assert_eq!(header.class, 2);
    assert_eq!(header.endian, 1);
    assert_eq!(header.endianness, 1);
    assert!(header.is_64bit());
    assert!(header.is_little_endian());
    assert_eq!(header.machine, 0x3E);
    assert_eq!(header.architecture, "x86_64");
    assert_eq!(header.signal, Some(11));
    assert_eq!(header.signal_code, Some(1));
    assert_eq!(header.fault_addr, Some(0xDEAD_BEEF_0000));
    assert_eq!(header.pid, Some(4242));
    assert_eq!(header.comm.as_deref(), Some("worker_crash"));
    assert_eq!(header.cmdline.as_deref(), Some("/usr/bin/worker_crash --arg"));
    assert_eq!(header.mapped_files.len(), 2);
    assert_eq!(header.mapped_files[0], "/usr/bin/worker_crash");
    assert_eq!(header.mapped_files[1], "/usr/lib/libc.so.6");
}

#[test]
fn test_extract_elf_crash_headers_from_lz4() {
    let raw = build_synthetic_core_elf();
    let mut compressed = Vec::new();
    {
        let mut encoder = lz4_flex::frame::FrameEncoder::new(&mut compressed);
        encoder.write_all(&raw).unwrap();
        encoder.finish().unwrap();
    }

    let mut cursor = Cursor::new(compressed);
    let header = extract_elf_crash_headers(&mut cursor).unwrap();

    assert_eq!(header.architecture, "x86_64");
    assert_eq!(header.signal, Some(11));
    assert_eq!(header.pid, Some(4242));
    assert_eq!(header.comm.as_deref(), Some("worker_crash"));
}

#[test]
fn test_extract_elf_crash_headers_from_zstd() {
    let mut cursor = Cursor::new(SYNTHETIC_ZSTD_CORE);
    let header = extract_elf_crash_headers(&mut cursor).unwrap();

    assert_eq!(header.architecture, "x86_64");
    assert_eq!(header.signal, Some(11));
    assert_eq!(header.pid, Some(4242));
    assert_eq!(header.comm.as_deref(), Some("worker_crash"));
    assert_eq!(header.fault_addr, Some(0xDEAD_BEEF_0000));
}

#[test]
fn test_extract_elf_crash_headers_from_file_missing() {
    let dir = tempdir().unwrap();
    let missing = dir.path().join("non_existent.core");
    let err = extract_elf_crash_headers_from_file(&missing).unwrap_err();
    assert!(err.to_string().contains("Coredump file not found"));
}

#[test]
fn test_extract_elf_crash_headers_from_file_success() {
    let dir = tempdir().unwrap();
    let core_path = dir.path().join("core.worker.1000.zst");
    fs::write(&core_path, SYNTHETIC_ZSTD_CORE).unwrap();

    let header = extract_elf_crash_headers_from_file(&core_path).unwrap();
    assert_eq!(header.comm.as_deref(), Some("worker_crash"));
    assert_eq!(header.pid, Some(4242));
}
