use twitch_youtube_sync::{download_twitch_video, UploadSession};

fn main() {
    let video_id = "548819121";
    let video = download_twitch_video(video_id);

    let token_filepath = "token.json";
    let session = UploadSession::new(video, token_filepath);
    session.upload();
}
