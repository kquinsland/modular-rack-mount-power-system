use pdcan_drivers::config_record::{
    COMMIT_MARKER, COMMIT_OFFSET, ConfigRecord, JournalSlot, RECORD_SIZE, SelectedRecord,
    encode_uncommitted, next_slot, select_newest,
};
use pdcan_types::{PersistentSettings, SettingsValidationError};

pub const REGION_SIZE: u32 = 8 * 1024;
pub const SLOT_SIZE: u32 = REGION_SIZE / 2;
pub const SLOT_A_OFFSET: u32 = 0;
pub const SLOT_B_OFFSET: u32 = SLOT_SIZE;
const COMMIT_OFFSET_U32: u32 = 104;

const _: () = assert!(RECORD_SIZE < SLOT_SIZE as usize);
const _: () = assert!(COMMIT_OFFSET == COMMIT_OFFSET_U32 as usize);

pub trait FlashStorage {
    type Error;

    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error>;
    fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error>;
    fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StoreError<E> {
    Storage(E),
    InvalidSettings(SettingsValidationError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SaveOutcome {
    Written { slot: JournalSlot, sequence: u32 },
    Unchanged { slot: JournalSlot, sequence: u32 },
}

pub struct ConfigStore<S> {
    storage: S,
    selected: Option<SelectedRecord>,
}

impl<S: FlashStorage> ConfigStore<S> {
    pub const fn new(storage: S) -> Self {
        Self {
            storage,
            selected: None,
        }
    }

    pub fn load(&mut self) -> Result<Option<ConfigRecord>, StoreError<S::Error>> {
        let mut slot_a = [u8::MAX; RECORD_SIZE];
        let mut slot_b = [u8::MAX; RECORD_SIZE];
        self.storage
            .read(SLOT_A_OFFSET, &mut slot_a)
            .map_err(StoreError::Storage)?;
        self.storage
            .read(SLOT_B_OFFSET, &mut slot_b)
            .map_err(StoreError::Storage)?;
        self.selected = select_newest(&slot_a, &slot_b);
        Ok(self.selected.map(|selected| selected.record))
    }

    pub fn save(
        &mut self,
        settings: PersistentSettings,
    ) -> Result<SaveOutcome, StoreError<S::Error>> {
        settings.validate().map_err(StoreError::InvalidSettings)?;
        if let Some(selected) = self.selected
            && selected.record.settings == settings
        {
            return Ok(SaveOutcome::Unchanged {
                slot: selected.slot,
                sequence: selected.record.sequence,
            });
        }

        let sequence = self
            .selected
            .map_or(0, |selected| selected.record.sequence.wrapping_add(1));
        let slot = next_slot(self.selected.map(|selected| selected.slot));
        let offset = slot_offset(slot);
        let record = encode_uncommitted(sequence, settings);

        self.storage
            .erase(offset, offset + SLOT_SIZE)
            .map_err(StoreError::Storage)?;
        self.storage
            .write(offset, &record[..COMMIT_OFFSET])
            .map_err(StoreError::Storage)?;
        self.storage
            .write(offset + COMMIT_OFFSET_U32, &COMMIT_MARKER)
            .map_err(StoreError::Storage)?;

        self.selected = Some(SelectedRecord {
            slot,
            record: ConfigRecord { sequence, settings },
        });
        Ok(SaveOutcome::Written { slot, sequence })
    }

    pub fn into_inner(self) -> S {
        self.storage
    }
}

pub const fn slot_offset(slot: JournalSlot) -> u32 {
    match slot {
        JournalSlot::A => SLOT_A_OFFSET,
        JournalSlot::B => SLOT_B_OFFSET,
    }
}

#[cfg(feature = "firmware-bin")]
pub mod stm32 {
    use embassy_stm32::flash::{self, Blocking, Flash};

    use super::{FlashStorage, REGION_SIZE};
    use crate::board::{CONFIG_FLASH_LENGTH, CONFIG_FLASH_START};

    const FLASH_BASE: u32 = 0x0800_0000;
    const CONFIG_OFFSET: u32 = CONFIG_FLASH_START - FLASH_BASE;

    const _: () = assert!(CONFIG_FLASH_LENGTH == REGION_SIZE);
    const _: () = assert!(embassy_stm32::flash::MAX_ERASE_SIZE == 2 * 1024);
    const _: () = assert!(embassy_stm32::flash::WRITE_SIZE == 8);
    const _: () = assert!(CONFIG_OFFSET.is_multiple_of(4 * 1024));

    pub struct Stm32ConfigFlash<'d> {
        flash: Flash<'d, Blocking>,
    }

    impl<'d> Stm32ConfigFlash<'d> {
        pub fn new(peripheral: embassy_stm32::Peri<'d, embassy_stm32::peripherals::FLASH>) -> Self {
            Self {
                flash: Flash::new_blocking(peripheral),
            }
        }
    }

    impl FlashStorage for Stm32ConfigFlash<'_> {
        type Error = flash::Error;

        fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
            self.flash.blocking_read(CONFIG_OFFSET + offset, bytes)
        }

        fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
            self.flash
                .blocking_erase(CONFIG_OFFSET + from, CONFIG_OFFSET + to)
        }

        fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
            self.flash.blocking_write(CONFIG_OFFSET + offset, bytes)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pdcan_types::NodeId;
    use std::boxed::Box;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FakeError {
        Injected,
        OutOfBounds,
        InvalidWrite,
    }

    struct FakeFlash {
        bytes: Box<[u8; REGION_SIZE as usize]>,
        fail_write_number: Option<u32>,
        write_count: u32,
    }

    impl FakeFlash {
        fn erased() -> Self {
            Self {
                bytes: Box::new([u8::MAX; REGION_SIZE as usize]),
                fail_write_number: None,
                write_count: 0,
            }
        }

        fn fail_next_write(&mut self) {
            self.fail_write_number = Some(self.write_count + 1);
        }
    }

    impl FlashStorage for FakeFlash {
        type Error = FakeError;

        fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), Self::Error> {
            let start = usize::try_from(offset).map_err(|_| FakeError::OutOfBounds)?;
            let end = start
                .checked_add(bytes.len())
                .ok_or(FakeError::OutOfBounds)?;
            let source = self.bytes.get(start..end).ok_or(FakeError::OutOfBounds)?;
            bytes.copy_from_slice(source);
            Ok(())
        }

        fn erase(&mut self, from: u32, to: u32) -> Result<(), Self::Error> {
            let start = usize::try_from(from).map_err(|_| FakeError::OutOfBounds)?;
            let end = usize::try_from(to).map_err(|_| FakeError::OutOfBounds)?;
            self.bytes
                .get_mut(start..end)
                .ok_or(FakeError::OutOfBounds)?
                .fill(u8::MAX);
            Ok(())
        }

        fn write(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Self::Error> {
            self.write_count += 1;
            if self.fail_write_number == Some(self.write_count) {
                return Err(FakeError::Injected);
            }
            let start = usize::try_from(offset).map_err(|_| FakeError::OutOfBounds)?;
            let end = start
                .checked_add(bytes.len())
                .ok_or(FakeError::OutOfBounds)?;
            let destination = self
                .bytes
                .get_mut(start..end)
                .ok_or(FakeError::OutOfBounds)?;
            if destination
                .iter()
                .zip(bytes)
                .any(|(old, new)| old & new != *new)
            {
                return Err(FakeError::InvalidWrite);
            }
            for (destination, source) in destination.iter_mut().zip(bytes) {
                *destination &= *source;
            }
            Ok(())
        }
    }

    fn settings(node: u8, emergency_latched: bool) -> PersistentSettings {
        PersistentSettings {
            node_id: Some(NodeId::new(node).unwrap()),
            emergency_latched,
            ..PersistentSettings::FACTORY_DEFAULT
        }
    }

    #[test]
    fn invalid_flash_loads_as_unprovisioned() {
        let mut store = ConfigStore::new(FakeFlash::erased());
        assert_eq!(store.load(), Ok(None));
    }

    #[test]
    fn saves_alternate_slots_and_skip_unchanged_settings() {
        let mut store = ConfigStore::new(FakeFlash::erased());
        store.load().unwrap();
        assert_eq!(
            store.save(settings(1, false)),
            Ok(SaveOutcome::Written {
                slot: JournalSlot::A,
                sequence: 0,
            })
        );
        assert_eq!(
            store.save(settings(1, false)),
            Ok(SaveOutcome::Unchanged {
                slot: JournalSlot::A,
                sequence: 0,
            })
        );
        assert_eq!(
            store.save(settings(1, true)),
            Ok(SaveOutcome::Written {
                slot: JournalSlot::B,
                sequence: 1,
            })
        );
    }

    #[test]
    fn interrupted_payload_write_preserves_previous_committed_slot() {
        let mut store = ConfigStore::new(FakeFlash::erased());
        store.load().unwrap();
        store.save(settings(1, false)).unwrap();
        store.storage.fail_next_write();
        assert_eq!(
            store.save(settings(2, true)),
            Err(StoreError::Storage(FakeError::Injected))
        );

        let mut rebooted = ConfigStore::new(store.into_inner());
        assert_eq!(
            rebooted.load().unwrap().unwrap().settings,
            settings(1, false)
        );
    }

    #[test]
    fn interrupted_commit_write_preserves_previous_committed_slot() {
        let mut store = ConfigStore::new(FakeFlash::erased());
        store.load().unwrap();
        store.save(settings(1, false)).unwrap();
        store.storage.fail_write_number = Some(store.storage.write_count + 2);
        assert_eq!(
            store.save(settings(2, true)),
            Err(StoreError::Storage(FakeError::Injected))
        );

        let mut rebooted = ConfigStore::new(store.into_inner());
        assert_eq!(
            rebooted.load().unwrap().unwrap().settings,
            settings(1, false)
        );
    }

    #[test]
    fn emergency_latch_survives_a_complete_reconstruction() {
        let mut store = ConfigStore::new(FakeFlash::erased());
        store.load().unwrap();
        store.save(settings(7, true)).unwrap();

        let mut rebooted = ConfigStore::new(store.into_inner());
        let loaded = rebooted.load().unwrap().unwrap();
        assert_eq!(loaded.settings.node_id, Some(NodeId::new(7).unwrap()));
        assert!(loaded.settings.emergency_latched);
    }

    #[test]
    fn invalid_settings_are_rejected_before_flash_is_modified() {
        let mut store = ConfigStore::new(FakeFlash::erased());
        store.load().unwrap();
        let mut invalid = settings(1, false);
        invalid.fan.duty_percent = 101;

        assert_eq!(
            store.save(invalid),
            Err(StoreError::InvalidSettings(SettingsValidationError::Fan(
                101
            )))
        );
        assert_eq!(store.storage.write_count, 0);
    }
}
