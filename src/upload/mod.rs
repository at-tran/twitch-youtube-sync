mod auth;

use crate::video::Video;
use auth::get_auth_token;
use reqwest::blocking::{Client, Response};
use reqwest::header::{CONTENT_LENGTH, CONTENT_RANGE, CONTENT_TYPE, LOCATION};
use serde_json::json;
use std::fs::File;
use std::io::{Seek, SeekFrom};
use std::path::Path;
use std::time::Duration;

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
        println!("Starting upload with URI: {}", self.upload_uri);
        self.start_upload();

        loop {
            let upload_status = self.check_upload_status();
            println!("{:?}", upload_status);

            match upload_status.status().as_u16() {
                308 => {
                    let mut continue_index: u64 = 0;
                    if let Some(range) = upload_status.headers().get("Range") {
                        continue_index = range.to_str().unwrap()[8..].parse::<u64>().unwrap() + 1;
                    }
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
        let file = File::open(&self.video.path).unwrap();

        let _ = self
            .client
            .put(&self.upload_uri)
            .bearer_auth(&self.auth_token)
            .header(CONTENT_LENGTH, self.video.size)
            .header(CONTENT_TYPE, "video/*")
            .timeout(Duration::from_secs(3600 * 15)) // 15 hours
            .body(file)
            .send();
    }

    fn resume_upload(&self, start_index: u64) {
        let mut file = File::open(&self.video.path).unwrap();
        file.seek(SeekFrom::Start(start_index)).unwrap();

        let res = self
            .client
            .put(&self.upload_uri)
            .bearer_auth(&self.auth_token)
            .header(CONTENT_LENGTH, self.video.size - start_index)
            .header(
                CONTENT_RANGE,
                &format!(
                    "bytes {}-{}/{}",
                    start_index,
                    self.video.size - 1,
                    self.video.size
                ),
            )
            .timeout(Duration::from_secs(3600 * 15)) // 15 hours
            .body(file)
            .send();
        println!("{:?}", res);
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
            .header("X-Upload-Content-Type", "video/*")
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
