//! 1:1 Unit QA tests for coredump directory scanner and ELF fallback.

use sentry_driver::coredump::find_latest_coredump;
use sentry_driver::coredump::lz4_flex;
use std::fs;
use std::io::Write;
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
    elf.extend_from_slice(&[0x7F, b'E', b'L', b'F', 2, 1, 1, 0]);
    elf.extend_from_slice(&[0u8; 8]);
    elf.extend_from_slice(&4_u16.to_le_bytes()); // ET_CORE
    elf.extend_from_slice(&0x3E_u16.to_le_bytes()); // x86_64
    elf.extend_from_slice(&1_u32.to_le_bytes());
    elf.extend_from_slice(&0_u64.to_le_bytes());
    elf.extend_from_slice(&64_u64.to_le_bytes());
    elf.extend_from_slice(&0_u64.to_le_bytes());
    elf.extend_from_slice(&0_u32.to_le_bytes());
    elf.extend_from_slice(&64_u16.to_le_bytes());
    elf.extend_from_slice(&56_u16.to_le_bytes());
    elf.extend_from_slice(&1_u16.to_le_bytes());
    elf.extend_from_slice(&0_u16.to_le_bytes());
    elf.extend_from_slice(&0_u16.to_le_bytes());
    elf.extend_from_slice(&0_u16.to_le_bytes());

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

    let mut desc_status = vec![0u8; 336];
    desc_status[0..4].copy_from_slice(&11_i32.to_le_bytes());
    desc_status[12..14].copy_from_slice(&11_i16.to_le_bytes());
    desc_status[32..36].copy_from_slice(&7777_u32.to_le_bytes());
    add_note(&mut notes, 1, b"CORE\0", &desc_status);

    let mut desc_psinfo = vec![0u8; 136];
    desc_psinfo[24..28].copy_from_slice(&7777_u32.to_le_bytes());
    let comm = b"target_proc\0";
    desc_psinfo[40..40 + comm.len()].copy_from_slice(comm);
    add_note(&mut notes, 3, b"CORE\0", &desc_psinfo);

    elf.extend_from_slice(&4_u32.to_le_bytes()); // PT_NOTE
    elf.extend_from_slice(&0_u32.to_le_bytes());
    elf.extend_from_slice(&120_u64.to_le_bytes());
    elf.extend_from_slice(&0_u64.to_le_bytes());
    elf.extend_from_slice(&0_u64.to_le_bytes());
    elf.extend_from_slice(&(notes.len() as u64).to_le_bytes());
    elf.extend_from_slice(&0_u64.to_le_bytes());
    elf.extend_from_slice(&4_u64.to_le_bytes());

    elf.extend_from_slice(&notes);
    elf
}

#[test]
fn test_find_latest_coredump_fallback_to_elf_zstd() {
    let dir = tempdir().unwrap();
    let core_file = dir.path().join("core.worker_crash.1000.zst");
    fs::write(&core_file, SYNTHETIC_ZSTD_CORE).unwrap();

    let record = find_latest_coredump(dir.path(), Some("worker_crash")).unwrap();
    assert_eq!(record.pid, 4242);
    assert_eq!(record.signal, 11);
    assert_eq!(record.signal_name, "SIGSEGV");
    assert_eq!(record.executable.as_deref(), Some("/usr/bin/worker_crash"));
    assert!(record.core_file.unwrap().ends_with(".zst"));
}

#[test]
fn test_find_latest_coredump_fallback_to_elf_lz4() {
    let dir = tempdir().unwrap();
    let raw = build_synthetic_core_elf();

    let mut compressed = Vec::new();
    {
        let mut encoder = lz4_flex::frame::FrameEncoder::new(&mut compressed);
        encoder.write_all(&raw).unwrap();
        encoder.finish().unwrap();
    }

    let core_file = dir.path().join("core.target_proc.1000.lz4");
    fs::write(&core_file, compressed).unwrap();

    let record = find_latest_coredump(dir.path(), Some("target_proc")).unwrap();
    assert_eq!(record.pid, 7777);
    assert_eq!(record.signal, 11);
    assert_eq!(record.signal_name, "SIGSEGV");
}

#[test]
fn test_find_latest_coredump_mismatch() {
    let dir = tempdir().unwrap();
    let core_file = dir.path().join("core.worker_crash.1000.zst");
    fs::write(&core_file, SYNTHETIC_ZSTD_CORE).unwrap();

    let res = find_latest_coredump(dir.path(), Some("completely_different_service"));
    assert!(res.is_none());
}

#[test]
fn test_find_latest_coredump_empty_dir() {
    let dir = tempdir().unwrap();
    let res = find_latest_coredump(dir.path(), None);
    assert!(res.is_none());

    let missing = dir.path().join("non_existent_subdir");
    let res2 = find_latest_coredump(&missing, None);
    assert!(res2.is_none());
}

#[test]
fn test_find_latest_coredump_extension_without_core_in_name() {
    let dir = tempdir().unwrap();
    let crash_file = dir.path().join("crash_dump_worker.zst");
    fs::write(&crash_file, SYNTHETIC_ZSTD_CORE).unwrap();

    let record = find_latest_coredump(dir.path(), Some("worker_crash")).unwrap();
    assert_eq!(record.pid, 4242);
    assert_eq!(record.signal, 11);
}
