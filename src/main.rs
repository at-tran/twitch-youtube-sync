use regex::Regex;
use reqwest::blocking;
use reqwest::header::CONTENT_TYPE;
use serde_json::{json, Value};
use serde_urlencoded;
use std::collections::HashMap;
use std::fs;
use std::process::{Command, Stdio};

fn main() {
    let video_id = "567314621";
    // let (token, sig) = get_access_token(&get_client_id(), video_id);
    // let m3u8_url = get_m3u8_url(video_id, &token, &sig);
    // download_video(video_id, &m3u8_url);
    let (client_id, client_secret) = get_client_secrets();
    let auth_token = get_auth_token(&client_id);
    // get_upload_session_uri(&api_key, &video_id);
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
    let _ = fs::create_dir("videos");
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
            &format!("videos/{}.mp4", video_id),
        ])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .status()
        .unwrap();
}

fn get_client_secrets() -> (String, String) {
    let filepath = "client_secrets.json";
    let file = fs::File::open(filepath).expect(&format!("Cannot find {}", filepath));
    let json: Value =
        serde_json::from_reader(file).expect(&format!("{} does not contain valid json", filepath));

    (
        get_string_from_value(&json["web"]["client_id"]),
        get_string_from_value(&json["web"]["client_secret"]),
    )
}

fn get_string_from_value(v: &Value) -> String {
    if let Value::String(s) = &v {
        s.clone()
    } else {
        panic!("{} is not a string", v)
    }
}

fn get_auth_token(client_id: &str) -> String {
    let client = reqwest::blocking::Client::new();
    let res = client
        .post("https://oauth2.googleapis.com/device/code")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(format!(
            "client_id={}&scope=https://www.googleapis.com/auth/youtube.upload",
            client_id
        ))
        .send()
        .unwrap()
        .text()
        .unwrap();
    println!("{:?}", res);
    "haha".to_string()
}

fn get_upload_session_uri(api_key: &str, video_id: &str) {
    let client = reqwest::blocking::Client::new();
    let req_body = json!({
      "snippet": {
        "title": video_id,
        "description": "This is a description of my video",
        "tags": ["cool", "video", "more keywords"],
        "categoryId": 22
      },
      "status": {
        "privacyStatus": "unlisted",
        "embeddable": true,
        "license": "youtube"
      }
    })
    .to_string();
    let res = client
        .post("https://www.googleapis.com/upload/youtube/v3/videos")
        .bearer_auth(api_key)
        .header(CONTENT_TYPE, "application/json; charset=UTF-8")
        .header("X-Upload-Content-Length", "300")
        .header("X-Upload-Content-Type", "video/*")
        .body(req_body)
        .send();
    println!("{:?}", res);
}
