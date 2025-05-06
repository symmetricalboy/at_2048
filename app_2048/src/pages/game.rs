use crate::agent::{StorageRequest, StorageResponse, StorageTask};
use crate::idb::{
    CURRENT_GAME_STORE, DB_NAME, SELF_KEY, STATS_STORE, object_delete, object_get, transaction_put,
};
use crate::store::UserStore;
use atrium_api::types::string::Datetime;
use gloo::dialogs::alert;
use gloo::events::EventListener;
use gloo::timers::callback::Timeout;
use indexed_db_futures::database::Database;
use js_sys::encode_uri_component;
use numfmt::{Formatter, Precision};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::rc::Rc;
use twothousand_forty_eight::direction::Direction;
use twothousand_forty_eight::{unified::game::GameState, v2::recording::SeededRecording};
use types_2048::blue;
use types_2048::blue::_2048::defs::SyncStatusData;
use web_sys::{HtmlElement, wasm_bindgen::JsCast, wasm_bindgen::closure::Closure};
use yew::platform::spawn_local;
use yew::{
    Callback, Html, Properties, Reducible, function_component, html, use_effect_with, use_mut_ref,
    use_node_ref, use_reducer, use_state, use_state_eq,
};
use yew_agent::oneshot::use_oneshot_runner;
use yew_hooks::use_effect_once;
use yewdux::use_store;
use urlencoding;
use gloo_console::log as gloo_log;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct State {
    gamestate: GameState,
    history: SeededRecording,
    message: String,
    hiscore: usize,
    // current_game: game::RecordData,
}

impl PartialEq for State {
    fn eq(&self, other: &Self) -> bool {
        self.history == other.history
            && self.message == other.message
            && self.hiscore == other.hiscore
    }
}

pub enum Action {
    Move(Direction),
}

impl Reducible for State {
    type Action = Action;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            Action::Move(direction) => {
                if !self.gamestate.allowed_moves.contains(&direction) {
                    let mut message = "".to_string();
                    if self.gamestate.over {
                        message = "Game over".to_string();
                    }
                    return Rc::new(State {
                        gamestate: self.gamestate.clone(),
                        history: self.history.clone(),
                        message,
                        hiscore: self.hiscore,
                    });
                }
                let mut new_history = self.history.clone();
                new_history.moves.push(direction);
                let history_string: String = (&new_history).into();

                let mut state = match history_string.parse::<SeededRecording>() {
                    Ok(history) => match GameState::from_reconstructable_ruleset(&history) {
                        Ok(gamestate) => State {
                            gamestate: gamestate.clone(),
                            history,
                            message: String::new(),
                            hiscore: self.hiscore.max(gamestate.score_max),
                        },
                        Err(e) => {
                            log::error!("{:?}", e);
                            State {
                                gamestate: self.gamestate.clone(),
                                history: self.history.clone(),
                                message: format!("{:?}", e),
                                hiscore: self.hiscore,
                            }
                        }
                    },
                    Err(e) => State {
                        gamestate: self.gamestate.clone(),
                        history: self.history.clone(),
                        message: format!("{:?}", e),
                        hiscore: self.hiscore,
                    },
                };
                let mut state_clone = state.clone();
                spawn_local(async move {
                    state_clone.save().await;
                });
                if state.gamestate.over {
                    state.message = "Game over".to_string();
                }

                //TODO spawn off to a worker to calculate new stats and save them
                Rc::new(state)
            }
        }
    }
}

impl State {
    pub fn new() -> Self {
        let random_seed = rand::random();
        let history = SeededRecording::empty(random_seed, 4, 4);
        let gamestate = GameState::from_reconstructable_ruleset(&history).unwrap();
        Self {
            gamestate,
            history,
            message: "".to_string(),
            hiscore: 0,
        }
    }

    pub async fn save(&mut self) {
        let history_string: String = (&self.history).into();

        let db = match Database::open(DB_NAME).await {
            Ok(db) => db,
            Err(err) => {
                panic!("Error opening database: {:?}", err);
            }
        };

        let mut current_game = match object_get::<blue::_2048::game::RecordData>(
            db.clone(),
            CURRENT_GAME_STORE,
            SELF_KEY,
        )
        .await
        {
            Ok(current_game) => match current_game {
                Some(current_game) => current_game,
                None => {
                    let current_game = blue::_2048::game::RecordData {
                        completed: self.gamestate.over,
                        created_at: Datetime::now(),
                        current_score: self.gamestate.score_current as i64,
                        seeded_recording: history_string,
                        sync_status: SyncStatusData {
                            created_at: Datetime::now(),
                            hash: "".to_string(),
                            synced_with_at_repo: false,
                            updated_at: Datetime::now(),
                        }
                        .into(),
                        won: self.gamestate.won,
                    };
                    let result = transaction_put(
                        db.clone(),
                        current_game.clone(),
                        CURRENT_GAME_STORE,
                        Some(SELF_KEY.to_string()),
                    )
                    .await;
                    match result {
                        Ok(_) => {}
                        Err(e) => {
                            log::info!("{:?}", current_game);
                            log::error!("{:?}", e.to_string());
                        }
                    };
                    return;
                }
            },
            Err(e) => {
                log::error!("{:?}", e.to_string());
                return;
            }
        };

        current_game.current_score = self.gamestate.score_current as i64;
        current_game.seeded_recording = history_string;

        let result = transaction_put(
            db.clone(),
            current_game,
            CURRENT_GAME_STORE,
            Some(SELF_KEY.to_string()),
        )
        .await;
        match result {
            Ok(_) => {}
            Err(e) => {
                log::error!("{:?}", e.to_string());
            }
        };
    }

    pub async fn load() -> Option<Self> {
        let db = match Database::open(DB_NAME).await {
            Ok(db) => db,
            Err(err) => {
                panic!("Error opening database: {:?}", err);
                // return Err(AtRepoSyncError::ThereWasAnError(err.to_string()));
            }
        };

        let current_game =
            match object_get::<blue::_2048::game::RecordData>(db, CURRENT_GAME_STORE, SELF_KEY)
                .await
            {
                Ok(current_game) => match current_game {
                    Some(current_game) => current_game,
                    None => {
                        return None;
                    }
                },
                Err(e) => {
                    log::error!("{:?}", e.to_string());
                    return None;
                }
            };

        let history_string = current_game.seeded_recording.clone();
        let history: SeededRecording = match history_string.parse() {
            Ok(history) => history,
            Err(e) => {
                log::error!("Error parsing history: {:?}", e.to_string());
                return None;
                // return Self::new(Some(&format!("Error parsing history: {:?}", e)));
            }
        };
        let gamestate = match GameState::from_reconstructable_ruleset(&history) {
            Ok(gamestate) => gamestate,
            Err(e) => {
                log::error!("Error reconstructing game: {:?}", e.to_string());
                return None;
            }
        };
        let hiscore = gamestate.score_max;
        Some(Self {
            history,
            message: "".to_string(),
            gamestate,
            hiscore,
        })
    }
}

fn get_position_class(row_start: usize, col_start: usize, size: usize) -> String {
    //Have to do this or tailwindcss does not pick up and send the css it seems
    let row_class = match row_start {
        0 => "top-0",
        //4x4
        1 if size == 4 => "top-1/4",
        2 if size == 4 => "top-2/4",
        3 if size == 4 => "top-3/4",
        //5x5
        1 if size == 5 => "top-1/5",
        2 if size == 5 => "top-2/5",
        3 if size == 5 => "top-3/5",
        4 if size == 5 => "top-4/5",
        //6x6
        1 if size == 6 => "top-1/6",
        2 if size == 6 => "top-2/6",
        3 if size == 6 => "top-3/6",
        4 if size == 6 => "top-4/6",
        5 if size == 6 => "top-5/6",
        _ => "", // Optionally handle unexpected cases
    };

    let col_class = match col_start {
        0 => "left-0",
        //4x4
        1 if size == 4 => "left-1/4",
        2 if size == 4 => "left-2/4",
        3 if size == 4 => "left-3/4",
        //5x5
        1 if size == 5 => "left-1/5",
        2 if size == 5 => "left-2/5",
        3 if size == 5 => "left-3/5",
        4 if size == 5 => "left-4/5",
        // 6x6
        1 if size == 6 => "left-1/6",
        2 if size == 6 => "left-2/6",
        3 if size == 6 => "left-3/6",
        4 if size == 6 => "left-4/6",
        5 if size == 6 => "left-5/6",
        _ => "", // Optionally handle unexpected cases
    };

    let temp = format!("{} {}", row_class, col_class);
    // log::info!("temp:{:?}", temp);
    temp.to_string()
}

fn get_bg_color_and_text_color<'a>(tile_size: usize) -> &'a str {
    match tile_size {
        0 => "bg-light-grid-cell-0",
        2 => "bg-light-grid-cell-2 text-light-grid-cell-text-2",
        4 => "bg-light-grid-cell-4 text-light-grid-cell-text-4",
        8 => "bg-light-grid-cell-8 text-light-grid-cell-text-8",
        16 => "bg-light-grid-cell-16 text-light-grid-cell-text-16",
        32 => "bg-light-grid-cell-32 text-light-grid-cell-text-32",
        64 => "bg-light-grid-cell-64 text-light-grid-cell-text-64",
        128 => "bg-light-grid-cell-128 text-light-grid-cell-text-128",
        256 => "bg-light-grid-cell-256 text-light-grid-cell-text-256",
        512 => "bg-light-grid-cell-512 text-light-grid-cell-text-512",
        1024 => "bg-light-grid-cell-1024 text-light-grid-cell-text-1024",
        //If it's over just keep to the same color as 2048
        2048 | _ => "bg-light-grid-cell-2048 text-light-grid-cell-text-2048",
    }
}

fn get_font_size(text: &str) -> String {
    let font_size = match text.len() {
        1 => "text-[4rem] md:text-[4rem]",
        2 => "text-[3.2rem] md:text-[3.7rem] lg:text-[4rem]",
        3 => "text-[2.5rem] lg:text-[4rem]",
        //If over 4 just keep to same size
        4 | _ => "text-[1.5rem] lg:text-[4rem]",
    };
    font_size.to_string()
}

#[derive(Properties, PartialEq, Clone)]
pub struct GridProps {
    pub position: usize,
    pub size: usize,
}

#[function_component(Grid)]
pub fn grid(props: &GridProps) -> Html {
    let GridProps { position, size } = props;
    let position_class = get_position_class(position / size, position % size, *size);
    html! {
        <div
            class={format!("absolute w-1/4 h-1/4 p-1 flex items-center justify-center {}",position_class)}
        >
            <div
                class="flex items-center justify-center w-full h-full bg-light-grid-cell-0 rounded-[5px]"
            />
        </div>
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct TileProps {
    pub tile_value: usize,
    pub new_tile: bool,
    pub x: usize,
    pub y: usize,
    pub size: usize,
}
#[function_component(Tile)]
pub fn tile(props: &TileProps) -> Html {
    let TileProps {
        tile_value: tile_value_ref,
        new_tile: new_tile_ref,
        x,
        y,
        size,
    } = props;

    let text = if *tile_value_ref == 0 {
        String::new()
    } else {
        // log::info!("value:{:?} loc: x{} y{}", *tile_value_ref, x, y);
        tile_value_ref.to_string()
    };
    let position_class = get_position_class(*y, *x, *size);

    let tile_class = get_bg_color_and_text_color(*tile_value_ref);
    let font_size = get_font_size(&text);

    let new_tile_animation = if *new_tile_ref && *tile_value_ref != 0 {
        "animate-spawn eink:animate-none duration-500s"
    } else {
        ""
    };
    let move_animation = "transition-all eink:transition-none duration-200 ease-out";

    html! {
        <div
            class={format!("absolute w-1/4 h-1/4 {} p-1 flex items-center justify-center {} {}", position_class, new_tile_animation, move_animation)}
        >
            <div
                class={format!(
                        "flex items-center justify-center w-full h-full {} font-bold {} rounded-md",
                        tile_class, font_size
                    )}
            >
                { text }
            </div>
        </div>
    }
}

pub enum ScoreBoardAction {
    NewGame,
}

#[derive(Properties, PartialEq, Clone)]
pub struct ScoreboardProps {
    pub current_score: usize,
    pub hiscore: usize,
    pub message: String,
    pub action: Callback<ScoreBoardAction>,
}

#[function_component(ScoreBoard)]
pub fn scoreboard(props: &ScoreboardProps) -> Html {
    let ScoreboardProps {
        current_score: score,
        hiscore,
        message,
        action,
    } = props.clone();
    let hiscore_to_display = if score > hiscore { score } else { hiscore };

    let onclick = {
        move |_| {
            action.emit(ScoreBoardAction::NewGame);
        }
    };
    let mut number_formatter = Formatter::new()
        .precision(Precision::Decimals(0))
        .separator(',')
        .expect("Could not build the number formatter.");

    html! {
        <>
            <div
                class="flex flex-row items-center justify-center rounded-md md:p-4 p-1 w-full mx-auto md:mt-4 mt-1"
            >
                //Stats
                <div class="stats shadow mx-5">
                    <div class="stat">
                        <div class="stat-title">{ "Score" }</div>
                        <div class="stat-value">{ number_formatter.fmt2(score) }</div>
                    </div>
                    <div class="stat">
                        <div class="stat-title">{ "Best" }</div>
                        <div class="stat-value">{ number_formatter.fmt2(hiscore_to_display) }</div>
                    </div>
                </div>
                <div class="flex flex-col items-center justify-center mx-5">
                    <button {onclick} class="btn btn-outline btn-sm">{ "New game" }</button>
                </div>
            </div>
            <div class="text-center md:mt-4 mt-2">
                <p>{ message }</p>
            </div>
        </>
    }
}

#[derive(Properties, PartialEq, Clone)]
pub struct GameBSkyButtonProps {
    pub seeded_recording_string: String,
    pub score: String, // Keep score for the text part of the bsky post
}

#[function_component(BSkyButton)]
fn bsky_button(props: &GameBSkyButtonProps) -> Html {
    // TODO: Make the base_url configurable, this should match the one in api_2048/src/share_routes.rs
    let app_base_url = "https://2048.symm.app"; // Updated for local testing API
    let encoded_seeded_recording = urlencoding::encode(&props.seeded_recording_string);
    
    // This is the URL that will have the OG tags and will be included in the post text
    let og_page_url = format!(
        "{}/share/game?seeded_recording={}",
        app_base_url,
        encoded_seeded_recording
    );

    // The text for the Bluesky post itself, now including the URL
    let display_text_for_bsky_post = format!(
        "I just scored {} on a game of at://2048!\nSee my final board here: {}",
        props.score,
        og_page_url // og_page_url is now defined
    );

    // The URL we send to bsky.app for it to scrape for OG tags
    let bsky_intent_url = format!(
        "https://bsky.app/intent/compose?text={}&url={}",
        encode_uri_component(&display_text_for_bsky_post),
        encode_uri_component(&og_page_url) // Bluesky will fetch this URL for OG tags
    );

    html!(
        <div class="flex justify-center my-2">
            <a class="btn btn-sm btn-accent" href={bsky_intent_url} target="_blank" rel="noopener noreferrer">
                { "Share on Bluesky" }
                <svg
                    class="inline-block w-8 ml-2 fill-[#0a7aff]" // Added ml-2 for a little spacing
                    viewBox="0 0 1024 1024"
                    fill="none"
                    xmlns="http://www.w3.org/2000/svg"
                >
                    <path
                        d="M351.121 315.106C416.241 363.994 486.281 463.123 512 516.315C537.719 463.123 607.759 363.994 672.879 315.106C719.866 279.83 796 252.536 796 339.388C796 356.734 786.055 485.101 780.222 505.943C759.947 578.396 686.067 596.876 620.347 585.691C735.222 605.242 764.444 670.002 701.333 734.762C581.473 857.754 529.061 703.903 515.631 664.481C513.169 657.254 512.017 653.873 512 656.748C511.983 653.873 510.831 657.254 508.369 664.481C494.939 703.903 442.527 857.754 322.667 734.762C259.556 670.002 288.778 605.242 403.653 585.691C337.933 596.876 264.053 578.396 243.778 505.943C237.945 485.101 228 356.734 228 339.388C228 252.536 304.134 279.83 351.121 315.106Z"
                    />
                </svg>
            </a>
        </div>
    )
}

#[derive(Properties, PartialEq, Clone)]
pub struct GameProps {
    pub state: State,
    pub action: Callback<ScoreBoardAction>,
}

#[function_component(Board)]
pub fn board(game_props: &GameProps) -> Html {
    let state = use_reducer(|| game_props.state.clone());
    let (user_store, _) = use_store::<UserStore>();
    let move_delay: Rc<RefCell<Option<Timeout>>> = use_mut_ref(|| None);
    let storage_task = use_oneshot_runner::<StorageTask>();
    let storage_agent = storage_task.clone();
    let hiscore_handle = use_state_eq(|| state.hiscore);

    use_effect_with(hiscore_handle.clone(), move |h_handle| {
        let current_hiscore_handle = h_handle.clone();
        spawn_local(async move {
            let db = match Database::open(DB_NAME).await {
                Ok(db) => db,
                Err(err) => {
                    panic!("Error opening database: {:?}", err);
                }
            };
            match object_get::<blue::_2048::player::stats::RecordData>(db, STATS_STORE, SELF_KEY)
                .await
            {
                Ok(stats_opt) => match stats_opt {
                    Some(stats_data) => {
                        current_hiscore_handle.set(stats_data.highest_score as usize);
                    }
                    None => {}
                },
                Err(_) => {}
            }
        });
        || ()
    });

    let game_over_state_reducer = state.clone();
    let user_store_for_effect = user_store.clone();
    let storage_agent_for_effect = storage_agent.clone();

    use_effect_with(state.gamestate.over, move |gameover| {
        if *gameover {
            let history_string: String = (&(*game_over_state_reducer).history.clone()).into();
            let did = user_store_for_effect.did.clone();
            
            spawn_local(async move {
                let request = StorageRequest::GameCompleted(history_string, did);
                let result = storage_agent_for_effect.run(request).await;
                match result {
                    StorageResponse::Error(err) => {
                        let message_sorry = "Sorry there was an error saving your game. This is still in alpha and has some bugs so please excuse us. If you are logged in with your AT Proto account may try relogging and refreshing this page without hitting new game. It will try to sync again. Sorry again and thanks for trying out at://2048!";
                        alert(message_sorry);
                        log::error!("Error saving game: {:?}", err.to_string());
                    }
                    _ => {
                        log::info!("Game completion processed by storage agent.");
                    }
                }
            });
        }
        ||()
    });

    use_effect_with(state.clone(), {
        let move_delay = move_delay.clone();
        move |state| {
            let state = state.clone();
            let listener = EventListener::new(&gloo::utils::document(), "keydown", move |event| {
                if let Some(event) = event.dyn_ref::<web_sys::KeyboardEvent>() {
                    let direction = match event.key().as_str() {
                        "k" | "w" | "ArrowUp" => Direction::UP,
                        "j" | "s" | "ArrowDown" => Direction::DOWN,
                        "h" | "a" | "ArrowLeft" => Direction::LEFT,
                        "l" | "d" | "ArrowRight" => Direction::RIGHT,
                        _ => return,
                    };
                    let cloned_state = state.clone();
                    *move_delay.borrow_mut() = Some(Timeout::new(150, {
                        let move_delay_timer = move_delay.clone();
                        move || {
                            move_delay_timer.borrow_mut().take();
                            cloned_state.dispatch(Action::Move(direction));
                        }
                    }));
                }
            });

            move || drop(listener)
        }
    });

    let board_ref = use_node_ref();
    let touch_start = use_mut_ref(|| (0, 0));

    {
        let touch_start = touch_start.clone();
        use_effect_with(board_ref.clone(), move |board_ref| {
            let board = board_ref
                .cast::<HtmlElement>()
                .expect("board_ref not attached to div element");

            let callback = Closure::wrap(Box::new(move |event: web_sys::Event| {
                event.prevent_default();
                if let Some(event) = event.dyn_ref::<web_sys::TouchEvent>() {
                    if let Some(touch) = event.changed_touches().item(0) {
                        let x = touch.client_x();
                        let y = touch.client_y();
                        *touch_start.borrow_mut() = (x, y);
                    }
                }
            }) as Box<dyn FnMut(_)>);

            board
                .add_event_listener_with_callback("touchstart", callback.as_ref().unchecked_ref())
                .unwrap();
            callback.forget();

            || ()
        });
    }

    {
        let touch_start = touch_start.clone();
        let move_delay = move_delay.clone();
        let state = state.clone();

        use_effect_with(board_ref.clone(), move |board_ref| {
            let board = board_ref
                .cast::<HtmlElement>()
                .expect("board_ref not attached to div element");

            let callback = Closure::wrap(Box::new(move |event: web_sys::Event| {
                event.prevent_default();

                if let Some(event) = event.dyn_ref::<web_sys::TouchEvent>() {
                    if let Some(touch) = event.changed_touches().item(0) {
                        let touch_end_x = touch.client_x();
                        let touch_end_y = touch.client_y();

                        let (start_x, start_y) = *touch_start.borrow();

                        let delta_x = touch_end_x - start_x;
                        let delta_y = touch_end_y - start_y;
                        if delta_x.abs() < 10 && delta_y.abs() < 10 {
                            return;
                        }

                        let direction = if delta_x.abs() > delta_y.abs() {
                            if delta_x > 0 {
                                Direction::RIGHT
                            } else {
                                Direction::LEFT
                            }
                        } else {
                            if delta_y > 0 {
                                Direction::DOWN
                            } else {
                                Direction::UP
                            }
                        };

                        *move_delay.borrow_mut() = Some(Timeout::new(150, {
                            let cloned_state = state.clone();
                            let move_delay = move_delay.clone();
                            move || {
                                move_delay.borrow_mut().take();
                                cloned_state.dispatch(Action::Move(direction));
                            }
                        }));
                    }
                }
            }) as Box<dyn FnMut(_)>);

            board
                .add_event_listener_with_callback("touchend", callback.as_ref().unchecked_ref())
                .unwrap();
            callback.forget();

            || ()
        });
    }

    let state = state.clone();
    let width = state.gamestate.board.width;
    let height = state.gamestate.board.height;
    let total_tiles = width * height;
    let flatten_tiles = state
        .gamestate
        .board
        .tiles
        .iter()
        .flatten()
        .filter_map(|tile| *tile)
        .collect::<Vec<_>>();

    let action = game_props.action.clone();
    let score_board_callback =
        Callback::from(move |board_action: ScoreBoardAction| match board_action {
            ScoreBoardAction::NewGame => {
                action.emit(ScoreBoardAction::NewGame);
            }
        });
    html! {
        <div class="flex flex-col ">
            <ScoreBoard
                current_score={state.gamestate.score_max}
                hiscore={*hiscore_handle as usize}
                message={state.message.clone()}
                action={score_board_callback.clone()}
            />
            {
                if state.gamestate.over {
                    let history_string: String = (&state.history).into();
                    gloo_log!(format!("Game Over! Seeded Recording: {}", history_string));
                    html! { <BSkyButton score={state.hiscore.to_string()} seeded_recording_string={history_string.clone()} /> }
                } else {
                    html! {}
                }
            }
            <div
                ref={board_ref}
                id="game-board"
                class="flex-1 mx-auto md:p-4 p-4 w-90 md:w-1/2 lg:w-1/2 xl:w-120 bg-light-board-background shadow-2xl rounded-md md:mt-4 xs:mt-1 mt-2"
            >
                <div class="aspect-square p-2 flex flex-col  rounded-md w-full  relative ">
                    <div className="flex flex-col p-2 relative w-full h-full">
                        { (0..total_tiles).map(|i| {
                                html! { <Grid key={format!("grid-parent-{}", i)} position={i} size={width} /> }
                            }).collect::<Html>() }
                        { flatten_tiles.into_iter().map(|tile| {
                                html! { <Tile key={tile.id} tile_value={tile.value} new_tile={tile.new} x={tile.x} y={tile.y} size={width} /> }
                            }).collect::<Html>() }
                    </div>
                </div>
            </div>
        </div>
    }
}

#[function_component(GamePage)]
pub fn game() -> Html {
    let current_game_state = use_state(|| None);
    let current_game_state_clone = current_game_state.clone();
    let cloned_state_for_callback = current_game_state_clone.clone();

    let score_board_callback = {
        let cloned_state = cloned_state_for_callback.clone();
        Callback::from(move |action: ScoreBoardAction| match action {
            ScoreBoardAction::NewGame => {
                let cloned_state = cloned_state.clone();
                cloned_state.set(None);
                spawn_local(async move {
                    let db = match Database::open(DB_NAME).await {
                        Ok(db) => db,
                        Err(err) => {
                            panic!("Error opening database: {:?}", err);
                        }
                    };
                    let _ = object_delete(db, CURRENT_GAME_STORE, SELF_KEY).await;
                    cloned_state.set(Some(State::new()));
                })
            }
        })
    };

    use_effect_once(move || {
        spawn_local(async move {
            match State::load().await {
                None => {
                    current_game_state_clone.set(Some(State::new()));
                }
                Some(current_game) => {
                    current_game_state_clone.set(Some(current_game));
                }
            }
        });
        || ()
    });

    if let Some(game) = (*current_game_state).clone() {
        html! { <Board state={game} action={score_board_callback} /> }
    } else {
        html! {
            <div class="flex flex-col items-center justify-center h-screen bg-base-200">
                <div class="flex items-center justify-center">
                    <span class="loading loading-spinner loading-lg" />
                    <h1 class="ml-4 text-3xl font-bold">{ "Loading..." }</h1>
                </div>
            </div>
        }
    }
}
