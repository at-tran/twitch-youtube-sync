use twitch_youtube_sync::{download_twitch_video, UploadSession};

const VIDEOS_FOLDER: &str = "videos";
const TOKEN_FILEPATH: &str = "token.json";

fn main() {
    let video_id = "582619810";
    let video = download_twitch_video(video_id, VIDEOS_FOLDER);

    let session = UploadSession::new(video, TOKEN_FILEPATH);
    session.upload();
}
