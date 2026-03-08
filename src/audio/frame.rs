use crate::audio::buffer::PooledBuffer;

#[derive(Debug)]
pub enum AudioFrame {
    Pcm(PooledBuffer),
    Opus(Vec<u8>),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_frame_variants() {
        let pcm = AudioFrame::Pcm(vec![0, 1, 2]);
        let opus = AudioFrame::Opus(vec![3, 4, 5]);

        match pcm {
            AudioFrame::Pcm(data) => assert_eq!(data, vec![0, 1, 2]),
            _ => panic!("Expected Pcm variant"),
        }

        match opus {
            AudioFrame::Opus(data) => assert_eq!(data, vec![3, 4, 5]),
            _ => panic!("Expected Opus variant"),
        }
    }
}
