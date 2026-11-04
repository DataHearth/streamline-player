use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Capabilities {
    pub video: Vec<VideoCap>,
    pub hdr: HdrCaps,
    pub audio: AudioCaps,
    pub containers: Vec<String>,
    pub subtitles: Vec<String>,
    pub hls_keyframe_exact: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VideoCap {
    pub codec: String,
    pub profiles: Vec<String>,
    pub max_level: Option<u32>,
    pub max_width: u32,
    pub max_height: u32,
    pub max_fps: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HdrCaps {
    pub formats: Vec<String>,
    pub dv_profiles: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioCaps {
    pub codecs: Vec<String>,
    pub max_channels: u32,
    pub passthrough: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = r#"{
      "video": [
        { "codec": "hevc", "profiles": ["main", "main10"], "max_level": 153,
          "max_width": 3840, "max_height": 2160, "max_fps": 60 }
      ],
      "hdr": { "formats": ["hdr10", "hdr10plus", "hlg", "dolby_vision"], "dv_profiles": [5, 8] },
      "audio": { "codecs": ["aac", "ac3", "eac3", "flac", "opus", "truehd"], "max_channels": 6, "passthrough": ["eac3"] },
      "containers": ["mkv", "mp4"],
      "subtitles": ["webvtt", "srt", "ass", "pgs"],
      "hls_keyframe_exact": false
    }"#;

    #[test]
    fn caps_reject_unknown_field() {
        let caps: Capabilities = serde_json::from_str(EXAMPLE).unwrap();
        assert_eq!(caps.video[0].max_level, Some(153));

        let mut v: serde_json::Value = serde_json::from_str(EXAMPLE).unwrap();
        v["foo"] = 1.into();
        assert!(serde_json::from_value::<Capabilities>(v).is_err());

        let mut v: serde_json::Value = serde_json::from_str(EXAMPLE).unwrap();
        v["video"][0]["foo"] = 1.into();
        assert!(serde_json::from_value::<Capabilities>(v).is_err());
    }
}
