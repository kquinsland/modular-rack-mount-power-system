use pdcan_types::{
    CarrierProfile, FirmwareVersion, HardwareRevision, ImageTarget, NodeRole, Sha256Digest,
    UpdateManifest,
};
use sha2::{Digest as _, Sha256};

pub const MAGIC: &[u8; 8] = b"PDCANFW\0";
pub const FORMAT_VERSION: u16 = 1;
pub const HEADER_BYTES: usize = 160;
pub const MAX_KEY_ID_BYTES: usize = 16;
pub const MAX_SIGNATURE_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SignatureAlgorithm {
    Unsigned = 0,
    Ed25519 = 1,
}

impl TryFrom<u8> for SignatureAlgorithm {
    type Error = BundleError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Unsigned),
            1 => Ok(Self::Ed25519),
            _ => Err(BundleError::SignatureAlgorithm(value)),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SignatureRecord<'a> {
    pub algorithm: SignatureAlgorithm,
    pub key_id: &'a [u8],
    pub signature: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Bundle<'a> {
    pub manifest: UpdateManifest,
    pub signature: SignatureRecord<'a>,
    pub image: &'a [u8],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundleError {
    TooShort,
    Magic,
    FormatVersion(u16),
    HeaderLength(u16),
    Length,
    Role(u8),
    CarrierProfile(u8),
    HardwareRevision(u8),
    ImageTarget,
    SignatureAlgorithm(u8),
    SignatureShape,
    Reserved,
    Digest,
    ImageTooLarge,
}

impl<'a> Bundle<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, BundleError> {
        if bytes.len() < HEADER_BYTES {
            return Err(BundleError::TooShort);
        }
        if &bytes[..8] != MAGIC {
            return Err(BundleError::Magic);
        }
        let format = read_u16(bytes, 8);
        if format != FORMAT_VERSION {
            return Err(BundleError::FormatVersion(format));
        }
        let header_length = read_u16(bytes, 10);
        if usize::from(header_length) != HEADER_BYTES {
            return Err(BundleError::HeaderLength(header_length));
        }

        let role = NodeRole::try_from(bytes[12]).map_err(|_| BundleError::Role(bytes[12]))?;
        let carrier_profile = if bytes[13] == 0 {
            None
        } else {
            Some(
                CarrierProfile::try_from(bytes[13])
                    .map_err(|_| BundleError::CarrierProfile(bytes[13]))?,
            )
        };
        let hardware_revision = HardwareRevision::new(bytes[14])
            .map_err(|_| BundleError::HardwareRevision(bytes[14]))?;
        let target = ImageTarget {
            role,
            carrier_profile,
            hardware_revision,
            partition_layout: bytes[15],
        };
        target.validate().map_err(|_| BundleError::ImageTarget)?;

        let algorithm = SignatureAlgorithm::try_from(bytes[22])?;
        let key_id_length = usize::from(bytes[23]);
        let image_length = usize::try_from(read_u32(bytes, 24)).map_err(|_| BundleError::Length)?;
        let signature_length = usize::from(read_u16(bytes, 28));
        if key_id_length > MAX_KEY_ID_BYTES || signature_length > MAX_SIGNATURE_BYTES {
            return Err(BundleError::SignatureShape);
        }
        if bytes[30..32].iter().any(|byte| *byte != 0)
            || bytes[64 + key_id_length..80].iter().any(|byte| *byte != 0)
            || bytes[80 + signature_length..144]
                .iter()
                .any(|byte| *byte != 0)
            || bytes[144..HEADER_BYTES].iter().any(|byte| *byte != 0)
        {
            return Err(BundleError::Reserved);
        }
        match algorithm {
            SignatureAlgorithm::Unsigned if key_id_length == 0 && signature_length == 0 => {}
            SignatureAlgorithm::Ed25519 if key_id_length > 0 && signature_length == 64 => {}
            _ => return Err(BundleError::SignatureShape),
        }
        if bytes.len() != HEADER_BYTES + image_length {
            return Err(BundleError::Length);
        }

        let mut digest = [0; 32];
        digest.copy_from_slice(&bytes[32..64]);
        let bundle = Self {
            manifest: UpdateManifest {
                target,
                version: FirmwareVersion {
                    major: read_u16(bytes, 16),
                    minor: read_u16(bytes, 18),
                    patch: read_u16(bytes, 20),
                },
                image_size: read_u32(bytes, 24),
                digest: Sha256Digest::from_bytes(digest),
            },
            signature: SignatureRecord {
                algorithm,
                key_id: &bytes[64..64 + key_id_length],
                signature: &bytes[80..80 + signature_length],
            },
            image: &bytes[HEADER_BYTES..],
        };
        bundle.verify_digest()?;
        Ok(bundle)
    }

    pub fn verify_digest(&self) -> Result<(), BundleError> {
        let actual: [u8; 32] = Sha256::digest(self.image).into();
        if actual == *self.manifest.digest.as_bytes() {
            Ok(())
        } else {
            Err(BundleError::Digest)
        }
    }
}

pub fn build_unsigned(manifest: UpdateManifest, image: &[u8]) -> Result<Vec<u8>, BundleError> {
    manifest
        .target
        .validate()
        .map_err(|_| BundleError::ImageTarget)?;
    let image_size = u32::try_from(image.len()).map_err(|_| BundleError::ImageTooLarge)?;
    if manifest.image_size != image_size {
        return Err(BundleError::Length);
    }
    let actual: [u8; 32] = Sha256::digest(image).into();
    if actual != *manifest.digest.as_bytes() {
        return Err(BundleError::Digest);
    }

    let mut output = vec![0; HEADER_BYTES + image.len()];
    output[..8].copy_from_slice(MAGIC);
    output[8..10].copy_from_slice(&FORMAT_VERSION.to_le_bytes());
    output[10..12].copy_from_slice(
        &u16::try_from(HEADER_BYTES)
            .expect("bundle header fits u16")
            .to_le_bytes(),
    );
    output[12] = manifest.target.role as u8;
    output[13] = manifest
        .target
        .carrier_profile
        .map_or(0, |profile| profile as u8);
    output[14] = manifest.target.hardware_revision.get();
    output[15] = manifest.target.partition_layout;
    output[16..18].copy_from_slice(&manifest.version.major.to_le_bytes());
    output[18..20].copy_from_slice(&manifest.version.minor.to_le_bytes());
    output[20..22].copy_from_slice(&manifest.version.patch.to_le_bytes());
    output[22] = SignatureAlgorithm::Unsigned as u8;
    output[24..28].copy_from_slice(&image_size.to_le_bytes());
    output[32..64].copy_from_slice(manifest.digest.as_bytes());
    output[HEADER_BYTES..].copy_from_slice(image);
    Ok(output)
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

    fn manifest(image: &[u8]) -> UpdateManifest {
        let digest: [u8; 32] = Sha256::digest(image).into();
        UpdateManifest {
            target: ImageTarget {
                role: NodeRole::Carrier,
                carrier_profile: Some(CarrierProfile::Sw3538),
                hardware_revision: HardwareRevision::REV_A,
                partition_layout: 1,
            },
            version: FirmwareVersion {
                major: 1,
                minor: 2,
                patch: 3,
            },
            image_size: u32::try_from(image.len()).unwrap(),
            digest: Sha256Digest::from_bytes(digest),
        }
    }

    #[test]
    fn unsigned_bundle_round_trip_preserves_manifest_and_image() {
        let image = b"application image";
        let expected = manifest(image);
        let bytes = build_unsigned(expected, image).unwrap();
        let parsed = Bundle::parse(&bytes).unwrap();
        assert_eq!(parsed.manifest, expected);
        assert_eq!(parsed.image, image);
        assert_eq!(parsed.signature.algorithm, SignatureAlgorithm::Unsigned);
    }

    #[test]
    fn corruption_is_detected_before_staging() {
        let image = b"application image";
        let mut bytes = build_unsigned(manifest(image), image).unwrap();
        *bytes.last_mut().unwrap() ^= 1;
        assert_eq!(Bundle::parse(&bytes), Err(BundleError::Digest));
    }

    #[test]
    fn signature_fields_are_reserved_for_ed25519_without_enforcing_it_yet() {
        let image = b"signed later";
        let mut bytes = build_unsigned(manifest(image), image).unwrap();
        bytes[22] = SignatureAlgorithm::Ed25519 as u8;
        bytes[23] = 3;
        bytes[28..30].copy_from_slice(&64_u16.to_le_bytes());
        bytes[64..67].copy_from_slice(b"dev");
        bytes[80..144].fill(0xA5);
        let parsed = Bundle::parse(&bytes).unwrap();
        assert_eq!(parsed.signature.algorithm, SignatureAlgorithm::Ed25519);
        assert_eq!(parsed.signature.key_id, b"dev");
        assert_eq!(parsed.signature.signature.len(), 64);
    }
}
