//! Pure-Rust parser for 64-bit and 32-bit ELF headers and program headers.
//!
//! Locates `PT_NOTE` program header offset and segment size.

use sentry_core::error::CoredumpError;

/// Segment type for notes in ELF program headers.
pub const PT_NOTE: u32 = 4;

/// Parsed ELF header metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ElfHeaderInfo {
    /// ELF class (1 = 32-bit, 2 = 64-bit).
    pub class: u8,
    /// Endianness (1 = little-endian, 2 = big-endian).
    pub endian: u8,
    /// Machine architecture identifier.
    pub machine: u16,
    /// Architecture descriptive name.
    pub architecture: String,
    /// Program header table file offset.
    pub phoff: u64,
    /// Size of each program header entry in bytes.
    pub phentsize: usize,
    /// Number of program header entries.
    pub phnum: usize,
}

impl ElfHeaderInfo {
    /// Returns true if this is a 64-bit ELF binary.
    pub fn is_64bit(&self) -> bool {
        self.class == 2
    }

    /// Returns true if this binary is little-endian.
    pub fn is_little_endian(&self) -> bool {
        self.endian == 1
    }
}

/// Parses 32-bit or 64-bit ELF identification and program header offsets.
pub fn parse_elf_header(bytes: &[u8]) -> Result<ElfHeaderInfo, CoredumpError> {
    if bytes.len() < 16 {
        return Err(CoredumpError::ParseError("Buffer too small for ELF header".into()));
    }

    if bytes[0..4] != [0x7F, b'E', b'L', b'F'] {
        return Err(CoredumpError::ParseError("Invalid ELF magic bytes".into()));
    }

    let class = bytes[4];
    if class != 1 && class != 2 {
        return Err(CoredumpError::ParseError(format!("Unsupported ELF class: {class}")));
    }

    let endian = bytes[5];
    if endian != 1 && endian != 2 {
        return Err(CoredumpError::ParseError(format!("Unsupported ELF endianness: {endian}")));
    }

    let is_le = endian == 1;
    let machine = read_u16(bytes, 18, is_le)
        .ok_or_else(|| CoredumpError::ParseError("Truncated ELF machine field".into()))?;

    let architecture = match machine {
        0x03 => "x86".to_string(),
        0x3E => "x86_64".to_string(),
        0x28 => "arm".to_string(),
        0xB7 => "aarch64".to_string(),
        0xF3 => "riscv".to_string(),
        0x15 => "ppc64".to_string(),
        0x14 => "ppc".to_string(),
        0x16 => "s390".to_string(),
        0x02 => "sparc".to_string(),
        0x2B => "sparc9".to_string(),
        0x08 => "mips".to_string(),
        m => format!("machine_{m:#x}"),
    };

    let (phoff, phentsize, phnum) = if class == 2 {
        if bytes.len() < 64 {
            return Err(CoredumpError::ParseError("Truncated 64-bit ELF header".into()));
        }
        let phoff = read_u64(bytes, 32, is_le)
            .ok_or_else(|| CoredumpError::ParseError("Truncated 64-bit phoff".into()))?;
        let phentsize = read_u16(bytes, 54, is_le)
            .ok_or_else(|| CoredumpError::ParseError("Truncated 64-bit phentsize".into()))? as usize;
        let phnum = read_u16(bytes, 56, is_le)
            .ok_or_else(|| CoredumpError::ParseError("Truncated 64-bit phnum".into()))? as usize;
        (phoff, phentsize, phnum)
    } else {
        if bytes.len() < 52 {
            return Err(CoredumpError::ParseError("Truncated 32-bit ELF header".into()));
        }
        let phoff = read_u32(bytes, 28, is_le)
            .ok_or_else(|| CoredumpError::ParseError("Truncated 32-bit phoff".into()))? as u64;
        let phentsize = read_u16(bytes, 42, is_le)
            .ok_or_else(|| CoredumpError::ParseError("Truncated 32-bit phentsize".into()))? as usize;
        let phnum = read_u16(bytes, 44, is_le)
            .ok_or_else(|| CoredumpError::ParseError("Truncated 32-bit phnum".into()))? as usize;
        (phoff, phentsize, phnum)
    };

    Ok(ElfHeaderInfo {
        class,
        endian,
        machine,
        architecture,
        phoff,
        phentsize,
        phnum,
    })
}

/// Scans the program header table and locates all `PT_NOTE` segments as (offset, size).
pub fn locate_note_segments(
    bytes: &[u8],
    header: &ElfHeaderInfo,
) -> Result<Vec<(u64, u64)>, CoredumpError> {
    let mut notes = Vec::new();
    let is_le = header.is_little_endian();

    if header.phentsize == 0 {
        return Ok(notes);
    }

    let phoff = match usize::try_from(header.phoff) {
        Ok(off) => off,
        Err(_) => return Ok(notes),
    };

    for i in 0..header.phnum {
        let ph_step = match i.checked_mul(header.phentsize) {
            Some(step) => step,
            None => break,
        };

        let entry_offset = match phoff.checked_add(ph_step) {
            Some(off) => off,
            None => break,
        };

        let entry_end = match entry_offset.checked_add(header.phentsize) {
            Some(end) => end,
            None => break,
        };

        if entry_end > bytes.len() {
            // Buffer bounded to <64KB may not contain all trailing program headers
            break;
        }

        let Some(entry_bytes) = bytes.get(entry_offset..entry_end) else {
            break;
        };

        let p_type = read_u32(entry_bytes, 0, is_le).unwrap_or(0);

        if p_type == PT_NOTE {
            let (offset, size) = if header.is_64bit() {
                let off = read_u64(entry_bytes, 8, is_le).unwrap_or(0);
                let sz = read_u64(entry_bytes, 32, is_le).unwrap_or(0);
                (off, sz)
            } else {
                let off = read_u32(entry_bytes, 4, is_le).unwrap_or(0) as u64;
                let sz = read_u32(entry_bytes, 16, is_le).unwrap_or(0) as u64;
                (off, sz)
            };
            notes.push((offset, size));
        }
    }

    Ok(notes)
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

fn read_u64(buf: &[u8], offset: usize, is_le: bool) -> Option<u64> {
    let end = offset.checked_add(8)?;
    let slice = buf.get(offset..end)?;
    let arr: [u8; 8] = slice.try_into().ok()?;
    Some(if is_le { u64::from_le_bytes(arr) } else { u64::from_be_bytes(arr) })
}
