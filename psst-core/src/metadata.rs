use crate::{
    audio_file::{AudioFile, AudioPath},
    error::Error,
    item_id::{FileId, ItemId, ItemIdType},
    protocol::metadata::{Restriction, Track},
    session::SessionHandle,
};
use quick_protobuf::MessageRead;
use std::time::Duration;

pub trait Fetch: MessageRead<'static> {
    fn uri(id: ItemId) -> String;
    fn fetch(session: &SessionHandle, id: ItemId) -> Result<Self, Error> {
        session.connected()?.get_mercury_protobuf(Self::uri(id))
    }
}

impl Fetch for Track {
    fn uri(id: ItemId) -> String {
        format!("hm://metadata/3/track/{}", id.to_base16())
    }
}

pub trait ToAudioPath {
    fn is_restricted_in_region(&self, country: &str) -> bool;
    fn find_allowed_alternative(&self, country: &str) -> Option<ItemId>;
    fn to_audio_path(&self, preferred_bitrate: usize) -> Option<AudioPath>;
}

impl ToAudioPath for Track {
    fn is_restricted_in_region(&self, country: &str) -> bool {
        self.restriction
            .iter()
            .any(|rest| is_restricted_in_region(rest, country))
    }

    fn find_allowed_alternative(&self, country: &str) -> Option<ItemId> {
        let alt_track = self
            .alternative
            .iter()
            .find(|alt_track| !alt_track.is_restricted_in_region(country))?;
        ItemId::from_raw(alt_track.gid.as_ref()?, ItemIdType::Track)
    }

    fn to_audio_path(&self, preferred_bitrate: usize) -> Option<AudioPath> {
        let file = AudioFile::compatible_audio_formats(preferred_bitrate)
            .iter()
            .find_map(|&preferred_format| {
                self.file
                    .iter()
                    .find(|file| file.format == Some(preferred_format))
            })?;
        let file_format = file.format?;
        let item_id = ItemId::from_raw(self.gid.as_ref()?, ItemIdType::Track)?;
        let file_id = FileId::from_raw(file.file_id.as_ref()?)?;
        let duration = Duration::from_millis(self.duration? as u64);
        Some(AudioPath {
            item_id,
            file_id,
            file_format,
            duration,
        })
    }
}

fn is_restricted_in_region(restriction: &Restriction, country: &str) -> bool {
    if let Some(allowed) = &restriction.countries_allowed {
        if allowed.is_empty() {
            return true;
        }
        if is_country_in_list(allowed.as_bytes(), country.as_bytes()) {
            return false;
        }
    }
    if let Some(forbidden) = &restriction.countries_forbidden {
        if is_country_in_list(forbidden.as_bytes(), country.as_bytes()) {
            return true;
        }
    }
    false
}

fn is_country_in_list(countries: &[u8], country: &[u8]) -> bool {
    countries.chunks(2).any(|code| code == country)
}
