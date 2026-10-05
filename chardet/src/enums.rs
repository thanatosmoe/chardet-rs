//! Bit-flag encoding eras, mirroring `chardet.enums.EncodingEra`.

/// Namespace for encoding-era bit flags.
pub struct Era;

impl Era {
    pub const MODERN_WEB: u8 = 1;
    pub const LEGACY_ISO: u8 = 2;
    pub const LEGACY_MAC: u8 = 4;
    pub const LEGACY_REGIONAL: u8 = 8;
    pub const DOS: u8 = 16;
    pub const MAINFRAME: u8 = 32;
    pub const ALL: u8 = Self::MODERN_WEB
        | Self::LEGACY_ISO
        | Self::LEGACY_MAC
        | Self::LEGACY_REGIONAL
        | Self::DOS
        | Self::MAINFRAME;
}

/// Language filter flags for `UniversalDetector` (chardet 6.x compatibility).
///
/// Accepted but unused: the pipeline does not filter by language group.
pub struct LanguageFilter;

impl LanguageFilter {
    pub const CHINESE_SIMPLIFIED: u32 = 0x01;
    pub const CHINESE_TRADITIONAL: u32 = 0x02;
    pub const JAPANESE: u32 = 0x04;
    pub const KOREAN: u32 = 0x08;
    pub const NON_CJK: u32 = 0x10;
    pub const ALL: u32 = 0x1F;
}
