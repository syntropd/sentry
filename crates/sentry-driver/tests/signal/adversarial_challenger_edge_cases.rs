//! Empirical Challenger 2 Adversarial Stress Suite for HM1 Remediation.
//!
//! Validates:
//! 1. Malformed / truncated zstd frames and invalid lz4 block sizes.
//! 2. Truncated ELF headers (< 64 bytes for 64-bit, < 52 bytes for 32-bit).
//! 3. Near u64::MAX phoff, zero phentsize, huge phnum combinations.
//! 4. Corrupt note headers with u32::MAX namesz and descsz, unaligned lengths.
//! 5. Absolute panic-freedom and deterministic termination without infinite loops.

use sentry_driver::coredump::elf_extractor::extract_elf_crash_headers;
use sentry_driver::coredump::elf_header_parser::{locate_note_segments, parse_elf_header, ElfHeaderInfo};
use sentry_driver::coredump::elf_note_parser::{parse_elf_notes, NT_FILE, NT_PRPSINFO, NT_PRSTATUS, NT_SIGINFO};
use sentry_driver::coredump::stream_reader::read_bounded_coredump_bytes;
use std::io::Cursor;
use std::time::Instant;

fn build_header(class: u8, endian: u8, phoff: u64, phentsize: u16, phnum: u16) -> Vec<u8> {
    let sz = if class == 1 { 52 } else { 64 };
    let mut buf = vec![0u8; sz];
    buf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
    buf[4] = class;
    buf[5] = endian;
    let is_le = endian == 1;
    if is_le {
        buf[18..20].copy_from_slice(&0x3E_u16.to_le_bytes());
        if class == 2 {
            buf[32..40].copy_from_slice(&phoff.to_le_bytes());
            buf[54..56].copy_from_slice(&phentsize.to_le_bytes());
            buf[56..58].copy_from_slice(&phnum.to_le_bytes());
        } else {
            buf[28..32].copy_from_slice(&(phoff as u32).to_le_bytes());
            buf[42..44].copy_from_slice(&phentsize.to_le_bytes());
            buf[44..46].copy_from_slice(&phnum.to_le_bytes());
        }
    }
    buf
}

#[test]
fn test_adversarial_malformed_zstd_and_lz4_matrix() {
    let test_payloads = vec![
        vec![],
        vec![0x28],
        vec![0x28, 0xB5],
        vec![0x28, 0xB5, 0x2F],
        vec![0x28, 0xB5, 0x2F, 0xFD],
        vec![0x28, 0xB5, 0x2F, 0xFD, 0x00],
        vec![0x28, 0xB5, 0x2F, 0xFD, 0xFF, 0xFF, 0xFF, 0xFF],
        vec![0x04],
        vec![0x04, 0x22],
        vec![0x04, 0x22, 0x4D],
        vec![0x04, 0x22, 0x4D, 0x18],
        vec![0x04, 0x22, 0x4D, 0x18, 0x64, 0x70, 0xB7, 0x00, 0x00, 0x00, 0x00],
        vec![0x04, 0x22, 0x4D, 0x18, 0x64, 0x70, 0xB7, 0xFF, 0xFF, 0xFF, 0x7F],
        vec![0x04, 0x22, 0x4D, 0x18, 0x64, 0x70, 0xB7, 0xFF, 0xFF, 0xFF, 0xFF],
    ];

    for (idx, payload) in test_payloads.iter().enumerate() {
        let panic_res = std::panic::catch_unwind(|| {
            let mut cursor = Cursor::new(payload);
            let _ = read_bounded_coredump_bytes(&mut cursor);
            let mut cursor2 = Cursor::new(payload);
            let _ = extract_elf_crash_headers(&mut cursor2);
        });
        assert!(panic_res.is_ok(), "Panic on malformed stream payload index {idx}");
    }
}

#[test]
fn test_adversarial_truncated_elf_headers_sweep() {
    // Sweep all lengths from 0 to 63 bytes for 64-bit and 32-bit ELF headers
    for len in 0..64 {
        let mut buf = vec![0u8; len];
        if len >= 4 {
            buf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
        }
        if len >= 5 { buf[4] = 2; }
        if len >= 6 { buf[5] = 1; }

        let panic_res = std::panic::catch_unwind(|| {
            let _ = parse_elf_header(&buf);
        });
        assert!(panic_res.is_ok(), "Panic on 64-bit ELF header length {len}");
    }

    for len in 0..52 {
        let mut buf = vec![0u8; len];
        if len >= 4 {
            buf[0..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
        }
        if len >= 5 { buf[4] = 1; }
        if len >= 6 { buf[5] = 1; }

        let panic_res = std::panic::catch_unwind(|| {
            let _ = parse_elf_header(&buf);
        });
        assert!(panic_res.is_ok(), "Panic on 32-bit ELF header length {len}");
    }
}

#[test]
fn test_adversarial_phoff_phentsize_phnum_stress_matrix() {
    let phoffs = [
        0, 1, 64, 65535, 65536, 1_000_000,
        (1_u64 << 31) - 1, (1_u64 << 31),
        (1_u64 << 32) - 1, (1_u64 << 32),
        u64::MAX - 65536, u64::MAX - 56, u64::MAX - 1, u64::MAX,
    ];
    let phentsizes: [u16; 8] = [0, 1, 2, 4, 16, 55, 56, u16::MAX];
    let phnums: [u16; 6] = [0, 1, 2, 1000, 65535, u16::MAX];

    let dummy_buffer = vec![0xAA; 4096];

    for &phoff in &phoffs {
        for &phentsize in &phentsizes {
            for &phnum in &phnums {
                let start = Instant::now();
                let header = ElfHeaderInfo {
                    class: 2,
                    endian: 1,
                    machine: 0x3E,
                    architecture: "x86_64".into(),
                    phoff,
                    phentsize: phentsize as usize,
                    phnum: phnum as usize,
                };

                let panic_res = std::panic::catch_unwind(|| {
                    locate_note_segments(&dummy_buffer, &header)
                });
                assert!(panic_res.is_ok(), "Panic on phoff={phoff}, phentsize={phentsize}, phnum={phnum}");
                assert!(start.elapsed().as_millis() < 50, "Excessive runtime/infinite loop detected");

                // Full parse test
                let raw_hdr = build_header(2, 1, phoff, phentsize, phnum);
                let panic_parse = std::panic::catch_unwind(|| {
                    if let Ok(hdr) = parse_elf_header(&raw_hdr) {
                        let _ = locate_note_segments(&dummy_buffer, &hdr);
                    }
                });
                assert!(panic_parse.is_ok(), "Panic on full parse with phoff={phoff}");
            }
        }
    }
}

#[test]
fn test_adversarial_corrupt_notes_u32_max_and_unaligned_lengths() {
    let boundary_vals: [u32; 10] = [
        0, 1, 2, 3, 5, 7, 13,
        u32::MAX - 4, u32::MAX - 3, u32::MAX,
    ];
    let ntypes = [NT_PRSTATUS, NT_PRPSINFO, NT_SIGINFO, NT_FILE, 0, 9999];

    for &namesz in &boundary_vals {
        for &descsz in &boundary_vals {
            for &ntype in &ntypes {
                let mut note = Vec::with_capacity(64);
                note.extend_from_slice(&namesz.to_le_bytes());
                note.extend_from_slice(&descsz.to_le_bytes());
                note.extend_from_slice(&ntype.to_le_bytes());
                // Trailing 16 bytes (unaligned relative to namesz/descsz)
                note.extend_from_slice(&[0x55; 16]);

                let segs = [(0_u64, note.len() as u64)];

                let start = Instant::now();
                let panic_64 = std::panic::catch_unwind(|| {
                    let _ = parse_elf_notes(&note, &segs, true, true);
                });
                assert!(panic_64.is_ok(), "Panic 64-bit notes: namesz={namesz}, descsz={descsz}, type={ntype}");

                let panic_32 = std::panic::catch_unwind(|| {
                    let _ = parse_elf_notes(&note, &segs, false, true);
                });
                assert!(panic_32.is_ok(), "Panic 32-bit notes: namesz={namesz}, descsz={descsz}, type={ntype}");

                assert!(start.elapsed().as_millis() < 50, "Infinite loop in note parser");
            }
        }
    }
}

#[test]
fn test_adversarial_corrupt_note_segments_and_nt_file() {
    let dummy = vec![0x77; 1024];
    let corrupt_segs = [
        (u64::MAX, u64::MAX),
        (u64::MAX - 100, 200),
        (100, u64::MAX),
        (0, 0),
        (1024, 10),
        (2000, 100),
    ];

    for seg in corrupt_segs {
        let panic_res = std::panic::catch_unwind(|| {
            let _ = parse_elf_notes(&dummy, &[seg], true, true);
        });
        assert!(panic_res.is_ok(), "Panic on corrupt seg ({}, {})", seg.0, seg.1);
    }

    // NT_FILE stress with non-aligned strings and overflow count
    let mut nt_file_desc = Vec::new();
    nt_file_desc.extend_from_slice(&(u64::MAX).to_le_bytes()); // Count = u64::MAX
    nt_file_desc.extend_from_slice(&4096_u64.to_le_bytes());
    nt_file_desc.extend_from_slice(&[0x01; 64]);

    let mut note = Vec::new();
    note.extend_from_slice(&4_u32.to_le_bytes());
    note.extend_from_slice(&(nt_file_desc.len() as u32).to_le_bytes());
    note.extend_from_slice(&NT_FILE.to_le_bytes());
    note.extend_from_slice(b"CORE");
    note.extend_from_slice(&nt_file_desc);

    let segs = [(0_u64, note.len() as u64)];
    let panic_nt_file = std::panic::catch_unwind(|| {
        let res = parse_elf_notes(&note, &segs, true, true);
        assert!(res.mapped_files.is_empty());
    });
    assert!(panic_nt_file.is_ok(), "Panic on NT_FILE u64::MAX count");
}
