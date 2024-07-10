use rspotify::{Device, RepeatState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub name: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    pub name: String,
    pub artists: Vec<Artist>,
    pub image_url: Option<String>,
    pub url: Option<String>,
    pub duration: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrentlyPlaying {
    pub device: Device,
    pub track: Track,
    pub progress_secs: u32,
    pub shuffled: bool,
    pub playing: bool,
    pub repeat_status: RepeatState,
}

// Holds the structs from the `rspotify` package. It's easier to just copy the structs because it
// saves space and there are some issues with using this package on the esp
mod rspotify {

    use serde::{Deserialize, Serialize};
    use strum::IntoStaticStr;

    /// Device object
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub struct Device {
        pub id: Option<String>,
        pub is_active: bool,
        pub is_private_session: bool,
        pub is_restricted: bool,
        pub name: String,
        #[serde(rename = "type")]
        pub _type: DeviceType,
        pub volume_percent: Option<u32>,
    }

    /// Device Type: `computer`, `smartphone`, `speaker`, `TV`
    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, IntoStaticStr)]
    #[strum(serialize_all = "snake_case")]
    pub enum DeviceType {
        Computer,
        Tablet,
        Smartphone,
        Smartwatch,
        Speaker,
        /// Though undocumented, it has been reported that the Web API returns both
        /// 'Tv' and 'TV' as the type.
        #[serde(alias = "TV")]
        Tv,
        /// Same as above, the Web API returns both 'AVR' and 'Avr' as the type.
        #[serde(alias = "AVR")]
        Avr,
        /// Same as above, the Web API returns both 'STB' and 'Stb' as the type.
        #[serde(alias = "STB")]
        Stb,
        AudioDongle,
        GameConsole,
        CastVideo,
        CastAudio,
        Automobile,
        Unknown,
    }

    /// Repeat state: `track`, `context` or `off`.
    #[derive(Clone, Debug, Copy, Serialize, Deserialize, PartialEq, Eq, IntoStaticStr)]
    #[serde(rename_all = "snake_case")]
    #[strum(serialize_all = "snake_case")]
    pub enum RepeatState {
        Off,
        Track,
        Context,
    }
}
