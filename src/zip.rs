//! A minimal deterministic ZIP writer.
//!
//! Every entry is stored without compression, which the ODF specification
//! requires for `mimetype` and accepts for every other part. The timestamp is
//! fixed, the entry order is the caller's and no extra field is written, so two
//! runs with the same input produce identical bytes.

use std::fmt;

/// One file to be written into the archive.
#[derive(Debug, Clone, Copy)]
pub struct Entry<'a> {
    name: &'a str,
    bytes: &'a [u8],
}

impl<'a> Entry<'a> {
    pub fn new(name: &'a str, bytes: &'a [u8]) -> Self {
        Self { name, bytes }
    }
}

/// Why an archive could not be written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipError {
    TooManyEntries,
    NameTooLong,
    EntryTooLarge,
    ArchiveTooLarge,
}

impl fmt::Display for ZipError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyEntries => formatter.write_str("the archive has too many entries"),
            Self::NameTooLong => formatter.write_str("an entry name is too long"),
            Self::EntryTooLarge => formatter.write_str("an entry is too large"),
            Self::ArchiveTooLarge => formatter.write_str("the archive is too large"),
        }
    }
}

impl std::error::Error for ZipError {}

const LOCAL_HEADER: u32 = 0x0403_4b50;
const CENTRAL_HEADER: u32 = 0x0201_4b50;
const END_OF_DIRECTORY: u32 = 0x0605_4b50;
/// 1980-01-01, the earliest date the format can represent.
const DOS_DATE: u16 = 0x0021;
const DOS_TIME: u16 = 0x0000;

/// Writes the entries as a stored ZIP archive.
pub fn write_stored(entries: &[Entry<'_>]) -> Result<Vec<u8>, ZipError> {
    if entries.len() > usize::from(u16::MAX) {
        return Err(ZipError::TooManyEntries);
    }
    let mut directory = Vec::new();
    let mut data = Vec::new();
    for entry in entries {
        let name = entry.name.as_bytes();
        let name_length = u16::try_from(name.len()).map_err(|_| ZipError::NameTooLong)?;
        let size = u32::try_from(entry.bytes.len()).map_err(|_| ZipError::EntryTooLarge)?;
        let offset = u32::try_from(data.len()).map_err(|_| ZipError::ArchiveTooLarge)?;
        let crc = crc32(entry.bytes);

        push_u32(&mut data, LOCAL_HEADER);
        push_u16(&mut data, 20);
        push_u16(&mut data, 0);
        push_u16(&mut data, 0);
        push_u16(&mut data, DOS_TIME);
        push_u16(&mut data, DOS_DATE);
        push_u32(&mut data, crc);
        push_u32(&mut data, size);
        push_u32(&mut data, size);
        push_u16(&mut data, name_length);
        push_u16(&mut data, 0);
        data.extend_from_slice(name);
        data.extend_from_slice(entry.bytes);

        push_u32(&mut directory, CENTRAL_HEADER);
        push_u16(&mut directory, 20);
        push_u16(&mut directory, 20);
        push_u16(&mut directory, 0);
        push_u16(&mut directory, 0);
        push_u16(&mut directory, DOS_TIME);
        push_u16(&mut directory, DOS_DATE);
        push_u32(&mut directory, crc);
        push_u32(&mut directory, size);
        push_u32(&mut directory, size);
        push_u16(&mut directory, name_length);
        push_u16(&mut directory, 0);
        push_u16(&mut directory, 0);
        push_u16(&mut directory, 0);
        push_u16(&mut directory, 0);
        push_u32(&mut directory, 0);
        push_u32(&mut directory, offset);
        directory.extend_from_slice(name);
    }

    let directory_offset = u32::try_from(data.len()).map_err(|_| ZipError::ArchiveTooLarge)?;
    let directory_size = u32::try_from(directory.len()).map_err(|_| ZipError::ArchiveTooLarge)?;
    let count = u16::try_from(entries.len()).map_err(|_| ZipError::TooManyEntries)?;
    let mut archive = data;
    archive.extend_from_slice(&directory);
    push_u32(&mut archive, END_OF_DIRECTORY);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, 0);
    push_u16(&mut archive, count);
    push_u16(&mut archive, count);
    push_u32(&mut archive, directory_size);
    push_u32(&mut archive, directory_offset);
    push_u16(&mut archive, 0);
    Ok(archive)
}

fn push_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

/// The CRC-32 of the ZIP format, computed without a table.
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffff_u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_crc_of_a_known_vector_matches() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
    }

    #[test]
    fn an_archive_starts_with_the_first_local_header_and_ends_with_a_directory() {
        let archive = write_stored(&[
            Entry::new("mimetype", b"a"),
            Entry::new("content.xml", b"bc"),
        ])
        .expect("the archive must be written");

        assert_eq!(&archive[..4], &LOCAL_HEADER.to_le_bytes());
        assert_eq!(
            &archive[archive.len() - 22..archive.len() - 18],
            &END_OF_DIRECTORY.to_le_bytes()
        );
    }

    #[test]
    fn the_same_entries_produce_the_same_bytes() {
        let entries = [Entry::new("a.txt", b"text"), Entry::new("b.txt", b"more")];

        assert_eq!(
            write_stored(&entries).expect("the archive"),
            write_stored(&entries).expect("the archive")
        );
    }
}
