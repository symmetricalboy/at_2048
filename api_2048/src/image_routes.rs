use dropshot::{HttpError, Query, RequestContext, Body as DropshotBody};
use image::{Rgba, RgbaImage, ImageFormat};
use imageproc::drawing::{draw_filled_rect_mut, draw_text_mut};
use imageproc::rect::Rect;
use ab_glyph::{FontRef, PxScale, Font, ScaleFont as _};
use serde::Deserialize;
use schemars::JsonSchema;
use twothousand_forty_eight::{unified::game::GameState, v2::recording::SeededRecording};
use crate::ApiContext;
use urlencoding;

const TILE_SIZE: u32 = 100;
const PADDING: u32 = 10;
const FONT_PATH: &str = "assets/DejaVuSans.ttf";

fn get_tile_color(value: usize) -> Rgba<u8> {
    match value {
        2 => Rgba([238, 228, 218, 255]),
        4 => Rgba([237, 224, 200, 255]),
        8 => Rgba([242, 177, 121, 255]),
        16 => Rgba([245, 149, 99, 255]),
        32 => Rgba([246, 124, 95, 255]),
        64 => Rgba([246, 94, 59, 255]),
        128 => Rgba([237, 207, 114, 255]),
        256 => Rgba([237, 204, 97, 255]),
        512 => Rgba([237, 200, 80, 255]),
        1024 => Rgba([237, 197, 63, 255]),
        2048 => Rgba([237, 194, 46, 255]),
        _ => Rgba([205, 193, 180, 255]),
    }
}

fn get_text_color(value: usize) -> Rgba<u8> {
    if value <= 4 {
        Rgba([119, 110, 101, 255])
    } else {
        Rgba([249, 246, 242, 255])
    }
}

#[derive(Deserialize, JsonSchema)]
pub struct GenerateImageQuery {
    pub seeded_recording: String,
}

#[dropshot::endpoint {
    method = GET,
    path = "/share/game/image.png",
}]
pub async fn generate_board_image(
    _rqctx: RequestContext<ApiContext>,
    query: Query<GenerateImageQuery>,
) -> Result<http::Response<DropshotBody>, HttpError> {
    let original_seeded_recording_param = query.into_inner().seeded_recording;

    // Decode the potentially double-encoded seeded_recording string for parsing
    let string_to_parse = match urlencoding::decode(&original_seeded_recording_param) {
        Ok(cow_str) => cow_str.into_owned(),
        Err(e) => {
            log::warn!(
                "ImageGen: Failed to URL-decode seeded_recording string: {}. Error: {}",
                original_seeded_recording_param,
                e
            );
            return Err(HttpError::for_bad_request(
                None,
                format!("ImageGen: Invalid seeded_recording parameter: could not decode. Error: {}", e),
            ));
        }
    };

    let seeded_recording: SeededRecording = match string_to_parse.parse() { // Use decoded string
        Ok(rec) => rec,
        Err(e) => {
            log::error!("ImageGen: Failed to parse seeded_recording from '{}': {}", string_to_parse, e);
            return Err(HttpError::for_bad_request(None, format!("ImageGen: Invalid seeded_recording: {}", e)));
        }
    };

    let game_state: GameState = match GameState::from_reconstructable_ruleset(&seeded_recording) {
        Ok(gs) => gs,
        Err(e) => {
            log::error!("Failed to reconstruct game state: {}", e);
            return Err(HttpError::for_internal_error(format!("Could not reconstruct game state: {}", e)));
        }
    };

    let board_dim = game_state.board.width;
    if game_state.board.height != board_dim {
        log::warn!("Board width and height differ, using width for square image generation.");
    }

    let img_dimension = (board_dim as u32 * TILE_SIZE) + ((board_dim as u32 + 1) * PADDING);
    let mut img = RgbaImage::new(img_dimension, img_dimension);
    let board_bg_color = Rgba([187, 173, 160, 255]);
    draw_filled_rect_mut(&mut img, Rect::at(0, 0).of_size(img_dimension, img_dimension), board_bg_color);

    let font_bytes = match std::fs::read(FONT_PATH) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!("Failed to load font file '{}': {}", FONT_PATH, e);
            return Err(HttpError::for_internal_error("Internal server error: font file missing".to_string()));
        }
    };
    let font = match FontRef::try_from_slice(&font_bytes) {
        Ok(f) => f,
        Err(e) => {
            log::error!("Failed to parse font file '{}' with ab_glyph: {}", FONT_PATH, e);
            return Err(HttpError::for_internal_error("Internal server error: font parsing failed".to_string()));
        }
    };
    
    for r in 0..board_dim {
        for c in 0..board_dim {
            let tile_opt = game_state.board.tiles.get(r).and_then(|row| row.get(c)).and_then(|&t| t);
            let x_offset = PADDING + (c as u32 * (TILE_SIZE + PADDING));
            let y_offset = PADDING + (r as u32 * (TILE_SIZE + PADDING));
            let tile_value = tile_opt.map_or(0, |t| t.value);
            let rect_color = if tile_value == 0 {
                Rgba([205, 193, 180, 128])
            } else {
                get_tile_color(tile_value)
            };
            draw_filled_rect_mut(
                &mut img,
                Rect::at(x_offset as i32, y_offset as i32).of_size(TILE_SIZE, TILE_SIZE),
                rect_color,
            );

            if tile_value > 0 {
                let text = tile_value.to_string();
                let text_color = get_text_color(tile_value);
                let font_scale_val = match text.len() {
                    1 => 55.0, 2 => 55.0, 3 => 45.0, 4 => 35.0, _ => 30.0,
                };
                let scale = PxScale::from(font_scale_val);
                let (text_width, text_height) = {
                    let scaled_font = font.as_scaled(scale);
                    let mut total_width: f32 = 0.0;
                    let mut max_y: f32 = f32::NEG_INFINITY;
                    let mut min_y: f32 = f32::INFINITY;
                    if text.is_empty() {
                        (0.0, 0.0)
                    } else {
                        for glyph_char in text.chars() {
                            let glyph_id = font.glyph_id(glyph_char);
                            let individual_scaled_glyph = scaled_font.scaled_glyph(glyph_char);
                            if let Some(outline) = scaled_font.outline_glyph(individual_scaled_glyph) {
                                let bb = outline.px_bounds();
                                max_y = max_y.max(bb.max.y);
                                min_y = min_y.min(bb.min.y);
                            }
                            total_width += scaled_font.h_advance(glyph_id);
                        }
                        let height = if max_y == f32::NEG_INFINITY {
                            scaled_font.ascent() - scaled_font.descent()
                        } else {
                            max_y - min_y
                        };
                        (total_width, height.max(0.0))
                    }
                };
                let text_x = x_offset + (TILE_SIZE / 2) - (text_width / 2.0) as u32;
                let text_y = y_offset + (TILE_SIZE / 2) - (text_height / 2.0) as u32;
                draw_text_mut(
                    &mut img,
                    text_color,
                    text_x as i32, 
                    text_y as i32, 
                    scale,
                    &font,
                    &text,
                );
            }
        }
    }

    let mut buffer = Vec::new();
    if let Err(e) = img.write_to(&mut std::io::Cursor::new(&mut buffer), ImageFormat::Png) {
        log::error!("Failed to encode image to PNG: {}", e);
        return Err(HttpError::for_internal_error("Failed to generate image".to_string()));
    }

    let response = http::Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "image/png")
        .body(DropshotBody::from(buffer))
        .map_err(|e| {
            log::error!("Failed to create response: {}", e);
            HttpError::for_internal_error("Failed to create image response".to_string())
        })?;
    Ok(response)
} 