use regex::Regex;
use reqwest::blocking;
use serde_urlencoded;
use std::collections::HashMap;
use std::process::{Command, Stdio};

fn main() {
    let video_id = "566138609";
    let (token, sig) = get_access_token(&get_client_id(), video_id);
    println!("{}\n{}", token, sig);
    let m3u8_url = get_m3u8_url(video_id, &token, &sig);
    println!("{}", m3u8_url);
    download_video(video_id, &m3u8_url);
}

fn get_client_id() -> String {
    let resp = blocking::get("https://www.twitch.tv/").unwrap();
    let text = resp.text().unwrap();
    let re = Regex::new(r#""Client-ID":"(.*?)""#).unwrap();
    let caps = re.captures(&text).unwrap();
    caps.get(1).unwrap().as_str().to_string()
}

fn get_access_token(client_id: &str, video_id: &str) -> (String, String) {
    let client = reqwest::blocking::Client::new();
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

fn download_video(video_id: &str, m3u8_url: &str) {
    Command::new("ffmpeg")
        .args(&[
            "-i",
            m3u8_url,
            "-c",
            "copy",
            "-bsf:a",
            "aac_adtstoasc",
            &format!("{}.mp4", video_id),
        ])
        .stdout(Stdio::inherit())
        .output()
        .unwrap();
}
