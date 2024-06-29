use rspotify::model::{Device, RepeatState};

#[derive(Debug, Clone, Serialize)]
pub struct Artist {
    name: String,
    url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Track {
    name: String,
    artists: Vec<Artist>,
    image_url: Option<String>,
    url: Option<String>,
    duration: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CurrentlyPlaying {
    device: Device,
    track: Track,
    progress_secs: u32,
    shuffled: bool,
    playing: bool,
    repeat_status: RepeatState,
}

