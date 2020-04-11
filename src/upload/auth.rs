use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use serde_json::Value;
use std::error::Error;
use std::path::Path;
use std::time::Duration;
use std::{fs, thread};

const SECRETS_PATH: &str = "client_secrets.json";

#[derive(Deserialize, Debug)]
struct ClientSecret {
    client_id: String,
    client_secret: String,
}

fn get_client_secrets() -> ClientSecret {
    let filepath = SECRETS_PATH;
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

pub fn get_auth_token(filepath: &Path) -> String {
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
