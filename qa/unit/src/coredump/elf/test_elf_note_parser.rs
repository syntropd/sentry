//! 1:1 Unit QA tests for ELF note segments parsing.

use sentry_driver::coredump::elf_note_parser::{
    parse_elf_notes, NT_FILE, NT_PRPSINFO, NT_PRSTATUS, NT_SIGINFO,
};

fn encode_note(ntype: u32, name: &[u8], desc: &[u8]) -> Vec<u8> {
    let mut buf = Vec::new();
    let namesz = name.len() as u32;
    let descsz = desc.len() as u32;

    buf.extend_from_slice(&namesz.to_le_bytes());
    buf.extend_from_slice(&descsz.to_le_bytes());
    buf.extend_from_slice(&ntype.to_le_bytes());

    buf.extend_from_slice(name);
    let name_pad = (name.len() + 3) & !3;
    buf.resize(buf.len() + (name_pad - name.len()), 0);

    buf.extend_from_slice(desc);
    let desc_pad = (desc.len() + 3) & !3;
    buf.resize(buf.len() + (desc_pad - desc.len()), 0);

    buf
}

#[test]
fn test_parse_elf_notes_prstatus_64bit() {
    let mut desc = vec![0u8; 336];
    desc[0..4].copy_from_slice(&11_i32.to_le_bytes()); // si_signo = 11 (SIGSEGV)
    desc[12..14].copy_from_slice(&11_i16.to_le_bytes()); // pr_cursig = 11
    desc[32..36].copy_from_slice(&4321_u32.to_le_bytes()); // pr_pid = 4321

    let note_bytes = encode_note(NT_PRSTATUS, b"CORE\0", &desc);
    let segs = [(0_u64, note_bytes.len() as u64)];
    let info = parse_elf_notes(&note_bytes, &segs, true, true);

    assert_eq!(info.signal, Some(11));
    assert_eq!(info.pid, Some(4321));
}

#[test]
fn test_parse_elf_notes_prpsinfo_64bit() {
    let mut desc = vec![0u8; 136];
    desc[24..28].copy_from_slice(&5555_u32.to_le_bytes()); // pr_pid

    let comm = b"sentry_service\0";
    desc[40..40 + comm.len()].copy_from_slice(comm);

    let cmdline = b"/usr/bin/sentry_service --worker\0";
    desc[56..56 + cmdline.len()].copy_from_slice(cmdline);

    let note_bytes = encode_note(NT_PRPSINFO, b"CORE\0", &desc);
    let segs = [(0_u64, note_bytes.len() as u64)];
    let info = parse_elf_notes(&note_bytes, &segs, true, true);

    assert_eq!(info.pid, Some(5555));
    assert_eq!(info.comm.as_deref(), Some("sentry_service"));
    assert_eq!(info.cmdline.as_deref(), Some("/usr/bin/sentry_service --worker"));
}

#[test]
fn test_parse_elf_notes_siginfo_64bit() {
    let mut desc = vec![0u8; 128];
    desc[0..4].copy_from_slice(&11_i32.to_le_bytes()); // si_signo
    desc[8..12].copy_from_slice(&1_i32.to_le_bytes()); // si_code
    desc[16..24].copy_from_slice(&0xDEAD_BEEF_CAFE_u64.to_le_bytes()); // fault_addr

    let note_bytes = encode_note(NT_SIGINFO, b"CORE\0", &desc);
    let segs = [(0_u64, note_bytes.len() as u64)];
    let info = parse_elf_notes(&note_bytes, &segs, true, true);

    assert_eq!(info.signal, Some(11));
    assert_eq!(info.signal_code, Some(1));
    assert_eq!(info.fault_addr, Some(0xDEAD_BEEF_CAFE));
}

#[test]
fn test_parse_elf_notes_nt_file_64bit() {
    let mut desc = Vec::new();
    let count = 2_u64;
    let page_size = 4096_u64;
    desc.extend_from_slice(&count.to_le_bytes());
    desc.extend_from_slice(&page_size.to_le_bytes());

    // Entry 0
    desc.extend_from_slice(&0x400000_u64.to_le_bytes());
    desc.extend_from_slice(&0x401000_u64.to_le_bytes());
    desc.extend_from_slice(&0_u64.to_le_bytes());

    // Entry 1
    desc.extend_from_slice(&0x7F0000_u64.to_le_bytes());
    desc.extend_from_slice(&0x7F1000_u64.to_le_bytes());
    desc.extend_from_slice(&0_u64.to_le_bytes());

    // String table
    desc.extend_from_slice(b"/usr/bin/target_bin\0/usr/lib/libtest.so\0");

    let note_bytes = encode_note(NT_FILE, b"CORE\0", &desc);
    let segs = [(0_u64, note_bytes.len() as u64)];
    let info = parse_elf_notes(&note_bytes, &segs, true, true);

    assert_eq!(info.mapped_files.len(), 2);
    assert_eq!(info.mapped_files[0], "/usr/bin/target_bin");
    assert_eq!(info.mapped_files[1], "/usr/lib/libtest.so");
}

#[test]
fn test_parse_elf_notes_32bit() {
    let mut desc = vec![0u8; 128];
    desc[0..4].copy_from_slice(&6_i32.to_le_bytes()); // SIGABRT
    desc[12..14].copy_from_slice(&6_i16.to_le_bytes());
    desc[24..28].copy_from_slice(&8888_u32.to_le_bytes()); // pid on 32-bit

    let note_bytes = encode_note(NT_PRSTATUS, b"CORE\0", &desc);
    let segs = [(0_u64, note_bytes.len() as u64)];
    let info = parse_elf_notes(&note_bytes, &segs, false, true);

    assert_eq!(info.signal, Some(6));
    assert_eq!(info.pid, Some(8888));
}

#[test]
fn test_parse_elf_notes_truncated() {
    let truncated = vec![0x05, 0x00, 0x00, 0x00, 0x50];
    let segs = [(0_u64, truncated.len() as u64)];
    let info = parse_elf_notes(&truncated, &segs, true, true);

    assert_eq!(info.signal, None);
    assert_eq!(info.pid, None);
}

#[test]
fn test_align_up_4_boundaries_and_overflow() {
    use sentry_driver::coredump::elf_note_parser::align_up_4;
    assert_eq!(align_up_4(0), Some(0));
    assert_eq!(align_up_4(1), Some(4));
    assert_eq!(align_up_4(4), Some(4));
    assert_eq!(align_up_4(5), Some(8));
    assert_eq!(align_up_4(usize::MAX), None);
    assert_eq!(align_up_4(usize::MAX - 1), None);
    assert_eq!(align_up_4(usize::MAX - 2), None);
    assert_eq!(align_up_4(usize::MAX - 3), Some((usize::MAX - 3) & !3));
}

#[test]
fn test_parse_elf_notes_huge_seg_offset_graceful() {
    let dummy = vec![0u8; 32];
    let segs = [(u64::MAX, 16_u64)];
    let info = parse_elf_notes(&dummy, &segs, true, true);
    assert_eq!(info.signal, None);
    assert_eq!(info.pid, None);
}

#[test]
fn test_parse_elf_notes_namesz_descsz_overflow_32bit() {
    let mut note = Vec::new();
    note.extend_from_slice(&u32::MAX.to_le_bytes()); // namesz = u32::MAX
    note.extend_from_slice(&u32::MAX.to_le_bytes()); // descsz = u32::MAX
    note.extend_from_slice(&NT_PRSTATUS.to_le_bytes());
    note.extend_from_slice(&[0u8; 16]);

    let segs = [(0_u64, note.len() as u64)];
    let info = parse_elf_notes(&note, &segs, false, true);
    assert_eq!(info.signal, None);
    assert_eq!(info.pid, None);
}
