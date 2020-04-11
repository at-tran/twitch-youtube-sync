use crate::video::Video;
use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_LENGTH, CONTENT_TYPE, LOCATION};
use serde::Deserialize;
use serde_json::{json, Value};
use std::error::Error;
use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;
use std::{fs, thread};

pub struct UploadSession {
    video: Video,
    auth_token: String,
    upload_uri: String,
    client: Client,
}

impl UploadSession {
    pub fn new<P: AsRef<Path>>(video: Video, token_filepath: P) -> UploadSession {
        let auth_token = get_auth_token(token_filepath.as_ref());
        let upload_uri = UploadSession::get_upload_session_uri(&video, &auth_token);

        UploadSession {
            video,
            auth_token,
            upload_uri,
            client: Client::new(),
        }
    }

    pub fn upload(&self) {
        // println!("Starting upload with URI: {}", upload_uri);
        self.start_upload();
        // println!("{:?}", res);

        loop {
            let upload_status = self.check_upload_status();

            match upload_status.status().as_u16() {
                308 => {
                    let mut continue_index: u64 = 0;
                    if let Some(range) = upload_status.headers().get("Range") {
                        continue_index = range.to_str().unwrap()[8..].parse::<u64>().unwrap() + 1;
                    }
                    println!("{:?}", upload_status);
                    println!(
                        "Upload interrupted. Resuming from byte {}/{}.",
                        continue_index, self.video.size
                    );
                    self.resume_upload(continue_index);
                }
                200 | 201 => {
                    println!("{:?}", upload_status.text());
                    println!("Upload successful.");
                    break;
                }
                _ => {
                    println!("{:?}", upload_status.text());
                    println!("Upload failed.");
                    break;
                }
            }
        }
    }

    fn check_upload_status(&self) -> Response {
        self.client
            .post(&self.upload_uri)
            .bearer_auth(&self.auth_token)
            .header(CONTENT_LENGTH, 0)
            .header("Content-Range", &format!("bytes */{}", self.video.size))
            .send()
            .unwrap()
    }

    fn start_upload(&self) {
        let mut file = File::open(&self.video.path).unwrap();

        let _ = self
            .client
            .put(&self.upload_uri)
            .bearer_auth(&self.auth_token)
            .header(CONTENT_LENGTH, self.video.size)
            .header(CONTENT_TYPE, "application/octet-stream")
            .body(file)
            .send();
    }

    fn resume_upload(&self, start_index: u64) {
        let mut file = File::open(&self.video.path).unwrap();
        file.seek(SeekFrom::Start(start_index)).unwrap();

        let _ = self
            .client
            .put(&self.upload_uri)
            .bearer_auth(&self.auth_token)
            .header(CONTENT_LENGTH, self.video.size - start_index)
            .header(
                "Content-Range",
                &format!(
                    "bytes {}-{}/{}",
                    start_index,
                    self.video.size - 1,
                    self.video.size
                ),
            )
            .body(file)
            .send();
    }

    fn get_upload_session_uri(video: &Video, auth_token: &str) -> String {
        let client = Client::new();
        let req_body = json!({
          "snippet": {
            "title": &video.name,
            "description": &video.description,
            "tags": ["gaming", "twitch", "live stream"],
            "categoryId": 20 // Gaming
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
            .header("X-Upload-Content-Length", video.size)
            .header("X-Upload-Content-Type", "application/octet-stream")
            .body(req_body)
            .send()
            .unwrap();
        res.headers()
            .get(LOCATION)
            .unwrap()
            .to_str()
            .unwrap()
            .to_string()
    }
}

#[derive(Deserialize, Debug)]
struct ClientSecret {
    client_id: String,
    client_secret: String,
}

fn get_client_secrets() -> ClientSecret {
    let filepath = "client_secrets.json";
    let file = fs::File::open(filepath).unwrap_or_else(|_| panic!("Cannot find {}", filepath));
    let json: Value = serde_json::from_reader(file)
        .unwrap_or_else(|_| panic!("{} does not contain valid json", filepath));
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

fn get_auth_token(filepath: &Path) -> String {
    let client_secret = get_client_secrets();
    if let Ok(auth) = get_auth_token_from_file(&client_secret, filepath) {
        auth.access_token
    } else {
        let auth = get_auth_token_from_server(&client_secret);
        let _ = fs::write(
            filepath,
            format!(r#"{{ "refresh_token": "{}" }}"#, auth.refresh_token),
        );
        auth.access_token
    }
}

fn get_auth_token_from_server(client_secret: &ClientSecret) -> AuthInfo {
    let client = Client::new();
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

        let res = client
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
    filepath: &Path,
) -> Result<AuthInfo, Box<dyn Error>> {
    let file = fs::File::open(filepath)?;
    let json: Value = serde_json::from_reader(file)?;
    let refresh_token = if let Value::String(s) = &json["refresh_token"] {
        s
    } else {
        panic!("refresh_token is must be a string");
    };

    let client = Client::new();
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
