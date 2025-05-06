use dropshot::{HttpError, Query, RequestContext, HttpResponseOk};
use schemars::JsonSchema;
use serde::Deserialize;
use twothousand_forty_eight::{unified::game::GameState, v2::recording::SeededRecording};
use crate::ApiContext;

// Assuming the image dimensions are known (e.g., for a 4x4 board)
// These should match the output of the image generation endpoint
const OG_IMAGE_WIDTH: u32 = 450; // (4 * 100px tiles) + (5 * 10px padding)
const OG_IMAGE_HEIGHT: u32 = 450;

#[derive(Deserialize, JsonSchema, Debug)]
pub struct ShareGameQuery {
    pub seeded_recording: String,
}

#[dropshot::endpoint {
    method = GET,
    path = "/share/game",
}]
pub async fn serve_shared_game_page(
    rqctx: RequestContext<ApiContext>,
    query: Query<ShareGameQuery>,
) -> Result<HttpResponseOk<String>, HttpError> {
    let api_context = rqctx.context();
    let query_params = query.into_inner();
    let original_seeded_recording_param = query_params.seeded_recording;

    // Decode the potentially double-encoded seeded_recording string for parsing
    let string_to_parse = match urlencoding::decode(&original_seeded_recording_param) {
        Ok(cow_str) => cow_str.into_owned(),
        Err(e) => {
            log::warn!(
                "Failed to URL-decode seeded_recording string: {}. Error: {}",
                original_seeded_recording_param,
                e
            );
            return Err(HttpError::for_bad_request(
                None,
                format!("Invalid seeded_recording parameter: could not decode. Error: {}", e),
            ));
        }
    };

    // Attempt to parse the recording to get game details
    let game_details = match string_to_parse.parse::<SeededRecording>() {
        Ok(rec) => match GameState::from_reconstructable_ruleset(&rec) {
            Ok(gs) => Some(gs),
            Err(e) => {
                log::warn!(
                    "Failed to create GameState from ruleset for decoded string: {}. Error: {:?}",
                    string_to_parse,
                    e
                );
                None
            }
        },
        Err(e) => {
            log::warn!(
                "Failed to parse SeededRecording from decoded string: {}. Error: {:?}",
                string_to_parse,
                e
            );
            None
        }
    };

    let score_str = game_details.map_or("a game".to_string(), |gs| gs.score_current.to_string());
    let title = format!("My 2048 Game Result - Score: {}", score_str);
    let description = format!(
        "I played a game of at://2048 and scored {}. Can you beat it?",
        score_str
    );

    let base_url = &api_context.config.base_url;

    // For the URLs in meta tags, use the original, once-encoded parameter string
    let page_url = format!(
        "{}/share/game?seeded_recording={}",
        base_url,
        original_seeded_recording_param // Use the original param value here
    );
    // Corrected image URL path and use original param value
    let image_url = format!(
        "{}/share/game/image.png?seeded_recording={}",
        base_url,
        original_seeded_recording_param // Use the original param value here
    );

    let html_content = format!(
        r#"<!DOCTYPE html>
        <html lang="en">
        <head>
            <meta charset="utf-8">
            <title>{}</title>
            <meta name="description" content="{}">
            
            <!-- Open Graph / Facebook -->
            <meta property="og:type" content="website">
            <meta property="og:url" content="{}">
            <meta property="og:title" content="{}">
            <meta property="og:description" content="{}">
            <meta property="og:image" content="{}">
            <meta property="og:image:width" content="{}">
            <meta property="og:image:height" content="{}">
            
            <!-- Twitter -->
            <meta property="twitter:card" content="summary_large_image">
            <meta property="twitter:url" content="{}">
            <meta property="twitter:title" content="{}">
            <meta property="twitter:description" content="{}">
            <meta property="twitter:image" content="{}">
            
            <!-- Optional: Redirect to the main game page if not a crawler -->
            <!-- <meta http-equiv="refresh" content="0;url=http://127.0.0.1:8080/"> --> <!-- Assuming Yew app runs on 8080 -->
        </head>
        <body>
            <h1>Sharing Game Result...</h1>
            <p>If you are not redirected, <a href="http://127.0.0.1:8080/">click here to play</a>.</p> <!-- Assuming Yew app runs on 8080 -->
            <p>Score: {}</p>
            <img src="{}" alt="Game Board Preview" width="{}" height="{}"/>
        </body>
        </html>"#,
        title, description, // head title, meta description
        page_url, title, description, image_url, OG_IMAGE_WIDTH, OG_IMAGE_HEIGHT, // OG tags
        page_url, title, description, image_url, // Twitter tags
        score_str, image_url, OG_IMAGE_WIDTH, OG_IMAGE_HEIGHT // body content
    );

    Ok(HttpResponseOk(html_content))
} 