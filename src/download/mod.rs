use crate::video::Video;
use regex::Regex;
use reqwest::blocking::{self, Client};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};

pub fn download_twitch_video<P: AsRef<Path>>(video_id: &str, save_folder: P) -> Video {
    let path = save_folder.as_ref().join(video_id).with_extension("mp4");

    if !path.exists() {
        let (token, sig) = get_access_token(&get_client_id(), video_id);
        let m3u8_url = get_m3u8_url(video_id, &token, &sig);
        download_video_from_m3u8(&m3u8_url, &path);
    }

    let size = fs::metadata(&path)
        .unwrap_or_else(|_| panic!("Downloading {} failed", path.to_string_lossy()))
        .len();

    Video {
        name: video_id.to_string(),
        description: "This is my description".to_string(),
        path,
        size,
    }
}

fn get_client_id() -> String {
    let resp = blocking::get("https://www.twitch.tv/").unwrap();
    let text = resp.text().unwrap();
    let re = Regex::new(r#""Client-ID":"(.*?)""#).unwrap();
    let caps = re.captures(&text).unwrap();
    caps.get(1).unwrap().as_str().to_string()
}

fn get_access_token(client_id: &str, video_id: &str) -> (String, String) {
    let client = Client::new();
    let mut resp = client
        .get(&format!(
            "https://api.twitch.tv/api/vods/{}/access_token",
            video_id
        ))
        .query(&[("client_id", client_id)])
        .send()
        .unwrap()
        .json::<HashMap<String, String>>()
        .unwrap();
    (resp.remove("token").unwrap(), resp.remove("sig").unwrap())
}

fn get_m3u8_url(video_id: &str, token: &str, sig: &str) -> String {
    format!(
        "https://usher.ttvnw.net/vod/{}.m3u8?&{}",
        video_id,
        serde_urlencoded::to_string(&[("allow_source", "true"), ("token", token), ("sig", sig)])
            .unwrap()
    )
}

fn download_video_from_m3u8(m3u8_url: &str, path: &Path) {
    let _ = fs::create_dir_all(path.parent().expect("Video save path cannot be root"));
    Command::new("ffmpeg")
        .args(&[
            "-i",
            m3u8_url,
            "-c",
            "copy",
            "-bsf:a",
            "aac_adtstoasc",
            "-crf",
            "17",
            "-y",
            path.to_str().unwrap(),
        ])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .expect("failed to execute ffmpeg");
}
