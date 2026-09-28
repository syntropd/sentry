//! Pure-Rust parser for ELF note segments in coredump images.
//!
//! Parses `NT_PRSTATUS`, `NT_PRPSINFO`, `NT_SIGINFO`, and `NT_FILE` to extract
//! signal, fault address, PID, process name, command line, and mapped files.

/// Note type for PRSTATUS (registers, signal, PID).
pub const NT_PRSTATUS: u32 = 1;
/// Note type for PRPSINFO (process info, comm, cmdline).
pub const NT_PRPSINFO: u32 = 3;
/// Note type for SIGINFO (signal details and fault address).
pub const NT_SIGINFO: u32 = 0x53494749;
/// Note type for NT_FILE (mapped memory files and executable paths).
pub const NT_FILE: u32 = 0x46494c45;

/// Aggregated crash metadata extracted from ELF note segments.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ElfNotesInfo {
    /// Terminating signal number.
    pub signal: Option<i32>,
    /// Signal code (si_code).
    pub signal_code: Option<i32>,
    /// Fault memory address.
    pub fault_addr: Option<u64>,
    /// Terminating process ID.
    pub pid: Option<u32>,
    /// Command / process name (pr_fname).
    pub comm: Option<String>,
    /// Command line arguments (pr_psargs).
    pub cmdline: Option<String>,
    /// Mapped executables and shared library paths.
    pub mapped_files: Vec<String>,
}

/// Safely aligns a length to the next 4-byte boundary without overflow.
#[inline]
pub fn align_up_4(n: usize) -> Option<usize> {
    n.checked_add(3).map(|v| v & !3)
}

/// Parses all note segments found in the decompressed coredump buffer.
pub fn parse_elf_notes(
    bytes: &[u8],
    note_segments: &[(u64, u64)],
    is_64bit: bool,
    is_le: bool,
) -> ElfNotesInfo {
    let mut info = ElfNotesInfo::default();

    for &(seg_offset, seg_size) in note_segments {
        let Ok(start) = usize::try_from(seg_offset) else { continue };
        if start >= bytes.len() { continue; }

        let end = match usize::try_from(seg_size) {
            Ok(sz) => start.saturating_add(sz).min(bytes.len()),
            Err(_) => bytes.len(),
        };
        if start >= end { continue; }

        if let Some(seg_bytes) = bytes.get(start..end) {
            parse_note_records(seg_bytes, is_64bit, is_le, &mut info);
        }
    }

    info
}

fn parse_note_records(bytes: &[u8], is_64bit: bool, is_le: bool, info: &mut ElfNotesInfo) {
    let mut pos = 0usize;
    while let Some(hdr_end) = pos.checked_add(12) {
        if hdr_end > bytes.len() { break; }

        let raw_namesz = read_u32(bytes, pos, is_le).unwrap_or(0);
        let raw_descsz = match pos.checked_add(4).and_then(|p| read_u32(bytes, p, is_le)) {
            Some(v) => v,
            None => break,
        };
        let ntype = match pos.checked_add(8).and_then(|p| read_u32(bytes, p, is_le)) {
            Some(v) => v,
            None => break,
        };

        let Ok(namesz) = usize::try_from(raw_namesz) else { break };
        let Ok(descsz) = usize::try_from(raw_descsz) else { break };

        let Some(name_pad) = align_up_4(namesz) else { break };
        let Some(desc_pad) = align_up_4(descsz) else { break };

        let Some(desc_offset) = hdr_end.checked_add(name_pad) else { break };
        let Some(desc_end) = desc_offset.checked_add(descsz) else { break };

        if desc_end > bytes.len() { break; }

        if let Some(desc) = bytes.get(desc_offset..desc_end) {
            match ntype {
                NT_PRSTATUS => parse_prstatus(desc, is_64bit, is_le, info),
                NT_PRPSINFO => parse_prpsinfo(desc, is_64bit, is_le, info),
                NT_SIGINFO => parse_siginfo(desc, is_64bit, is_le, info),
                NT_FILE => parse_nt_file(desc, is_64bit, is_le, info),
                _ => {}
            }
        }

        let Some(next_pos) = desc_offset.checked_add(desc_pad) else { break };
        if next_pos <= pos { break; }
        pos = next_pos;
    }
}

fn parse_prstatus(desc: &[u8], is_64bit: bool, is_le: bool, info: &mut ElfNotesInfo) {
    if is_64bit && desc.len() >= 36 {
        let cursig = read_u16(desc, 12, is_le).unwrap_or(0) as i32;
        let signo = read_i32(desc, 0, is_le).unwrap_or(0);
        let pid = read_u32(desc, 32, is_le).unwrap_or(0);

        if info.signal.is_none() {
            info.signal = if cursig > 0 { Some(cursig) } else if signo > 0 { Some(signo) } else { None };
        }
        if info.pid.is_none() && pid > 0 { info.pid = Some(pid); }
    } else if !is_64bit && desc.len() >= 28 {
        let cursig = read_u16(desc, 12, is_le).unwrap_or(0) as i32;
        let signo = read_i32(desc, 0, is_le).unwrap_or(0);
        let pid = read_u32(desc, 24, is_le).unwrap_or(0);

        if info.signal.is_none() {
            info.signal = if cursig > 0 { Some(cursig) } else if signo > 0 { Some(signo) } else { None };
        }
        if info.pid.is_none() && pid > 0 { info.pid = Some(pid); }
    }
}

fn parse_prpsinfo(desc: &[u8], is_64bit: bool, is_le: bool, info: &mut ElfNotesInfo) {
    if is_64bit && desc.len() >= 136 {
        let pid = read_u32(desc, 24, is_le).unwrap_or(0);
        if info.pid.is_none() && pid > 0 { info.pid = Some(pid); }
        if info.comm.is_none() {
            if let Some(comm_b) = desc.get(40..56) {
                let comm = extract_cstring(comm_b);
                if !comm.is_empty() { info.comm = Some(comm); }
            }
        }
        if info.cmdline.is_none() {
            if let Some(cmd_b) = desc.get(56..136) {
                let cmdline = extract_cstring(cmd_b);
                if !cmdline.is_empty() { info.cmdline = Some(cmdline); }
            }
        }
    } else if !is_64bit && desc.len() >= 124 {
        let pid = read_u32(desc, 12, is_le).unwrap_or(0);
        if info.pid.is_none() && pid > 0 { info.pid = Some(pid); }
        if info.comm.is_none() {
            if let Some(comm_b) = desc.get(28..44) {
                let comm = extract_cstring(comm_b);
                if !comm.is_empty() { info.comm = Some(comm); }
            }
        }
        if info.cmdline.is_none() {
            if let Some(cmd_b) = desc.get(44..124) {
                let cmdline = extract_cstring(cmd_b);
                if !cmdline.is_empty() { info.cmdline = Some(cmdline); }
            }
        }
    }
}

fn parse_siginfo(desc: &[u8], is_64bit: bool, is_le: bool, info: &mut ElfNotesInfo) {
    if desc.len() < 12 { return; }
    let signo = read_i32(desc, 0, is_le).unwrap_or(0);
    let code = read_i32(desc, 8, is_le).unwrap_or(0);

    if signo > 0 { info.signal = Some(signo); }
    info.signal_code = Some(code);

    if is_64bit && desc.len() >= 24 {
        info.fault_addr = read_u64(desc, 16, is_le);
    } else if !is_64bit && desc.len() >= 16 {
        info.fault_addr = read_u32(desc, 12, is_le).map(|a| a as u64);
    }
}

fn parse_nt_file(desc: &[u8], is_64bit: bool, is_le: bool, info: &mut ElfNotesInfo) {
    let (count, str_offset) = if is_64bit {
        if desc.len() < 16 { return; }
        let Ok(count) = usize::try_from(read_u64(desc, 0, is_le).unwrap_or(0)) else { return };
        let Some(table_sz) = count.checked_mul(24) else { return };
        let Some(offset) = 16_usize.checked_add(table_sz) else { return };
        (count, offset)
    } else {
        if desc.len() < 8 { return; }
        let Ok(count) = usize::try_from(read_u32(desc, 0, is_le).unwrap_or(0)) else { return };
        let Some(table_sz) = count.checked_mul(12) else { return };
        let Some(offset) = 8_usize.checked_add(table_sz) else { return };
        (count, offset)
    };

    if count == 0 || str_offset >= desc.len() { return; }

    let Some(strings_buf) = desc.get(str_offset..) else { return };
    let paths: Vec<String> = strings_buf
        .split(|&b| b == 0)
        .filter(|s| !s.is_empty())
        .filter_map(|s| std::str::from_utf8(s).ok().map(|str_val| str_val.trim().to_string()))
        .collect();

    for p in paths {
        if !info.mapped_files.contains(&p) {
            info.mapped_files.push(p);
        }
    }
}

fn extract_cstring(slice: &[u8]) -> String {
    let len = slice.iter().position(|&b| b == 0).unwrap_or(slice.len());
    std::str::from_utf8(&slice[..len]).unwrap_or("").trim().to_string()
}

fn read_u16(buf: &[u8], offset: usize, is_le: bool) -> Option<u16> {
    let end = offset.checked_add(2)?;
    let slice = buf.get(offset..end)?;
    let arr: [u8; 2] = slice.try_into().ok()?;
    Some(if is_le { u16::from_le_bytes(arr) } else { u16::from_be_bytes(arr) })
}

fn read_u32(buf: &[u8], offset: usize, is_le: bool) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let slice = buf.get(offset..end)?;
    let arr: [u8; 4] = slice.try_into().ok()?;
    Some(if is_le { u32::from_le_bytes(arr) } else { u32::from_be_bytes(arr) })
}

fn read_i32(buf: &[u8], offset: usize, is_le: bool) -> Option<i32> {
    let end = offset.checked_add(4)?;
    let slice = buf.get(offset..end)?;
    let arr: [u8; 4] = slice.try_into().ok()?;
    Some(if is_le { i32::from_le_bytes(arr) } else { i32::from_be_bytes(arr) })
}

fn read_u64(buf: &[u8], offset: usize, is_le: bool) -> Option<u64> {
    let end = offset.checked_add(8)?;
    let slice = buf.get(offset..end)?;
    let arr: [u8; 8] = slice.try_into().ok()?;
    Some(if is_le { u64::from_le_bytes(arr) } else { u64::from_be_bytes(arr) })
}
