use pdcan_types::{
    FanConfig, FanMode, MAX_PORTS, NodeId, PersistentSettings, PolicyValidationError, PortPolicy,
};

pub const RECORD_SIZE: usize = 112;
pub const WRITE_GRANULARITY: usize = 8;
pub const COMMIT_OFFSET: usize = RECORD_SIZE - WRITE_GRANULARITY;
pub const COMMIT_MARKER: [u8; WRITE_GRANULARITY] = *b"PDC_DONE";

const MAGIC: [u8; 4] = *b"PDC1";
const HEADER_SIZE: usize = 16;
const PAYLOAD_SIZE: usize = 80;
const CRC_OFFSET: usize = HEADER_SIZE + PAYLOAD_SIZE;
const POLICY_SIZE: usize = 9;
const POLICY_OFFSET: usize = HEADER_SIZE + 8;
const PAYLOAD_SIZE_U16: u16 = 80;

const _: () = assert!(RECORD_SIZE.is_multiple_of(WRITE_GRANULARITY));
const _: () = assert!(COMMIT_OFFSET.is_multiple_of(WRITE_GRANULARITY));
const _: () = assert!(POLICY_OFFSET + POLICY_SIZE * MAX_PORTS == CRC_OFFSET);
const _: () = assert!(PAYLOAD_SIZE == PAYLOAD_SIZE_U16 as usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConfigRecord {
    pub sequence: u32,
    pub settings: PersistentSettings,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordDecodeError {
    WrongLength,
    NotCommitted,
    BadMagic,
    UnsupportedFormat(u16),
    WrongPayloadLength(u16),
    Integrity,
    InvalidNodeId(u8),
    InvalidFanMode(u8),
    InvalidFanDuty(u8),
    InvalidEmergencyLatch(u8),
    InvalidPolicyFlags {
        port: u8,
        flags: u8,
    },
    InvalidPolicy {
        port: u8,
        error: PolicyValidationError,
    },
    ReservedData,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalSlot {
    A,
    B,
}

impl JournalSlot {
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SelectedRecord {
    pub slot: JournalSlot,
    pub record: ConfigRecord,
}

pub fn encode_uncommitted(sequence: u32, settings: PersistentSettings) -> [u8; RECORD_SIZE] {
    let mut record = [u8::MAX; RECORD_SIZE];
    record[..4].copy_from_slice(&MAGIC);
    record[4..6].copy_from_slice(&PersistentSettings::FORMAT_VERSION.to_le_bytes());
    record[6..8].copy_from_slice(&PAYLOAD_SIZE_U16.to_le_bytes());
    record[8..12].copy_from_slice(&sequence.to_le_bytes());
    record[12..16].fill(0);

    record[16] = settings.node_id.map_or(0, NodeId::get);
    record[17] = settings.fan.mode as u8;
    record[18] = settings.fan.duty_percent;
    record[19] = u8::from(settings.emergency_latched);
    record[20..24].fill(0);

    for (index, policy) in settings.port_policy.iter().enumerate() {
        let offset = POLICY_OFFSET + index * POLICY_SIZE;
        record[offset] = u8::from(policy.enabled);
        record[offset + 1..offset + 3].copy_from_slice(&policy.max_voltage_mv.to_le_bytes());
        record[offset + 3..offset + 5].copy_from_slice(&policy.max_current_ma.to_le_bytes());
        record[offset + 5..offset + 9].copy_from_slice(&policy.max_power_mw.to_le_bytes());
    }

    let crc = crc32(&record[..CRC_OFFSET]);
    record[CRC_OFFSET..CRC_OFFSET + 4].copy_from_slice(&crc.to_le_bytes());
    record
}

pub fn mark_committed(record: &mut [u8; RECORD_SIZE]) {
    record[COMMIT_OFFSET..].copy_from_slice(&COMMIT_MARKER);
}

pub fn decode(record: &[u8]) -> Result<ConfigRecord, RecordDecodeError> {
    if record.len() != RECORD_SIZE {
        return Err(RecordDecodeError::WrongLength);
    }
    if record[COMMIT_OFFSET..] != COMMIT_MARKER {
        return Err(RecordDecodeError::NotCommitted);
    }
    if record[..4] != MAGIC {
        return Err(RecordDecodeError::BadMagic);
    }

    let format = read_u16(record, 4);
    if format != PersistentSettings::FORMAT_VERSION {
        return Err(RecordDecodeError::UnsupportedFormat(format));
    }
    let payload_length = read_u16(record, 6);
    if usize::from(payload_length) != PAYLOAD_SIZE {
        return Err(RecordDecodeError::WrongPayloadLength(payload_length));
    }
    if record[12..16] != [0; 4]
        || record[20..24] != [0; 4]
        || record[CRC_OFFSET + 4..COMMIT_OFFSET]
            .iter()
            .any(|byte| *byte != u8::MAX)
    {
        return Err(RecordDecodeError::ReservedData);
    }

    let expected_crc = read_u32(record, CRC_OFFSET);
    if crc32(&record[..CRC_OFFSET]) != expected_crc {
        return Err(RecordDecodeError::Integrity);
    }

    let node_id = match record[16] {
        0 => None,
        raw => Some(NodeId::new(raw).map_err(|_| RecordDecodeError::InvalidNodeId(raw))?),
    };
    let fan_mode = match record[17] {
        value if value == FanMode::ThreeWire as u8 => FanMode::ThreeWire,
        value if value == FanMode::FourWire as u8 => FanMode::FourWire,
        value => return Err(RecordDecodeError::InvalidFanMode(value)),
    };
    let fan = FanConfig {
        mode: fan_mode,
        duty_percent: record[18],
    };
    fan.validate()
        .map_err(|error| RecordDecodeError::InvalidFanDuty(error.0))?;
    let emergency_latched = match record[19] {
        0 => false,
        1 => true,
        value => return Err(RecordDecodeError::InvalidEmergencyLatch(value)),
    };

    let mut port_policy = [PortPolicy::SAFE_DISABLED; MAX_PORTS];
    for (index, policy) in port_policy.iter_mut().enumerate() {
        let port = u8::try_from(index).expect("MAX_PORTS fits in a u8");
        let offset = POLICY_OFFSET + index * POLICY_SIZE;
        let flags = record[offset];
        if flags & !1 != 0 {
            return Err(RecordDecodeError::InvalidPolicyFlags { port, flags });
        }
        *policy = PortPolicy {
            enabled: flags & 1 != 0,
            max_voltage_mv: read_u16(record, offset + 1),
            max_current_ma: read_u16(record, offset + 3),
            max_power_mw: read_u32(record, offset + 5),
        };
        policy
            .validate()
            .map_err(|error| RecordDecodeError::InvalidPolicy { port, error })?;
    }

    Ok(ConfigRecord {
        sequence: read_u32(record, 8),
        settings: PersistentSettings {
            node_id,
            fan,
            port_policy,
            emergency_latched,
        },
    })
}

pub fn select_newest(a: &[u8], b: &[u8]) -> Option<SelectedRecord> {
    let a = decode(a).ok().map(|record| SelectedRecord {
        slot: JournalSlot::A,
        record,
    });
    let b = decode(b).ok().map(|record| SelectedRecord {
        slot: JournalSlot::B,
        record,
    });

    match (a, b) {
        (None, None) => None,
        (Some(record), None) | (None, Some(record)) => Some(record),
        (Some(a), Some(b)) => {
            if sequence_is_newer(b.record.sequence, a.record.sequence) {
                Some(b)
            } else {
                Some(a)
            }
        }
    }
}

pub const fn next_slot(current: Option<JournalSlot>) -> JournalSlot {
    match current {
        Some(slot) => slot.other(),
        None => JournalSlot::A,
    }
}

pub const fn sequence_is_newer(candidate: u32, current: u32) -> bool {
    let distance = candidate.wrapping_sub(current);
    distance != 0 && distance < 0x8000_0000
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = 0u32.wrapping_sub(crc & 1);
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
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

    fn populated_settings() -> PersistentSettings {
        let mut settings = PersistentSettings::FACTORY_DEFAULT;
        settings.node_id = Some(NodeId::new(37).unwrap());
        settings.fan = FanConfig {
            mode: FanMode::FourWire,
            duty_percent: 63,
        };
        settings.emergency_latched = true;
        for (index, policy) in settings.port_policy.iter_mut().enumerate() {
            let index_u16 = u16::try_from(index).unwrap();
            let index_u32 = u32::try_from(index).unwrap();
            *policy = PortPolicy {
                enabled: index % 2 == 0,
                max_voltage_mv: 5_000 + index_u16 * 1_000,
                max_current_ma: 1_000 + index_u16 * 100,
                max_power_mw: 20_000 + index_u32 * 1_000,
            };
        }
        settings
    }

    fn committed(sequence: u32, settings: PersistentSettings) -> [u8; RECORD_SIZE] {
        let mut record = encode_uncommitted(sequence, settings);
        mark_committed(&mut record);
        record
    }

    #[test]
    fn round_trip_preserves_all_eight_policies() {
        let settings = populated_settings();
        let record = committed(0x1234_5678, settings);
        assert_eq!(
            decode(&record),
            Ok(ConfigRecord {
                sequence: 0x1234_5678,
                settings,
            })
        );
    }

    #[test]
    fn an_interrupted_write_without_commit_marker_is_rejected() {
        let record = encode_uncommitted(1, populated_settings());
        assert_eq!(decode(&record), Err(RecordDecodeError::NotCommitted));
    }

    #[test]
    fn corruption_is_detected_even_when_the_marker_is_present() {
        let mut record = committed(1, populated_settings());
        record[POLICY_OFFSET + 5] ^= 0x40;
        assert_eq!(decode(&record), Err(RecordDecodeError::Integrity));
    }

    #[test]
    fn newest_valid_slot_survives_an_interrupted_replacement() {
        let old = committed(41, populated_settings());
        let replacement = encode_uncommitted(42, PersistentSettings::FACTORY_DEFAULT);
        assert_eq!(
            select_newest(&old, &replacement).unwrap().record.sequence,
            41
        );
    }

    #[test]
    fn sequence_selection_handles_wraparound() {
        let old = committed(u32::MAX, populated_settings());
        let new = committed(0, PersistentSettings::FACTORY_DEFAULT);
        let selected = select_newest(&old, &new).unwrap();
        assert_eq!(selected.slot, JournalSlot::B);
        assert_eq!(selected.record.sequence, 0);
    }

    #[test]
    fn invalid_payload_values_are_rejected_after_integrity_validation() {
        let mut record = encode_uncommitted(1, populated_settings());
        record[18] = 101;
        let crc = crc32(&record[..CRC_OFFSET]);
        record[CRC_OFFSET..CRC_OFFSET + 4].copy_from_slice(&crc.to_le_bytes());
        mark_committed(&mut record);
        assert_eq!(decode(&record), Err(RecordDecodeError::InvalidFanDuty(101)));
    }

    #[test]
    fn journal_alternates_slots_and_starts_at_a() {
        assert_eq!(next_slot(None), JournalSlot::A);
        assert_eq!(next_slot(Some(JournalSlot::A)), JournalSlot::B);
        assert_eq!(next_slot(Some(JournalSlot::B)), JournalSlot::A);
    }

    #[test]
    fn every_byte_boundary_of_a_torn_replacement_preserves_the_old_record() {
        let old_settings = populated_settings();
        let old = committed(10, old_settings);
        let replacement = committed(11, PersistentSettings::FACTORY_DEFAULT);

        for written_prefix in 0..RECORD_SIZE {
            let mut torn = [u8::MAX; RECORD_SIZE];
            torn[..written_prefix].copy_from_slice(&replacement[..written_prefix]);
            let selected = select_newest(&old, &torn).expect("old record remains valid");
            assert_eq!(
                selected.record.settings, old_settings,
                "torn prefix length {written_prefix} became authoritative"
            );
        }

        assert_eq!(
            select_newest(&old, &replacement).unwrap().record.sequence,
            11
        );
    }
}
