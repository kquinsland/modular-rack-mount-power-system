pub const RECORD_BYTES: usize = 96;
pub const PAYLOAD_BYTES: usize = 72;
pub const COMMIT_OFFSET: usize = RECORD_BYTES - 4;
pub const ERASED_WORD: [u8; 4] = [0xFF; 4];
pub const COMMIT_WORD: [u8; 4] = [0x50, 0x44, 0x43, 0x32];

const MAGIC: [u8; 4] = *b"PDC2";
const FORMAT_VERSION: u8 = 1;
const PAYLOAD_OFFSET: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RecordKind {
    BackplaneConfiguration = 1,
    CarrierConfiguration = 2,
    StagedUpdate = 3,
}

impl TryFrom<u8> for RecordKind {
    type Error = DecodeError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::BackplaneConfiguration),
            2 => Ok(Self::CarrierConfiguration),
            3 => Ok(Self::StagedUpdate),
            _ => Err(DecodeError::InvalidKind(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Payload {
    bytes: [u8; PAYLOAD_BYTES],
    len: u8,
}

impl Payload {
    pub fn new(bytes: &[u8]) -> Result<Self, EncodeError> {
        let len = u8::try_from(bytes.len()).map_err(|_| EncodeError::PayloadTooLong)?;
        if bytes.len() > PAYLOAD_BYTES {
            return Err(EncodeError::PayloadTooLong);
        }
        let mut storage = [0; PAYLOAD_BYTES];
        storage[..bytes.len()].copy_from_slice(bytes);
        Ok(Self {
            bytes: storage,
            len,
        })
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Record {
    pub kind: RecordKind,
    pub generation: u32,
    pub payload: Payload,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EncodeError {
    PayloadTooLong,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DecodeError {
    Uncommitted,
    InvalidMagic,
    InvalidVersion(u8),
    InvalidKind(u8),
    InvalidLength(u8),
    ReservedNonZero,
    Checksum,
}

pub fn encode(record: Record) -> [u8; RECORD_BYTES] {
    let mut bytes = [0; RECORD_BYTES];
    bytes[..4].copy_from_slice(&MAGIC);
    bytes[4] = FORMAT_VERSION;
    bytes[5] = record.kind as u8;
    bytes[6] = record.payload.len;
    bytes[8..12].copy_from_slice(&record.generation.to_le_bytes());
    bytes[PAYLOAD_OFFSET..PAYLOAD_OFFSET + PAYLOAD_BYTES].copy_from_slice(&record.payload.bytes);
    let checksum = checksum(&bytes[..12], &bytes[PAYLOAD_OFFSET..COMMIT_OFFSET]);
    bytes[12..16].copy_from_slice(&checksum.to_le_bytes());
    bytes[COMMIT_OFFSET..].copy_from_slice(&COMMIT_WORD);
    bytes
}

pub fn decode(bytes: &[u8; RECORD_BYTES]) -> Result<Record, DecodeError> {
    if bytes[COMMIT_OFFSET..] != COMMIT_WORD {
        return Err(DecodeError::Uncommitted);
    }
    if bytes[..4] != MAGIC {
        return Err(DecodeError::InvalidMagic);
    }
    if bytes[4] != FORMAT_VERSION {
        return Err(DecodeError::InvalidVersion(bytes[4]));
    }
    if bytes[7] != 0 {
        return Err(DecodeError::ReservedNonZero);
    }
    let len = bytes[6];
    if usize::from(len) > PAYLOAD_BYTES {
        return Err(DecodeError::InvalidLength(len));
    }
    if bytes[PAYLOAD_OFFSET + usize::from(len)..COMMIT_OFFSET]
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(DecodeError::ReservedNonZero);
    }
    let expected = checksum(&bytes[..12], &bytes[PAYLOAD_OFFSET..COMMIT_OFFSET]);
    if read_u32(bytes, 12) != expected {
        return Err(DecodeError::Checksum);
    }
    Ok(Record {
        kind: RecordKind::try_from(bytes[5])?,
        generation: read_u32(bytes, 8),
        payload: Payload::new(&bytes[PAYLOAD_OFFSET..PAYLOAD_OFFSET + usize::from(len)])
            .expect("a decoded payload was already bounds checked"),
    })
}

/// Selects the newest valid record using wrapping generation ordering.
pub fn select_newest(first: &[u8; RECORD_BYTES], second: &[u8; RECORD_BYTES]) -> Option<Record> {
    match (decode(first), decode(second)) {
        (Ok(first), Ok(second)) => Some(
            if generation_is_newer(second.generation, first.generation) {
                second
            } else {
                first
            },
        ),
        (Ok(record), Err(_)) | (Err(_), Ok(record)) => Some(record),
        (Err(_), Err(_)) => None,
    }
}

pub const fn generation_is_newer(candidate: u32, current: u32) -> bool {
    candidate != current && candidate.wrapping_sub(current) < 0x8000_0000
}

fn checksum(header: &[u8], payload: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in header.iter().chain(payload) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(generation: u32) -> Record {
        Record {
            kind: RecordKind::CarrierConfiguration,
            generation,
            payload: Payload::new(&[1, 2, 3, 4]).unwrap(),
        }
    }

    #[test]
    fn record_round_trip_preserves_kind_generation_and_payload() {
        let record = record(17);
        assert_eq!(decode(&encode(record)), Ok(record));
    }

    #[test]
    fn commit_word_is_written_last_and_required() {
        let mut bytes = encode(record(1));
        bytes[COMMIT_OFFSET..].copy_from_slice(&ERASED_WORD);
        assert_eq!(decode(&bytes), Err(DecodeError::Uncommitted));
    }

    #[test]
    fn checksum_rejects_interrupted_or_corrupted_payloads() {
        let mut bytes = encode(record(1));
        bytes[PAYLOAD_OFFSET + 1] ^= 0x80;
        assert_eq!(decode(&bytes), Err(DecodeError::Checksum));
    }

    #[test]
    fn newest_valid_slot_survives_generation_wrap() {
        let first = encode(record(u32::MAX));
        let second = encode(record(0));
        assert_eq!(select_newest(&first, &second), Some(record(0)));
    }
}
