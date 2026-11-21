const MICROPHONE: u32 = 1;
const CAMERA: u32 = 2;
const SCREEN_AUDIO: u32 = 4;
const SCREEN_VIDEO: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(clippy::struct_excessive_bools)]
pub struct MediaAccess {
    pub microphone: bool,
    pub camera: bool,
    pub screen: bool,
}

impl MediaAccess {
    pub fn restrict_mask(self, requested: u32) -> u32 {
        let mut allowed = 0;
        if self.microphone {
            allowed |= MICROPHONE;
        }
        if self.camera {
            allowed |= CAMERA;
        }
        if self.screen {
            allowed |= SCREEN_AUDIO | SCREEN_VIDEO;
        }
        requested & allowed
    }
}

#[cfg(test)]
mod tests {
    use super::MediaAccess;

    #[test]
    fn screen_sharing_covers_audio_and_video() {
        let screen_only = MediaAccess {
            screen: true,
            ..MediaAccess::default()
        };
        assert_eq!(screen_only.restrict_mask(15), 12);
    }

    #[test]
    fn keeps_only_allowed_devices() {
        let microphone_only = MediaAccess {
            microphone: true,
            ..MediaAccess::default()
        };
        assert_eq!(microphone_only.restrict_mask(3), 1);
        assert_eq!(MediaAccess::default().restrict_mask(15), 0);
    }
}
