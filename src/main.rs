use twitch_youtube_sync::{download_twitch_video, UploadSession};

fn main() {
    let video_id = "548819121";
    download_twitch_video(video_id);

    let filepath = format!("videos/{}.mp4", video_id);
    let token_filepath = "token.json";
    let session = UploadSession::new(&video_id, &filepath, &token_filepath);
    session.upload();
}
