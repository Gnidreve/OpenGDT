//! Tags, Value Representations und das (bewusst kleine) Datenwörterbuch.

use std::fmt;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Tag(pub u16, pub u16);

impl fmt::Debug for Tag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "({:04X},{:04X})", self.0, self.1)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Vr(pub [u8; 2]);

impl Vr {
    pub const AE: Vr = Vr(*b"AE");
    pub const AS: Vr = Vr(*b"AS");
    pub const CS: Vr = Vr(*b"CS");
    pub const DA: Vr = Vr(*b"DA");
    pub const DS: Vr = Vr(*b"DS");
    pub const IS: Vr = Vr(*b"IS");
    pub const LO: Vr = Vr(*b"LO");
    pub const OB: Vr = Vr(*b"OB");
    pub const OW: Vr = Vr(*b"OW");
    pub const PN: Vr = Vr(*b"PN");
    pub const SH: Vr = Vr(*b"SH");
    pub const SQ: Vr = Vr(*b"SQ");
    pub const TM: Vr = Vr(*b"TM");
    pub const UI: Vr = Vr(*b"UI");
    pub const UL: Vr = Vr(*b"UL");
    pub const UN: Vr = Vr(*b"UN");
    pub const US: Vr = Vr(*b"US");

    /// Explizites VR mit 2 reservierten Bytes + 4-Byte-Länge.
    pub fn has_long_length(self) -> bool {
        matches!(
            &self.0,
            b"OB"
                | b"OW"
                | b"OF"
                | b"OD"
                | b"OL"
                | b"OV"
                | b"SQ"
                | b"UN"
                | b"UC"
                | b"UR"
                | b"UT"
                | b"SV"
                | b"UV"
        )
    }

    pub fn is_text(self) -> bool {
        matches!(
            &self.0,
            b"AE"
                | b"AS"
                | b"CS"
                | b"DA"
                | b"DS"
                | b"DT"
                | b"IS"
                | b"LO"
                | b"LT"
                | b"PN"
                | b"SH"
                | b"ST"
                | b"TM"
                | b"UI"
                | b"UC"
                | b"UR"
                | b"UT"
        )
    }

    pub fn pad_byte(self) -> u8 {
        if self == Vr::UI || self.is_binary() {
            0
        } else {
            b' '
        }
    }

    fn is_binary(self) -> bool {
        !self.is_text()
    }

    pub fn is_valid(self) -> bool {
        self.0.iter().all(|b| b.is_ascii_uppercase())
    }
}

macro_rules! tags {
    ($($name:ident = $g:expr, $e:expr, $vr:ident;)*) => {
        pub mod t {
            use super::Tag;
            $(pub const $name: Tag = Tag($g, $e);)*
        }
        const DICT: &[(Tag, Vr)] = &[$((Tag($g, $e), Vr::$vr),)*];
    };
}

tags! {
    // Command (Gruppe 0000) – immer Implicit VR LE
    COMMAND_GROUP_LENGTH = 0x0000, 0x0000, UL;
    AFFECTED_SOP_CLASS_UID = 0x0000, 0x0002, UI;
    COMMAND_FIELD = 0x0000, 0x0100, US;
    MESSAGE_ID = 0x0000, 0x0110, US;
    MESSAGE_ID_RESPONDED_TO = 0x0000, 0x0120, US;
    PRIORITY = 0x0000, 0x0700, US;
    COMMAND_DATA_SET_TYPE = 0x0000, 0x0800, US;
    STATUS = 0x0000, 0x0900, US;
    ERROR_COMMENT = 0x0000, 0x0902, LO;
    AFFECTED_SOP_INSTANCE_UID = 0x0000, 0x1000, UI;
    // File Meta (Gruppe 0002)
    META_GROUP_LENGTH = 0x0002, 0x0000, UL;
    META_VERSION = 0x0002, 0x0001, OB;
    META_SOP_CLASS_UID = 0x0002, 0x0002, UI;
    META_SOP_INSTANCE_UID = 0x0002, 0x0003, UI;
    META_TRANSFER_SYNTAX = 0x0002, 0x0010, UI;
    META_IMPL_CLASS_UID = 0x0002, 0x0012, UI;
    META_IMPL_VERSION_NAME = 0x0002, 0x0013, SH;
    // Allgemein
    SPECIFIC_CHARACTER_SET = 0x0008, 0x0005, CS;
    IMAGE_TYPE = 0x0008, 0x0008, CS;
    SOP_CLASS_UID = 0x0008, 0x0016, UI;
    SOP_INSTANCE_UID = 0x0008, 0x0018, UI;
    STUDY_DATE = 0x0008, 0x0020, DA;
    CONTENT_DATE = 0x0008, 0x0023, DA;
    STUDY_TIME = 0x0008, 0x0030, TM;
    CONTENT_TIME = 0x0008, 0x0033, TM;
    ACCESSION_NUMBER = 0x0008, 0x0050, SH;
    MODALITY = 0x0008, 0x0060, CS;
    REFERRING_PHYSICIAN = 0x0008, 0x0090, PN;
    STUDY_DESCRIPTION = 0x0008, 0x1030, LO;
    SERIES_DESCRIPTION = 0x0008, 0x103E, LO;
    // Patient
    PATIENT_NAME = 0x0010, 0x0010, PN;
    PATIENT_ID = 0x0010, 0x0020, LO;
    PATIENT_BIRTH_DATE = 0x0010, 0x0030, DA;
    PATIENT_SEX = 0x0010, 0x0040, CS;
    // Bild-/Cine-Parameter
    CINE_RATE = 0x0018, 0x0040, IS;
    FRAME_TIME = 0x0018, 0x1063, DS;
    STUDY_INSTANCE_UID = 0x0020, 0x000D, UI;
    SERIES_INSTANCE_UID = 0x0020, 0x000E, UI;
    STUDY_ID = 0x0020, 0x0010, SH;
    SERIES_NUMBER = 0x0020, 0x0011, IS;
    INSTANCE_NUMBER = 0x0020, 0x0013, IS;
    SAMPLES_PER_PIXEL = 0x0028, 0x0002, US;
    PHOTOMETRIC = 0x0028, 0x0004, CS;
    PLANAR_CONFIGURATION = 0x0028, 0x0006, US;
    NUMBER_OF_FRAMES = 0x0028, 0x0008, IS;
    ROWS = 0x0028, 0x0010, US;
    COLUMNS = 0x0028, 0x0011, US;
    BITS_ALLOCATED = 0x0028, 0x0100, US;
    BITS_STORED = 0x0028, 0x0101, US;
    HIGH_BIT = 0x0028, 0x0102, US;
    PIXEL_REPRESENTATION = 0x0028, 0x0103, US;
    // Worklist
    SPS_STATION_AE = 0x0040, 0x0001, AE;
    SPS_START_DATE = 0x0040, 0x0002, DA;
    SPS_START_TIME = 0x0040, 0x0003, TM;
    SPS_PERFORMING_PHYSICIAN = 0x0040, 0x0006, PN;
    SPS_DESCRIPTION = 0x0040, 0x0007, LO;
    SPS_ID = 0x0040, 0x0009, SH;
    SPS_STATION_NAME = 0x0040, 0x0010, SH;
    SPS_LOCATION = 0x0040, 0x0011, SH;
    SPS_STATUS = 0x0040, 0x0020, CS;
    SPS_SEQUENCE = 0x0040, 0x0100, SQ;
    REQUESTED_PROCEDURE_ID = 0x0040, 0x1001, SH;
    REQUESTED_PROCEDURE_PRIORITY = 0x0040, 0x1003, CS;
    REQUESTING_PHYSICIAN = 0x0032, 0x1032, PN;
    REQUESTED_PROCEDURE_DESCRIPTION = 0x0032, 0x1060, LO;
    PIXEL_DATA = 0x7FE0, 0x0010, OW;
}

/// VR eines Tags laut Wörterbuch; unbekannte Tags → `UN`.
pub fn vr_of(tag: Tag) -> Vr {
    DICT.iter()
        .find(|(t, _)| *t == tag)
        .map(|(_, v)| *v)
        .unwrap_or(Vr::UN)
}

pub const ITEM: Tag = Tag(0xFFFE, 0xE000);
pub const ITEM_DELIM: Tag = Tag(0xFFFE, 0xE00D);
pub const SEQ_DELIM: Tag = Tag(0xFFFE, 0xE0DD);
