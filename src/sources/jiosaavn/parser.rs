use serde_json::Value;

use crate::protocol::tracks::{PlaylistData, PlaylistInfo, Track, TrackInfo};

pub fn parse_track(v: &Value) -> Option<Track> {
    let id = v.get("id").and_then(|v| {
        v.as_str()
            .map(|s| s.to_owned())
            .or_else(|| v.as_i64().map(|i| i.to_string()))
    })?;
    let title = super::helpers::clean_string(
        v.get("title")
            .or_else(|| v.get("song"))
            .and_then(|v| v.as_str())?,
    );
    let duration = v
        .pointer("/more_info/duration")
        .or_else(|| v.get("duration"))
        .and_then(|v| v.as_str())
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| v.pointer("/more_info/duration").and_then(|v| v.as_u64()))
        .or_else(|| v.get("duration").and_then(|v| v.as_u64()))
        .unwrap_or(0);

    let artwork_url = v
        .get("image")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.replace("150x150", "500x500").replace("50x50", "500x500"));

    let artists_str = v
        .pointer("/more_info/artistMap/primary_artists")
        .and_then(|v| v.as_array())
        .filter(|arr| !arr.is_empty())
        .or_else(|| {
            v.pointer("/more_info/artistMap/artists")
                .and_then(|v| v.as_array())
        })
        .map(|arr| {
            arr.iter()
                .filter_map(|a| a.get("name").and_then(|v| v.as_str()))
                .take(3)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .or_else(|| {
            v.pointer("/more_info/music")
                .or_else(|| v.get("subtitle"))
                .or_else(|| v.get("primary_artists"))
                .or_else(|| v.get("singers"))
                .or_else(|| v.get("header_desc"))
                .and_then(|v| v.as_str())
                .map(|s| {
                    s.split(',')
                        .map(|part| part.trim())
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(", ")
                })
        })
        .unwrap_or_else(|| "Unknown Artist".to_owned());

    let author = super::helpers::clean_string(&artists_str);

    let mut track = Track::new(TrackInfo {
        title,
        author,
        length: duration * 1000,
        identifier: id,
        source_name: "jiosaavn".to_owned(),
        uri: v
            .get("perma_url")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_owned()),
        artwork_url,
        is_stream: false,
        is_seekable: true,
        ..Default::default()
    });

    track.plugin_info = serde_json::json!({
        "albumName": v.get("album").or_else(|| v.pointer("/more_info/album")).and_then(|v| v.as_str()),
        "albumUrl": v.get("album_url").or_else(|| v.pointer("/more_info/album_url")).and_then(|v| v.as_str()),
        "artistUrl": v.pointer("/more_info/artistMap/primary_artists/0/perma_url").and_then(|v| v.as_str()),
        "artistArtworkUrl": v.pointer("/more_info/artistMap/primary_artists/0/image").and_then(|v| v.as_str()).map(|s| s.replace("150x150", "500x500").replace("50x50", "500x500")),
        "previewUrl": v.get("media_preview_url").or_else(|| v.pointer("/more_info/media_preview_url")).or_else(|| v.get("vlink")).or_else(|| v.pointer("/more_info/vlink")).and_then(|v| v.as_str()),
        "isPreview": false
    });

    Some(track)
}

pub fn parse_search_item(v: &Value) -> Option<Track> {
    let id = v.get("id").and_then(|v| v.as_str())?;
    let title = super::helpers::clean_string(v.get("title").and_then(|v| v.as_str())?);
    let artwork_url = v
        .get("image")
        .and_then(|v| v.as_str())
        .map(|s| s.replace("150x150", "500x500").replace("50x50", "500x500"));

    let author_str = v
        .get("subtitle")
        .or_else(|| v.get("description"))
        .and_then(|v| v.as_str())
        .map(|s| {
            s.split(',')
                .map(|part| part.trim())
                .take(3)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .unwrap_or_else(|| "Unknown Artist".to_owned());
    let author = super::helpers::clean_string(&author_str);

    let mut track = Track::new(TrackInfo {
        title,
        author,
        length: 0,
        identifier: id.to_owned(),
        source_name: "jiosaavn".to_owned(),
        uri: v.get("url").and_then(|v| v.as_str()).map(|s| s.to_owned()),
        artwork_url,
        is_stream: false,
        is_seekable: true,
        ..Default::default()
    });

    track.plugin_info = serde_json::json!({
        "albumName": v.get("album").and_then(|v| v.as_str()),
        "previewUrl": v.get("vlink").or_else(|| v.pointer("/more_info/vlink")).and_then(|v| v.as_str()),
        "isPreview": true
    });

    Some(track)
}

pub fn parse_search_playlist(v: &Value, type_: &str) -> Option<PlaylistData> {
    let title = super::helpers::clean_string(v.get("title").and_then(|v| v.as_str())?);
    let artwork_url = v
        .get("image")
        .and_then(|v| v.as_str())
        .map(|s| s.replace("150x150", "500x500").replace("50x50", "500x500"));

    let mut url = v
        .get("url")
        .or_else(|| v.get("perma_url"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();

    if url.is_empty() {
        url = v
            .get("perma_url")
            .or_else(|| v.get("permaurl"))
            .or_else(|| v.get("token"))
            .or_else(|| v.pointer("/more_info/perma_url"))
            .or_else(|| v.pointer("/more_info/token"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_owned();
    }

    if url.is_empty()
        && let Some(id) = v.get("id").and_then(|v| v.as_str())
    {
        if id.starts_with('/') || id.starts_with("http") {
            url = id.to_owned();
        } else {
            let path_type = match type_ {
                "playlist" => "s/playlist",
                "featured" => "featured",
                "album" => "album",
                "artist" => "artist",
                _ => type_,
            };
            let slug = title
                .to_lowercase()
                .replace(|c: char| !c.is_alphanumeric() && c != ' ', "")
                .replace(' ', "-");
            url = format!("/{path_type}/{slug}/{id}");
        }
    }

    if !url.is_empty() && !url.starts_with("http") {
        url = format!("https://www.jiosaavn.com{url}");
    }

    let total_tracks = v
        .pointer("/more_info/song_count")
        .or_else(|| v.pointer("/more_info/track_count"))
        .or_else(|| v.get("song_count"))
        .or_else(|| v.get("track_count"))
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse::<u64>().ok())
                .or_else(|| v.as_u64())
        })
        .unwrap_or(0);

    let author_raw = v
        .pointer("/more_info/artist_name")
        .or_else(|| v.pointer("/more_info/music"))
        .or_else(|| v.get("music"))
        .or_else(|| v.get("subtitle"))
        .or_else(|| v.get("description"))
        .and_then(|v| v.as_str())
        .map(|s| {
            s.split(',')
                .map(|part| part.trim())
                .take(3)
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty());

    let final_author = if type_ == "artist" {
        title.clone()
    } else {
        author_raw.unwrap_or_else(|| "Unknown Author".to_owned())
    };

    Some(PlaylistData {
        info: PlaylistInfo {
            name: title,
            selected_track: -1,
        },
        plugin_info: serde_json::json!({
            "url": url,
            "type": type_,
            "artworkUrl": artwork_url,
            "author": super::helpers::clean_string(&final_author),
            "totalTracks": total_tracks
        }),
        tracks: Vec::new(),
    })
}
