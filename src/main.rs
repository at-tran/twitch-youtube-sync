use regex::Regex;
use reqwest::blocking;
use reqwest::header::{CONTENT_LANGUAGE, CONTENT_LENGTH, CONTENT_TYPE, LOCATION};
use serde::Deserialize;
use serde_json::{json, Value};
use serde_urlencoded;
use std::collections::HashMap;
use std::error::Error;
use std::fs::File;
use std::process::{Command, Stdio};
use std::time::Duration;
use std::{fs, thread};

fn main() {
    let video_id = "567314621";
    let token_filepath = "token.json";
    // let (token, sig) = get_access_token(&get_client_id(), video_id);
    // let m3u8_url = get_m3u8_url(video_id, &token, &sig);
    // download_video(video_id, &m3u8_url);
    let client_secret = get_client_secrets();
    let auth = get_auth_token(&client_secret, token_filepath);
    println!("Auth token: {}", auth.access_token);
    upload_video(&auth.access_token, video_id);
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

#[derive(Deserialize, Debug)]
struct ClientSecret {
    client_id: String,
    client_secret: String,
}

fn get_client_secrets() -> ClientSecret {
    let filepath = "client_secrets.json";
    let file = fs::File::open(filepath).expect(&format!("Cannot find {}", filepath));
    let json: Value =
        serde_json::from_reader(file).expect(&format!("{} does not contain valid json", filepath));
    serde_json::from_value(json["web"].clone()).unwrap()
}

#[derive(Deserialize, Debug)]
struct UserCodeInfo {
    device_code: String,
    user_code: String,
    verification_url: String,
    expires_in: u32,
    interval: u32,
}

#[derive(Deserialize, Debug)]
struct AuthInfo {
    access_token: String,
    expires_in: u32,
    scope: String,
    token_type: String,
    #[serde(default)]
    refresh_token: String,
}

fn get_auth_token(client_secret: &ClientSecret, filepath: &str) -> AuthInfo {
    if let Ok(auth) = get_auth_token_from_file(client_secret, filepath) {
        auth
    } else {
        let auth = get_auth_token_from_server(client_secret);
        let _ = fs::write(
            filepath,
            format!(r#"{{ "refresh_token": "{}" }}"#, auth.refresh_token),
        );
        auth
    }
}

fn get_auth_token_from_server(client_secret: &ClientSecret) -> AuthInfo {
    let client = reqwest::blocking::Client::new();
    let res: UserCodeInfo = client
        .post("https://oauth2.googleapis.com/device/code")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(format!(
            "client_id={}&scope=https://www.googleapis.com/auth/youtube.upload",
            client_secret.client_id
        ))
        .send()
        .unwrap()
        .json()
        .unwrap();

    println!(
        "Please go to {} and enter code: {}",
        res.verification_url, res.user_code
    );

    for _ in 1..(res.expires_in / res.interval) {
        thread::sleep(Duration::from_secs(res.interval as u64));

        let mut res = client
            .post("https://oauth2.googleapis.com/token")
            .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
            .body(format!(
                "client_id={}&\
                client_secret={}&\
                device_code={}&\
                grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code",
                client_secret.client_id, client_secret.client_secret, res.device_code
            ))
            .send()
            .unwrap();

        if res.status().is_success() {
            return res.json().unwrap();
        }
    }

    panic!("User did not enter code");
}

fn get_auth_token_from_file(
    client_secret: &ClientSecret,
    filepath: &str,
) -> Result<AuthInfo, Box<dyn Error>> {
    let file = fs::File::open(filepath)?;
    let json: Value = serde_json::from_reader(file)?;
    let refresh_token = if let Value::String(s) = &json["refresh_token"] {
        s
    } else {
        panic!("refresh_token is must be a string");
    };

    let client = reqwest::blocking::Client::new();
    let res: AuthInfo = client
        .post("https://oauth2.googleapis.com/token")
        .header(CONTENT_TYPE, "application/x-www-form-urlencoded")
        .body(format!(
            "client_id={}&\
                client_secret={}&\
                refresh_token={}&\
                grant_type=refresh_token",
            client_secret.client_id, client_secret.client_secret, refresh_token
        ))
        .send()?
        .json()?;

    Ok(AuthInfo {
        refresh_token: refresh_token.clone(),
        ..res
    })
}

fn upload_video(auth_token: &str, video_id: &str) {
    let filepath = format!("videos/{}.mp4", video_id);
    let metadata = fs::metadata(&filepath).unwrap();

    let upload_uri = get_upload_session_uri(auth_token, video_id, metadata.len() as u32);
    let file = File::open(&filepath).unwrap();

    println!("Starting upload");
    let client = reqwest::blocking::Client::new();
    let res = client
        .post(&upload_uri)
        .bearer_auth(auth_token)
        .header(CONTENT_TYPE, "application/octet-stream")
        .header(CONTENT_LENGTH, metadata.len())
        .timeout(Duration::from_secs(36000))
        .body(file)
        .send();
    println!("{:?}", res);
}

fn get_upload_session_uri(auth_token: &str, video_name: &str, video_size: u32) -> String {
    let client = reqwest::blocking::Client::new();
    let req_body = json!({
      "snippet": {
        "title": video_name,
        "description": "This is a description of my video",
        "tags": ["cool", "video", "more keywords"],
        "categoryId": 20
      },
      "status": {
        "privacyStatus": "private",
      }
    })
    .to_string();
    let res = client
        .post("https://www.googleapis.com/upload/youtube/v3/videos")
        .query(&[
            ("uploadType", "resumable"),
            ("part", "snippet,status,contentDetails"),
        ])
        .bearer_auth(auth_token)
        .header(CONTENT_TYPE, "application/json; charset=UTF-8")
        .header(CONTENT_LENGTH, req_body.len())
        .header("X-Upload-Content-Length", video_size)
        .header("X-Upload-Content-Type", "application/octet-stream")
        .body(req_body)
        .send()
        .unwrap();
    let location = res
        .headers()
        .get(LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    location
}
