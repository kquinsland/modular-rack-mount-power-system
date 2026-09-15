pub const FAMILY_CODE: u8 = 0x28;
pub const MAX_CONVERSION_TIME_MS: u16 = 750;

pub mod command {
    pub const SEARCH_ROM: u8 = 0xF0;
    pub const MATCH_ROM: u8 = 0x55;
    pub const SKIP_ROM: u8 = 0xCC;
    pub const CONVERT_T: u8 = 0x44;
    pub const READ_SCRATCHPAD: u8 = 0xBE;
}

#[allow(async_fn_in_trait)]
pub trait OneWireBus {
    type Error;

    /// Reset the bus and return true when at least one presence pulse is seen.
    async fn reset(&mut self) -> Result<bool, Self::Error>;
    async fn write_bit(&mut self, bit: bool) -> Result<(), Self::Error>;
    async fn read_bit(&mut self) -> Result<bool, Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RomCode([u8; 8]);

impl RomCode {
    pub fn from_bytes(bytes: [u8; 8]) -> Result<Self, RomError> {
        if bytes[0] != FAMILY_CODE {
            return Err(RomError::WrongFamily(bytes[0]));
        }
        if crc8(&bytes[..7]) != bytes[7] {
            return Err(RomError::Crc);
        }
        Ok(Self(bytes))
    }

    pub const fn as_bytes(&self) -> &[u8; 8] {
        &self.0
    }

    pub const fn identifier(self) -> u64 {
        u64::from_le_bytes(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RomError {
    WrongFamily(u8),
    Crc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error<E> {
    Bus(E),
    NoPresence,
    SearchProtocol,
    Rom(RomError),
    Scratchpad(ScratchpadError),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScratchpadError {
    Crc,
    TemperatureOutOfRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SearchState {
    rom: [u8; 8],
    last_discrepancy: u8,
    complete: bool,
}

impl SearchState {
    pub const fn new() -> Self {
        Self {
            rom: [0; 8],
            last_discrepancy: 0,
            complete: false,
        }
    }

    pub const fn is_complete(&self) -> bool {
        self.complete
    }

    pub fn restart(&mut self) {
        *self = Self::new();
    }

    pub async fn next<B: OneWireBus>(
        &mut self,
        bus: &mut B,
    ) -> Result<Option<RomCode>, Error<B::Error>> {
        if self.complete {
            return Ok(None);
        }
        if !bus.reset().await.map_err(Error::Bus)? {
            return Err(Error::NoPresence);
        }
        write_byte(bus, command::SEARCH_ROM).await?;

        let mut last_zero = 0;
        for bit_number in 1_u8..=64 {
            let bit = bus.read_bit().await.map_err(Error::Bus)?;
            let complement = bus.read_bit().await.map_err(Error::Bus)?;
            if bit && complement {
                return Err(Error::SearchProtocol);
            }
            let direction = if bit != complement {
                bit
            } else if bit_number < self.last_discrepancy {
                get_bit(self.rom, bit_number)
            } else {
                bit_number == self.last_discrepancy
            };
            if !direction && !bit && !complement {
                last_zero = bit_number;
            }
            set_bit(&mut self.rom, bit_number, direction);
            bus.write_bit(direction).await.map_err(Error::Bus)?;
        }

        self.last_discrepancy = last_zero;
        self.complete = last_zero == 0;
        RomCode::from_bytes(self.rom).map(Some).map_err(Error::Rom)
    }
}

impl Default for SearchState {
    fn default() -> Self {
        Self::new()
    }
}

pub async fn start_conversion_all<B: OneWireBus>(bus: &mut B) -> Result<(), Error<B::Error>> {
    if !bus.reset().await.map_err(Error::Bus)? {
        return Err(Error::NoPresence);
    }
    write_byte(bus, command::SKIP_ROM).await?;
    write_byte(bus, command::CONVERT_T).await
}

/// Reads one selected sensor after the caller has allowed the externally
/// powered conversion to complete.
pub async fn read_temperature_centi_c<B: OneWireBus>(
    bus: &mut B,
    rom: RomCode,
) -> Result<i16, Error<B::Error>> {
    if !bus.reset().await.map_err(Error::Bus)? {
        return Err(Error::NoPresence);
    }
    write_byte(bus, command::MATCH_ROM).await?;
    for byte in rom.as_bytes() {
        write_byte(bus, *byte).await?;
    }
    write_byte(bus, command::READ_SCRATCHPAD).await?;
    let mut scratchpad = [0; 9];
    for byte in &mut scratchpad {
        *byte = read_byte(bus).await?;
    }
    decode_scratchpad_centi_c(scratchpad).map_err(Error::Scratchpad)
}

pub fn decode_scratchpad_centi_c(scratchpad: [u8; 9]) -> Result<i16, ScratchpadError> {
    if crc8(&scratchpad[..8]) != scratchpad[8] {
        return Err(ScratchpadError::Crc);
    }
    let raw = i16::from_le_bytes([scratchpad[0], scratchpad[1]]);
    let scaled = i32::from(raw) * 25;
    let centi_c = if scaled >= 0 {
        (scaled + 2) / 4
    } else {
        (scaled - 2) / 4
    };
    i16::try_from(centi_c).map_err(|_| ScratchpadError::TemperatureOutOfRange)
}

pub fn crc8(bytes: &[u8]) -> u8 {
    let mut crc = 0_u8;
    for byte in bytes {
        let mut input = *byte;
        for _ in 0..8 {
            let mix = (crc ^ input) & 1;
            crc >>= 1;
            if mix != 0 {
                crc ^= 0x8C;
            }
            input >>= 1;
        }
    }
    crc
}

async fn write_byte<B: OneWireBus>(bus: &mut B, byte: u8) -> Result<(), Error<B::Error>> {
    for bit in 0..8 {
        bus.write_bit(byte & (1 << bit) != 0)
            .await
            .map_err(Error::Bus)?;
    }
    Ok(())
}

async fn read_byte<B: OneWireBus>(bus: &mut B) -> Result<u8, Error<B::Error>> {
    let mut byte = 0;
    for bit in 0..8 {
        if bus.read_bit().await.map_err(Error::Bus)? {
            byte |= 1 << bit;
        }
    }
    Ok(byte)
}

fn get_bit(rom: [u8; 8], bit_number: u8) -> bool {
    let index = bit_number - 1;
    rom[usize::from(index / 8)] & (1 << (index % 8)) != 0
}

fn set_bit(rom: &mut [u8; 8], bit_number: u8, value: bool) {
    let index = bit_number - 1;
    let mask = 1 << (index % 8);
    let byte = &mut rom[usize::from(index / 8)];
    if value {
        *byte |= mask;
    } else {
        *byte &= !mask;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn datasheet_rom_example_passes_crc_and_preserves_all_64_bits() {
        let bytes = [0x28, 0xFF, 0x15, 0x8A, 0x74, 0x16, 0x04, 0x72];
        let rom = RomCode::from_bytes(bytes).unwrap();
        assert_eq!(rom.as_bytes(), &bytes);
        assert_eq!(rom.identifier(), u64::from_le_bytes(bytes));
    }

    #[test]
    fn scratchpad_crc_and_positive_temperature_are_checked() {
        let scratchpad = [0x50, 0x05, 0x4B, 0x46, 0x7F, 0xFF, 0x0C, 0x10, 0x1C];
        assert_eq!(crc8(&scratchpad[..8]), scratchpad[8]);
        assert_eq!(decode_scratchpad_centi_c(scratchpad), Ok(8_500));
    }

    #[test]
    fn negative_temperature_rounds_symmetrically() {
        let mut scratchpad = [0; 9];
        let raw = (-162_i16).to_le_bytes(); // -10.125 C
        scratchpad[0] = raw[0];
        scratchpad[1] = raw[1];
        scratchpad[8] = crc8(&scratchpad[..8]);
        assert_eq!(decode_scratchpad_centi_c(scratchpad), Ok(-1_013));
    }

    #[test]
    fn rom_family_and_crc_are_both_mandatory() {
        assert!(matches!(
            RomCode::from_bytes([0; 8]),
            Err(RomError::WrongFamily(0))
        ));
        assert_eq!(
            RomCode::from_bytes([0x28, 1, 2, 3, 4, 5, 6, 7]),
            Err(RomError::Crc)
        );
    }
}
