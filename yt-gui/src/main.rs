extern crate scarlet_ui_macros;

use std::f32;
#[cfg(not(test))]
use std::process::Command;
use std::sync::Mutex;
use std::thread;

use scarlet_ui::graphics;
mod ui;
use scarlet_ui::views::ImageFit;
use scarlet_ui::{BitmapImage, KeyCode, KeyEvent, State, hstack, prelude::*, vstack, zstack};
use scarlet_ui_macros::View;
use scarlet_youtube_net::{
    YoutubeSearchCursor, YoutubeVideoDetails, fetch_youtube_thumbnail_bytes, youtube_video_details,
};
use ui::gallery_columns;

const PAGE_SIZE: usize = 12;
const THUMB_WIDTH: u32 = 160;
const THUMB_HEIGHT: u32 = 90;
const DETAIL_TEXT_WIDTH: u32 = 272;
const DETAIL_DESCRIPTION_FONT_SIZE: f32 = 11.0;
const DETAIL_DESCRIPTION_MAX_CHARS: usize = 900;
const DETAIL_DESCRIPTION_MAX_LINES: usize = 24;

#[derive(Clone)]
enum ThumbnailState {
    NotRequested,
    Loading,
    Ready(BitmapImage),
    Failed,
}

impl ThumbnailState {
    fn is_not_requested(&self) -> bool {
        matches!(self, Self::NotRequested)
    }
}

#[derive(Clone)]
enum DetailState {
    Empty,
    Loading { video_id: String },
    Ready(GuiVideoDetails),
    Failed { video_id: String, message: String },
}

impl Default for DetailState {
    fn default() -> Self {
        Self::Empty
    }
}

#[derive(Clone)]
struct GuiVideoDetails {
    video_id: String,
    title: Option<String>,
    author: Option<String>,
    description: Option<String>,
    thumbnail_url: Option<String>,
}

impl GuiVideoDetails {
    fn from_youtube(details: YoutubeVideoDetails) -> Self {
        Self {
            video_id: details.video_id,
            title: details.title,
            author: details.author,
            description: details.description,
            thumbnail_url: details.thumbnail_url,
        }
    }
}

#[derive(Clone)]
struct GuiSearchResult {
    video_id: String,
    title: String,
    channel: Option<String>,
    duration: Option<String>,
    thumbnail: ThumbnailState,
    thumbnail_url: Option<String>,
}

impl GuiSearchResult {
    fn watch_url(&self) -> String {
        format!("https://www.youtube.com/watch?v={}", self.video_id)
    }
}

enum GuiMessage {
    SearchFinished {
        generation: u64,
        result: SearchLoadOutcome,
    },
    SearchMoreFinished {
        generation: u64,
        target_page: Option<usize>,
        result: SearchLoadOutcome,
    },
    ThumbnailFinished {
        generation: u64,
        video_id: String,
        thumbnail: ThumbnailState,
    },
    ThumbnailBatchFinished {
        generation: u64,
    },
    DetailsFinished {
        generation: u64,
        video_id: String,
        result: std::result::Result<GuiVideoDetails, String>,
    },
    PlaybackFinished {
        title: String,
        result: std::result::Result<(), String>,
    },
}

struct SearchLoadResult {
    results: Vec<GuiSearchResult>,
    has_more: bool,
    cursor: YoutubeSearchCursor,
}

struct SearchLoadError {
    message: String,
    cursor: Option<YoutubeSearchCursor>,
}

type SearchLoadOutcome = std::result::Result<SearchLoadResult, SearchLoadError>;

static YT_GUI_GENERATION: Mutex<u64> = Mutex::new(0);
static YT_GUI_MESSAGES: Mutex<Vec<GuiMessage>> = Mutex::new(Vec::new());
static YT_GUI_SEARCH_CURSOR: Mutex<Option<YoutubeSearchCursor>> = Mutex::new(None);
static YT_GUI_THUMBNAIL_ACTIVE: Mutex<bool> = Mutex::new(false);
static YT_GUI_DETAILS_ACTIVE: Mutex<bool> = Mutex::new(false);
static YT_GUI_PLAYBACK_ACTIVE: Mutex<bool> = Mutex::new(false);

#[derive(View, Clone)]
struct YtGuiApp {
    query: State<String>,
    results: State<Vec<GuiSearchResult>>,
    selected: State<usize>,
    page: State<usize>,
    status: State<String>,
    list_focused: State<bool>,
    has_more: State<bool>,
    loading_more: State<bool>,
    details: State<DetailState>,
    screen: State<u8>,
    control: State<usize>,
    bounds: State<Size>,
    confirm_held: State<bool>,
    fit_window: State<bool>,
    page_bounds: State<Size>,
    header_bounds: State<Size>,
}

impl YtGuiApp {
    fn new(query: String) -> Self {
        Self {
            query: State::new(scarlet_ui::state::generate_state_id(), query),
            results: State::new(scarlet_ui::state::generate_state_id(), Vec::new()),
            selected: State::new(scarlet_ui::state::generate_state_id(), 0),
            page: State::new(scarlet_ui::state::generate_state_id(), 0),
            status: State::new(
                scarlet_ui::state::generate_state_id(),
                String::from("Type a query and press Enter."),
            ),
            list_focused: State::new(scarlet_ui::state::generate_state_id(), false),
            has_more: State::new(scarlet_ui::state::generate_state_id(), false),
            loading_more: State::new(scarlet_ui::state::generate_state_id(), false),
            details: State::new(scarlet_ui::state::generate_state_id(), DetailState::Empty),
            screen: State::new(scarlet_ui::state::generate_state_id(), 0),
            control: State::new(scarlet_ui::state::generate_state_id(), 0),
            bounds: State::new(
                scarlet_ui::state::generate_state_id(),
                Size::new(800.0, 600.0),
            ),
            confirm_held: State::new(scarlet_ui::state::generate_state_id(), false),
            fit_window: State::new(scarlet_ui::state::generate_state_id(), true),
            page_bounds: State::new(
                scarlet_ui::state::generate_state_id(),
                Size::new(684.0, 528.0),
            ),
            header_bounds: State::new(
                scarlet_ui::state::generate_state_id(),
                Size::new(684.0, 40.0),
            ),
        }
    }

    fn selected_result(&self) -> Option<GuiSearchResult> {
        self.results.get().get(self.selected.get()).cloned()
    }

    fn submit_search_from_key(&self) {
        if self.confirm_held.get() {
            return;
        }
        self.confirm_held.set(true);
        self.submit_search();
    }

    fn submit_search(&self) {
        perform_search(
            self.query.clone(),
            self.results.clone(),
            self.selected.clone(),
            self.page.clone(),
            self.status.clone(),
            self.has_more.clone(),
            self.loading_more.clone(),
            self.details.clone(),
        );
        self.screen.set(0);
        self.control.set(1);
        self.list_focused.set(true);
    }

    fn play_video(&self, index: usize) {
        if index >= self.results.get().len() {
            return;
        }
        self.selected.set(index);
        play_selected(
            self.results.clone(),
            self.selected.clone(),
            self.status.clone(),
        );
    }

    fn open_details(&self, index: usize) {
        if index >= self.results.get().len() {
            return;
        }
        self.selected.set(index);
        self.control.set(1);
        self.screen.set(1);
        request_selected_details(
            self.results.clone(),
            self.selected.clone(),
            self.details.clone(),
            current_generation(),
        );
    }

    fn begin_search(&self) {
        self.screen.set(2);
        self.list_focused.set(false);
    }

    fn cancel_search(&self) {
        self.screen.set(0);
        self.control.set(0);
        self.list_focused.set(true);
        self.confirm_held.set(false);
    }

    fn handle_key(&self, event: KeyEvent) -> bool {
        if let KeyEvent::Released {
            keycode: KeyCode::Enter,
            ..
        } = event
        {
            self.confirm_held.set(false);
            return true;
        }
        let KeyEvent::Pressed { keycode, .. } = event else {
            return false;
        };
        if keycode == KeyCode::Enter {
            if self.confirm_held.get() {
                return true;
            }
            self.confirm_held.set(true);
        }
        if keycode == KeyCode::Escape {
            let was_browse = self.screen.get() == 0;
            self.screen.set(0);
            self.control.set(if was_browse { 0 } else { 1 });
            return true;
        }
        if self.screen.get() == 2 {
            // The focused platform TextField and existing SoftKeyboard own text input.
            return false;
        }
        if self.screen.get() == 1 {
            match keycode {
                KeyCode::Up | KeyCode::Down | KeyCode::Tab => {
                    self.control.set(1 - self.control.get().min(1))
                }
                KeyCode::Enter => {
                    if self.control.get() == 0 {
                        self.screen.set(0);
                        self.control.set(1);
                    } else {
                        play_selected(
                            self.results.clone(),
                            self.selected.clone(),
                            self.status.clone(),
                        );
                    }
                }
                _ => return false,
            }
            return true;
        }
        let layout = ui::PageLayout::new(self.page_bounds.get());
        let columns = gallery_columns(layout.width, layout.gallery_height);
        match keycode {
            KeyCode::Enter => match self.control.get() {
                0 => {
                    self.begin_search();
                }
                1 => self.open_details(self.selected.get()),
                2 => previous_page(
                    self.results.clone(),
                    self.page.clone(),
                    self.selected.clone(),
                    self.status.clone(),
                    self.details.clone(),
                ),
                _ => next_page(
                    self.results.clone(),
                    self.page.clone(),
                    self.selected.clone(),
                    self.status.clone(),
                    self.has_more.clone(),
                    self.loading_more.clone(),
                    self.details.clone(),
                ),
            },
            KeyCode::Tab => {
                let controls = [0, 1, 2, 3];
                let position = controls
                    .iter()
                    .position(|c| *c == self.control.get())
                    .unwrap_or(0);
                self.control.set(controls[(position + 1) % controls.len()]);
            }
            KeyCode::Up | KeyCode::Down => {
                let down = keycode == KeyCode::Down;
                if self.control.get() == 0 && down {
                    self.control.set(1);
                } else if self.control.get() == 1 {
                    if !down && self.selected.get() % PAGE_SIZE < columns {
                        self.control.set(0);
                    } else {
                        move_selection(
                            self.results.clone(),
                            self.selected.clone(),
                            self.page.clone(),
                            self.status.clone(),
                            self.has_more.clone(),
                            self.loading_more.clone(),
                            self.details.clone(),
                            if down {
                                columns as isize
                            } else {
                                -(columns as isize)
                            },
                        );
                    }
                } else {
                    self.control.set(if down { 1 } else { 0 });
                }
            }
            KeyCode::Left | KeyCode::Right => {
                let right = keycode == KeyCode::Right;
                if !self.control.get() == 1 {
                    let controls = [0, 2, 3];
                    let position = controls
                        .iter()
                        .position(|c| *c == self.control.get())
                        .unwrap_or(0);
                    self.control.set(
                        controls[if right {
                            (position + 1) % 3
                        } else {
                            (position + 2) % 3
                        }],
                    );
                } else {
                    move_selection(
                        self.results.clone(),
                        self.selected.clone(),
                        self.page.clone(),
                        self.status.clone(),
                        self.has_more.clone(),
                        self.loading_more.clone(),
                        self.details.clone(),
                        if right { 1 } else { -1 },
                    );
                }
            }
            KeyCode::PageUp => previous_page(
                self.results.clone(),
                self.page.clone(),
                self.selected.clone(),
                self.status.clone(),
                self.details.clone(),
            ),
            KeyCode::PageDown => next_page(
                self.results.clone(),
                self.page.clone(),
                self.selected.clone(),
                self.status.clone(),
                self.has_more.clone(),
                self.loading_more.clone(),
                self.details.clone(),
            ),
            _ => return false,
        }
        true
    }
}

fn content_size(window_size: Size) -> Size {
    let decoration = scarlet_ui::views::WindowContentLayout::new(true).decoration_size();
    Size::new(
        (window_size.width - decoration.width).max(1.0),
        (window_size.height - decoration.height).max(1.0),
    )
}

impl Application for YtGuiApp {
    fn scenes(&self) -> impl Scene {
        self.window_view()
    }
    fn on_window_created(
        &mut self,
        _: &scarlet_ui::WindowContext,
        window: &mut dyn scarlet_ui::PlatformWindow,
    ) {
        let _ = window.set_gamepad_input(false, true);
        self.list_focused.set(true);
    }
    fn on_window_resize(&mut self, _: &scarlet_ui::WindowContext, width: u32, height: u32) {
        self.bounds
            .set(content_size(Size::new(width as f32, height as f32)));
    }
    fn on_window_sync(
        &mut self,
        _: &scarlet_ui::WindowContext,
        window: &mut dyn scarlet_ui::PlatformWindow,
    ) {
        if self.fit_window.get() {
            self.fit_window.set(false);
            if scarlet_ui::current_input_environment().windowing_mode()
                != Some(scarlet_ui::WindowingMode::Focused)
                && let Ok((width, height)) = window.get_screen_size()
            {
                let desired = window.size();
                let _ = window.resize(
                    (desired.width as u32).min(width.saturating_sub(16)).max(1),
                    (desired.height as u32)
                        .min(height.saturating_sub(48))
                        .max(1),
                );
            }
        }
        // Read the per-window logical size, including compositor-driven changes.
        let size = content_size(window.managed_size());
        if self.bounds.get() != size {
            self.bounds.set(size);
        }
    }
    fn on_active_app_changed(&mut self, _: u32, app_name: &str, _: &str) {
        if app_name != "org.scarlet-os.yt-gui" {
            self.confirm_held.set(false);
        }
    }
    fn debug_logging(&self) -> bool {
        false
    }
    fn on_idle(&mut self) {
        while let Some(message) = pop_gui_message() {
            self.handle_message(message);
        }
        request_selected_details(
            self.results.clone(),
            self.selected.clone(),
            self.details.clone(),
            current_generation(),
        );
    }
}

impl YtGuiApp {
    fn handle_message(&self, message: GuiMessage) {
        match message {
            GuiMessage::SearchFinished { generation, result } => {
                if generation != current_generation() {
                    return;
                }
                match result {
                    Ok(load) => {
                        let count = load.results.len();
                        *YT_GUI_SEARCH_CURSOR.lock().expect("yt gui mutex poisoned") =
                            Some(load.cursor);
                        self.results.set(load.results);
                        self.selected.set(0);
                        self.page.set(0);
                        self.has_more.set(load.has_more);
                        self.loading_more.set(false);
                        self.status.set(format!(
                            "Loaded {} search results{}.",
                            count,
                            if load.has_more { " so far" } else { "" }
                        ));
                        request_visible_thumbnails(
                            self.results.clone(),
                            0,
                            self.status.clone(),
                            generation,
                        );
                        request_selected_details(
                            self.results.clone(),
                            self.selected.clone(),
                            self.details.clone(),
                            generation,
                        );
                    }
                    Err(error) => {
                        *YT_GUI_SEARCH_CURSOR.lock().expect("yt gui mutex poisoned") = None;
                        self.results.set(Vec::new());
                        self.selected.set(0);
                        self.page.set(0);
                        self.has_more.set(false);
                        self.loading_more.set(false);
                        self.details.set(DetailState::Empty);
                        self.status.set(format!("Search failed: {}", error.message));
                    }
                }
            }
            GuiMessage::SearchMoreFinished {
                generation,
                target_page,
                result,
            } => {
                if generation != current_generation() {
                    return;
                }
                self.loading_more.set(false);
                match result {
                    Ok(load) => {
                        let mut list = self.results.get();
                        let added = load.results.len();
                        list.extend(load.results);
                        let len = list.len();
                        *YT_GUI_SEARCH_CURSOR.lock().expect("yt gui mutex poisoned") =
                            Some(load.cursor);
                        self.results.set(list);
                        self.has_more.set(load.has_more);

                        let page_to_show = if let Some(target_page) = target_page {
                            target_page.min(page_count(len).saturating_sub(1))
                        } else {
                            self.page.get()
                        };
                        self.page.set(page_to_show);
                        if len > 0 {
                            self.selected
                                .set((page_to_show.saturating_mul(PAGE_SIZE)).min(len - 1));
                        }
                        self.status.set(format!(
                            "Loaded {} more results ({} total{}).",
                            added,
                            len,
                            if load.has_more { "+" } else { "" }
                        ));
                        request_visible_thumbnails(
                            self.results.clone(),
                            page_to_show,
                            self.status.clone(),
                            generation,
                        );
                        request_selected_details(
                            self.results.clone(),
                            self.selected.clone(),
                            self.details.clone(),
                            generation,
                        );
                    }
                    Err(error) => {
                        if let Some(cursor) = error.cursor {
                            *YT_GUI_SEARCH_CURSOR.lock().expect("yt gui mutex poisoned") =
                                Some(cursor);
                        }
                        self.status
                            .set(format!("Search continuation failed: {}", error.message));
                    }
                }
            }
            GuiMessage::ThumbnailFinished {
                generation,
                video_id,
                thumbnail,
            } => {
                if generation != current_generation() {
                    return;
                }
                let mut list = self.results.get();
                if let Some(result) = list.iter_mut().find(|result| result.video_id == video_id) {
                    result.thumbnail = thumbnail;
                    self.results.set(list);
                }
            }
            GuiMessage::ThumbnailBatchFinished { generation } => {
                *YT_GUI_THUMBNAIL_ACTIVE
                    .lock()
                    .expect("yt gui mutex poisoned") = false;
                if generation != current_generation() {
                    return;
                }
                request_visible_thumbnails(
                    self.results.clone(),
                    self.page.get(),
                    self.status.clone(),
                    generation,
                );
            }
            GuiMessage::DetailsFinished {
                generation,
                video_id,
                result,
            } => {
                *YT_GUI_DETAILS_ACTIVE.lock().expect("yt gui mutex poisoned") = false;
                if generation != current_generation() {
                    return;
                }
                let current = self.details.get();
                if !matches!(current, DetailState::Loading { video_id: ref current_id } if current_id == &video_id)
                {
                    return;
                }
                match result {
                    Ok(details) => self.details.set(DetailState::Ready(details)),
                    Err(message) => self.details.set(DetailState::Failed { video_id, message }),
                }
            }
            GuiMessage::PlaybackFinished { title, result } => {
                *YT_GUI_PLAYBACK_ACTIVE
                    .lock()
                    .expect("yt gui mutex poisoned") = false;
                self.screen.set(0);
                self.control.set(1);
                self.list_focused.set(true);
                match result {
                    Ok(()) => self.status.set(format!("Playback finished: {}", title)),
                    Err(error) => self
                        .status
                        .set(format!("Playback failed: {} ({})", title, error)),
                }
            }
        }
    }
}

fn perform_search(
    query: State<String>,
    results: State<Vec<GuiSearchResult>>,
    selected: State<usize>,
    page: State<usize>,
    status: State<String>,
    has_more: State<bool>,
    loading_more: State<bool>,
    details: State<DetailState>,
) {
    let query_text = query.get().trim().to_string();
    if query_text.is_empty() {
        status.set(String::from("Enter a search query."));
        return;
    }

    let generation_id = next_generation();
    #[cfg(test)]
    let _ = generation_id;
    results.set(Vec::new());
    selected.set(0);
    page.set(0);
    has_more.set(false);
    loading_more.set(true);
    details.set(DetailState::Empty);
    *YT_GUI_SEARCH_CURSOR.lock().expect("yt gui mutex poisoned") = None;
    status.set(format!("Searching: {}", query_text));
    println!("[yt-gui] searching: {}", query_text);
    #[cfg(not(test))]
    thread::spawn(move || {
        let result = load_search_page(YoutubeSearchCursor::new(&query_text), PAGE_SIZE);
        push_gui_message(GuiMessage::SearchFinished {
            generation: generation_id,
            result,
        });
    });
}

fn play_selected(
    results: State<Vec<GuiSearchResult>>,
    selected: State<usize>,
    status: State<String>,
) {
    let list = results.get();
    let Some(row) = list.get(selected.get()) else {
        status.set(String::from("No selected result."));
        return;
    };
    let url = row.watch_url();
    let title = row.title.clone();
    {
        let mut active = YT_GUI_PLAYBACK_ACTIVE
            .lock()
            .expect("yt gui mutex poisoned");
        if *active {
            status.set(String::from(
                "Playback is already active; wait for it to finish or close the player.",
            ));
            return;
        }
        *active = true;
    }
    status.set(format!("Starting playback: {}", title));
    println!("[yt-gui] spawn: yt --title <title> {}", url);
    #[cfg(not(test))]
    thread::spawn(move || {
        let result = Command::new("/bin/yt")
            .args(["--title", &title, &url])
            .status()
            .map_err(|error| format!("failed to start /bin/yt: {}", error))
            .and_then(|status| {
                if status.success() {
                    Ok(())
                } else {
                    Err(format!(
                        "/bin/yt exited with status {}",
                        status.code().unwrap_or(1)
                    ))
                }
            });
        push_gui_message(GuiMessage::PlaybackFinished { title, result });
    });
}

fn move_selection(
    results: State<Vec<GuiSearchResult>>,
    selected: State<usize>,
    page: State<usize>,
    status: State<String>,
    has_more: State<bool>,
    loading_more: State<bool>,
    details: State<DetailState>,
    delta: isize,
) {
    let len = results.get().len();
    if len == 0 {
        selected.set(0);
        page.set(0);
        return;
    }
    let current = selected.get().min(len - 1);
    let next = if delta < 0 {
        current.saturating_sub(delta.unsigned_abs())
    } else {
        current.saturating_add(delta as usize).min(len - 1)
    };
    if delta > 0 && next == current && has_more.get() {
        load_more_search_results(
            status,
            has_more,
            loading_more,
            Some(page.get().saturating_add(1)),
        );
        return;
    }
    let next_page = next / PAGE_SIZE;
    selected.set(next);
    page.set(next_page);
    let generation = current_generation();
    request_visible_thumbnails(results.clone(), next_page, status, generation);
    request_selected_details(results, selected, details, generation);
}

fn next_page(
    results: State<Vec<GuiSearchResult>>,
    page: State<usize>,
    selected: State<usize>,
    status: State<String>,
    has_more: State<bool>,
    loading_more: State<bool>,
    details: State<DetailState>,
) {
    let len = results.get().len();
    let next = page.get().saturating_add(1);
    if next.saturating_mul(PAGE_SIZE) >= len {
        if has_more.get() {
            load_more_search_results(status, has_more, loading_more, Some(next));
        }
        return;
    }
    page.set(next);
    if len > 0 {
        selected.set((next * PAGE_SIZE).min(len - 1));
    }
    let generation = current_generation();
    request_visible_thumbnails(results.clone(), next, status, generation);
    request_selected_details(results, selected, details, generation);
}

fn previous_page(
    results: State<Vec<GuiSearchResult>>,
    page: State<usize>,
    selected: State<usize>,
    status: State<String>,
    details: State<DetailState>,
) {
    let previous = page.get().saturating_sub(1);
    page.set(previous);
    selected.set(previous * PAGE_SIZE);
    let generation = current_generation();
    request_visible_thumbnails(results.clone(), previous, status, generation);
    request_selected_details(results, selected, details, generation);
}

fn page_count(len: usize) -> usize {
    len.div_ceil(PAGE_SIZE).max(1)
}

fn load_search_page(mut cursor: YoutubeSearchCursor, limit: usize) -> SearchLoadOutcome {
    match cursor.next_page(limit) {
        Ok(page) => Ok(SearchLoadResult {
            results: page.results.into_iter().map(gui_search_result).collect(),
            has_more: page.has_more,
            cursor,
        }),
        Err(message) => Err(SearchLoadError {
            message,
            cursor: Some(cursor),
        }),
    }
}

fn load_more_search_results(
    status: State<String>,
    has_more: State<bool>,
    loading_more: State<bool>,
    target_page: Option<usize>,
) {
    if loading_more.get() || !has_more.get() {
        return;
    }
    let generation = current_generation();
    let cursor = {
        let mut cursor = YT_GUI_SEARCH_CURSOR.lock().expect("yt gui mutex poisoned");
        cursor.take()
    };
    let Some(cursor) = cursor else {
        status.set(String::from("Search results are still loading."));
        return;
    };
    loading_more.set(true);
    status.set(String::from("Loading more search results."));
    thread::spawn(move || {
        let result = load_search_page(cursor, PAGE_SIZE);
        push_gui_message(GuiMessage::SearchMoreFinished {
            generation,
            target_page,
            result,
        });
    });
}

fn gui_search_result(result: scarlet_youtube_net::YoutubeSearchResult) -> GuiSearchResult {
    GuiSearchResult {
        video_id: result.video_id,
        title: result.title,
        channel: result.channel,
        duration: result.duration,
        thumbnail: ThumbnailState::NotRequested,
        thumbnail_url: result.thumbnail_url,
    }
}

fn thumbnail_image(thumbnail: ThumbnailState) -> scarlet_ui::Image {
    match thumbnail {
        ThumbnailState::Ready(image) => scarlet_ui::Image::from_bitmap(image),
        _ => scarlet_ui::Image::placeholder(THUMB_WIDTH, THUMB_HEIGHT),
    }
}

fn request_visible_thumbnails(
    results: State<Vec<GuiSearchResult>>,
    page: usize,
    status: State<String>,
    generation: u64,
) {
    {
        let mut active = YT_GUI_THUMBNAIL_ACTIVE
            .lock()
            .expect("yt gui mutex poisoned");
        if *active {
            return;
        }
        *active = true;
    }

    let mut list = results.get();
    let start = page.saturating_mul(PAGE_SIZE);
    let end = start.saturating_add(PAGE_SIZE).min(list.len());
    let mut requests = Vec::new();

    for result in list.iter_mut().take(end).skip(start) {
        if result.thumbnail.is_not_requested() {
            if result.thumbnail_url.is_some() {
                result.thumbnail = ThumbnailState::Loading;
                requests.push(result.video_id.clone());
            } else {
                result.thumbnail = ThumbnailState::Failed;
            }
        }
    }

    if requests.is_empty() {
        *YT_GUI_THUMBNAIL_ACTIVE
            .lock()
            .expect("yt gui mutex poisoned") = false;
        return;
    }

    results.set(list);
    status.set(format!("Loading thumbnails for page {}.", page + 1));

    thread::spawn(move || {
        for video_id in requests {
            if generation != current_generation() {
                break;
            }
            let thumbnail = match download_thumbnail_image(&video_id) {
                Ok(image) => ThumbnailState::Ready(image),
                Err(error) => {
                    println!("[yt-gui] thumbnail {} failed: {}", video_id, error);
                    ThumbnailState::Failed
                }
            };
            push_gui_message(GuiMessage::ThumbnailFinished {
                generation,
                video_id,
                thumbnail,
            });
        }
        push_gui_message(GuiMessage::ThumbnailBatchFinished { generation });
    });
}

fn request_selected_details(
    results: State<Vec<GuiSearchResult>>,
    selected: State<usize>,
    details: State<DetailState>,
    generation: u64,
) {
    let list = results.get();
    let Some(row) = list.get(selected.get()) else {
        if !matches!(details.get(), DetailState::Empty) {
            details.set(DetailState::Empty);
        }
        return;
    };
    let video_id = row.video_id.clone();
    if detail_state_matches_video(&details.get(), &video_id) {
        return;
    }
    {
        let mut active = YT_GUI_DETAILS_ACTIVE.lock().expect("yt gui mutex poisoned");
        if *active {
            return;
        }
        *active = true;
    }

    details.set(DetailState::Loading {
        video_id: video_id.clone(),
    });
    thread::spawn(move || {
        let result = if generation == current_generation() {
            youtube_video_details(&video_id).map(GuiVideoDetails::from_youtube)
        } else {
            Err(String::from("stale details request"))
        };
        push_gui_message(GuiMessage::DetailsFinished {
            generation,
            video_id,
            result,
        });
    });
}

fn detail_state_matches_video(state: &DetailState, video_id: &str) -> bool {
    match state {
        DetailState::Loading { video_id: current }
        | DetailState::Failed {
            video_id: current, ..
        } => current == video_id,
        DetailState::Ready(details) => details.video_id == video_id,
        DetailState::Empty => false,
    }
}

fn download_thumbnail_image(video_id: &str) -> std::result::Result<BitmapImage, String> {
    let bytes = fetch_youtube_thumbnail_bytes(video_id)?;
    BitmapImage::from_jpeg_bytes(&bytes).ok_or_else(|| String::from("thumbnail JPEG decode failed"))
}

fn next_generation() -> u64 {
    let mut generation = YT_GUI_GENERATION.lock().expect("yt gui mutex poisoned");
    *generation = generation.saturating_add(1);
    *generation
}

fn current_generation() -> u64 {
    *YT_GUI_GENERATION.lock().expect("yt gui mutex poisoned")
}

fn push_gui_message(message: GuiMessage) {
    YT_GUI_MESSAGES
        .lock()
        .expect("yt gui mutex poisoned")
        .push(message);
}

fn pop_gui_message() -> Option<GuiMessage> {
    let mut messages = YT_GUI_MESSAGES.lock().expect("yt gui mutex poisoned");
    if messages.is_empty() {
        None
    } else {
        Some(messages.remove(0))
    }
}

fn compact_text(input: &str, max_chars: usize) -> String {
    let mut out = String::new();
    for (index, ch) in input.chars().enumerate() {
        if index >= max_chars {
            out.push_str("...");
            return out;
        }
        out.push(ch);
    }
    out
}

fn wrap_description_text(input: &str) -> String {
    let mut lines = Vec::new();
    let mut truncated = false;
    let mut consumed_chars = 0usize;

    for paragraph in input.replace('\r', "").split('\n') {
        if consumed_chars >= DETAIL_DESCRIPTION_MAX_CHARS {
            truncated = true;
            break;
        }
        if lines.len() >= DETAIL_DESCRIPTION_MAX_LINES {
            truncated = true;
            break;
        }

        let paragraph = paragraph.trim();
        if paragraph.is_empty() {
            if !lines.is_empty() {
                lines.push(String::new());
            }
            continue;
        }

        let mut current = String::new();
        for word in paragraph.split_whitespace() {
            if consumed_chars >= DETAIL_DESCRIPTION_MAX_CHARS
                || lines.len() >= DETAIL_DESCRIPTION_MAX_LINES
            {
                truncated = true;
                break;
            }

            let remaining = DETAIL_DESCRIPTION_MAX_CHARS - consumed_chars;
            let word = compact_text(word, remaining);
            consumed_chars = consumed_chars.saturating_add(word.chars().count());

            let candidate = if current.is_empty() {
                word.clone()
            } else {
                format!("{} {}", current, word)
            };

            if text_fits_detail_width(&candidate) {
                current = candidate;
                continue;
            }

            if !current.is_empty() {
                lines.push(current);
                current = String::new();
            }
            push_wrapped_word(&mut lines, &mut current, &word);
        }

        if !current.is_empty() && lines.len() < DETAIL_DESCRIPTION_MAX_LINES {
            lines.push(current);
        }
    }

    if lines.len() > DETAIL_DESCRIPTION_MAX_LINES {
        lines.truncate(DETAIL_DESCRIPTION_MAX_LINES);
        truncated = true;
    }

    if truncated {
        if let Some(last) = lines.last_mut() {
            append_ellipsis(last);
        } else {
            lines.push(String::from("..."));
        }
    }

    join_lines(&lines)
}

fn push_wrapped_word(lines: &mut Vec<String>, current: &mut String, word: &str) {
    for ch in word.chars() {
        if lines.len() >= DETAIL_DESCRIPTION_MAX_LINES {
            return;
        }
        let mut candidate = current.clone();
        candidate.push(ch);
        if current.is_empty() || text_fits_detail_width(&candidate) {
            current.push(ch);
        } else {
            lines.push(std::mem::take(current));
            current.push(ch);
        }
    }
}

fn text_fits_detail_width(text: &str) -> bool {
    let (width, _) = scarlet_ui::measure_text_sized(text, DETAIL_DESCRIPTION_FONT_SIZE);
    width <= DETAIL_TEXT_WIDTH
}

fn append_ellipsis(text: &mut String) {
    while !text.is_empty() {
        let candidate = format!("{}...", text);
        if text_fits_detail_width(&candidate) {
            text.push_str("...");
            return;
        }
        text.pop();
    }
    text.push_str("...");
}

fn join_lines(lines: &[String]) -> String {
    let mut out = String::new();
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(line);
    }
    out
}

fn detail_pane_text(
    state: DetailState,
    selected_video_id: &str,
    base_title: String,
    base_channel: String,
) -> (String, String, String) {
    match state {
        DetailState::Ready(details) if details.video_id == selected_video_id => {
            let title = details
                .title
                .as_deref()
                .map(|title| compact_text(title, 90))
                .unwrap_or(base_title);
            let channel = details
                .author
                .as_deref()
                .map(|author| compact_text(author, 46))
                .unwrap_or(base_channel);
            let description = details
                .description
                .as_deref()
                .map(str::trim)
                .filter(|description| !description.is_empty())
                .map(|description| wrap_description_text(description))
                .unwrap_or_else(|| String::from("No description."));
            let _thumbnail_url = details.thumbnail_url.as_deref();
            (title, channel, description)
        }
        DetailState::Loading { video_id } if video_id == selected_video_id => {
            (base_title, base_channel, String::from("Loading details..."))
        }
        DetailState::Failed { video_id, message } if video_id == selected_video_id => (
            base_title,
            base_channel,
            format!("Details failed: {}", compact_text(&message, 180)),
        ),
        _ if selected_video_id == "-" => (
            base_title,
            base_channel,
            String::from("Search and select a result."),
        ),
        _ => (base_title, base_channel, String::from("Loading details...")),
    }
}

#[cfg(not(test))]
fn initial_query() -> String {
    let args: Vec<String> = std::env::args().collect();
    let mut query = String::new();
    for arg in args.iter().skip(1) {
        if !query.is_empty() {
            query.push(' ');
        }
        query.push_str(arg);
    }
    query
}

#[cfg(not(test))]
fn main() {
    println!("[yt-gui] Starting YouTube GUI");

    let mut app = YtGuiApp::new(initial_query());
    match app.run() {
        Ok(()) => println!("[yt-gui] exited"),
        Err(error) => println!("[yt-gui] error: {}", error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scarlet_ui::SceneBuilder;
    use scarlet_ui::event::KeyModifiers;
    use scarlet_ui::pipeline::RenderingPipeline;
    fn press(app: &YtGuiApp, keycode: KeyCode) {
        assert!(app.handle_key(KeyEvent::Pressed {
            keycode,
            modifiers: KeyModifiers::empty()
        }));
        app.handle_key(KeyEvent::Released {
            keycode,
            modifiers: KeyModifiers::empty(),
        });
    }
    fn button_bounds(
        element: &dyn scarlet_ui::Element,
        label: &str,
        origin: scarlet_ui::Point,
    ) -> Option<scarlet_ui::Rect> {
        let position = element.position();
        let origin = scarlet_ui::Point {
            x: origin.x + position.x,
            y: origin.y + position.y,
        };
        if let Some(button)=element.as_any().downcast_ref::<scarlet_ui::RenderElement<Button,scarlet_ui::views::ButtonRenderObject>>() {
            if button.view().label()==label {
                let size=element.bounds().size;
                return Some(scarlet_ui::Rect::from_xywh(origin.x,origin.y,size.width,size.height));
            }
        }
        element
            .children()
            .iter()
            .find_map(|child| button_bounds(child.as_ref(), label, origin))
    }
    fn refresh_window(app: &YtGuiApp, pipeline: &mut RenderingPipeline, width: f32, height: f32) {
        for _ in 0..3 {
            let mut scenes = SceneBuilder::new();
            app.scenes().build(&mut scenes);
            pipeline.set_root(scenes.into_declarations().remove(0).view.create_element());
            pipeline.layout_initial();
            pipeline.resize(Size::new(width, height));
            pipeline.render();
        }
    }
    fn fixture() -> YtGuiApp {
        // These tests exercise UI state, never Scarlet syscalls or YouTube I/O.
        *YT_GUI_DETAILS_ACTIVE.lock().unwrap() = true;
        *YT_GUI_PLAYBACK_ACTIVE.lock().unwrap() = false;
        let app = YtGuiApp::new("Scarlet OS".into());
        app.results.set(
            (0..29)
                .map(|i| GuiSearchResult {
                    video_id: format!("video-{i}"),
                    title: format!("Scarlet OS · Episode {}", i + 1),
                    channel: Some("Scarlet Studio".into()),
                    duration: Some("12:34".into()),
                    thumbnail: ThumbnailState::NotRequested,
                    thumbnail_url: None,
                })
                .collect(),
        );
        app.control.set(1);
        app.list_focused.set(true);
        app.status.set("Ready to watch".into());
        app
    }
    #[test]
    fn confirm_repeat_cannot_play_when_opening_a_video() {
        let app = fixture();
        let event = KeyEvent::Pressed {
            keycode: KeyCode::Enter,
            modifiers: KeyModifiers::empty(),
        };
        app.handle_key(event);
        assert_eq!(app.screen.get(), 1);
        assert!(!*YT_GUI_PLAYBACK_ACTIVE.lock().unwrap());
        app.status.set("held guard".into());
        for _ in 0..12 {
            app.handle_key(event);
        }
        assert_eq!(app.status.get(), "held guard");
        press(&app, KeyCode::Escape);
        assert_eq!(app.screen.get(), 0);
        assert_eq!(app.selected.get(), 0);
        assert_eq!(app.control.get(), 1);
    }
    #[test]
    fn selection_pages_back_and_rotation_preserve_video() {
        let mut app = fixture();
        app.page_bounds.set(Size::new(520.0, 580.0));
        for _ in 0..6 {
            press(&app, KeyCode::Right);
        }
        for _ in 0..3 {
            press(&app, KeyCode::Down);
        }
        assert_eq!((app.selected.get(), app.page.get()), (12, 1));
        press(&app, KeyCode::Enter);
        assert_eq!(app.screen.get(), 1);
        press(&app, KeyCode::Escape);
        assert_eq!((app.selected.get(), app.page.get()), (12, 1));
        let ctx = scarlet_ui::WindowContext {
            window_id: scarlet_ui::WindowId::generate(),
            scene_key: "main".into(),
            pipeline_id: scarlet_ui::pipeline::PipelineId::generate(),
            platform_window_id: 1,
            is_primary: true,
        };
        app.on_window_resize(&ctx, 390, 844);
        assert_eq!(app.selected.get(), 12);
        app.on_window_resize(&ctx, 844, 390);
        assert_eq!(app.selected.get(), 12);
        for _ in 0..40 {
            press(&app, KeyCode::Down);
        }
        assert_eq!(app.selected.get(), 28);
        press(&app, KeyCode::Escape);
        assert_eq!(app.control.get(), 0);
    }
    #[test]
    fn platform_search_cancellation_restores_focus_and_query() {
        let app = fixture();
        app.query.set("猫".into());
        app.control.set(0);
        press(&app, KeyCode::Enter);
        assert_eq!(app.screen.get(), 2);
        assert!(!app.list_focused.get());
        app.cancel_search();
        assert_eq!(app.screen.get(), 0);
        assert_eq!(app.control.get(), 0);
        assert_eq!(app.query.get(), "猫");
        assert!(app.list_focused.get());
    }
    #[test]
    fn stale_search_cannot_replace_new_results_and_playback_restores_focus() {
        let app = fixture();
        let selected = app.selected.get();
        app.handle_message(GuiMessage::SearchFinished {
            generation: current_generation() + 1,
            result: Err(SearchLoadError {
                message: "obsolete".into(),
                cursor: None,
            }),
        });
        assert_eq!(app.results.get().len(), 29);
        assert_eq!(app.selected.get(), selected);
        app.screen.set(1);
        app.handle_message(GuiMessage::PlaybackFinished {
            title: "Episode 1".into(),
            result: Ok(()),
        });
        assert_eq!(app.screen.get(), 0);
        assert_eq!(app.control.get(), 1);
        assert!(app.list_focused.get());
    }
    #[test]
    fn native_touch_cancel_and_video_selection_then_play() {
        use scarlet_ui::Event;
        use scarlet_ui::event::{TouchChange, TouchFrame, TouchPhase};
        let app = fixture();
        let mut pipeline = RenderingPipeline::new();
        pipeline.set_root(
            app.video_card(0, app.selected_result().unwrap(), 300.0, false)
                .create_element(),
        );
        pipeline.layout_initial();
        let touch = |serial, phase, x, y| {
            Event::TouchFrame(TouchFrame {
                seat_id: 0,
                serial,
                time_ns: serial * 16_000_000,
                changes: vec![TouchChange {
                    seat_id: 0,
                    serial,
                    time_ns: serial * 16_000_000,
                    id: 1,
                    phase,
                    x,
                    y,
                    pressure: None,
                    touch_major: None,
                }],
            })
        };
        pipeline.handle_event(&touch(1, TouchPhase::Down, 100, 50));
        pipeline.handle_event(&touch(2, TouchPhase::Cancel, 100, 50));
        assert_eq!(app.screen.get(), 0);
        pipeline.handle_event(&touch(3, TouchPhase::Down, 100, 50));
        pipeline.handle_event(&touch(4, TouchPhase::Up, 100, 50));
        assert_eq!(app.screen.get(), 1);
        assert!(!*YT_GUI_PLAYBACK_ACTIVE.lock().unwrap());
        pipeline.set_root(
            app.detail_view(280.0, 420.0)
                .frame(300.0, 420.0)
                .create_element(),
        );
        pipeline.layout_initial();
        pipeline.resize(Size::new(300.0, 420.0));
        let rect = button_bounds(
            pipeline.element_tree().root().unwrap(),
            "Play video",
            scarlet_ui::Point::ZERO,
        )
        .expect("one native playback button");
        assert_eq!(rect.size.height, 28.0, "desktop uses compact controls");
        assert!(
            rect.origin.y + rect.size.height <= 420.0,
            "play remains within the viewport"
        );
        let x = (rect.origin.x + rect.size.width / 2.0) as i32;
        let y = (rect.origin.y + rect.size.height / 2.0) as i32;
        pipeline.handle_event(&touch(5, TouchPhase::Down, x, y));
        pipeline.handle_event(&touch(6, TouchPhase::Up, x, y));
        assert!(
            *YT_GUI_PLAYBACK_ACTIVE.lock().unwrap(),
            "native Play video button starts playback"
        );
        pipeline.teardown();
    }

    #[test]
    fn native_search_focus_release_submit_and_cancel() {
        use scarlet_ui::Event;
        let app = fixture();
        app.control.set(0);
        let mut pipeline = RenderingPipeline::new();
        refresh_window(&app, &mut pipeline, 800.0, 600.0);
        assert!(
            button_bounds(
                pipeline.element_tree().root().unwrap(),
                "Play video",
                scarlet_ui::Point::ZERO
            )
            .is_none(),
            "browse contains no playback buttons"
        );
        let key = |keycode, pressed| {
            Event::Keyboard(if pressed {
                KeyEvent::Pressed {
                    keycode,
                    modifiers: KeyModifiers::empty(),
                }
            } else {
                KeyEvent::Released {
                    keycode,
                    modifiers: KeyModifiers::empty(),
                }
            })
        };
        pipeline.handle_event(&key(KeyCode::Enter, true));
        assert_eq!(app.screen.get(), 2);
        refresh_window(&app, &mut pipeline, 800.0, 600.0);
        pipeline.handle_event(&key(KeyCode::Enter, false));
        assert!(
            !app.confirm_held.get(),
            "release reaches the owner after TextField gains focus"
        );
        pipeline.handle_event(&key(KeyCode::Escape, true));
        assert_eq!(app.screen.get(), 0);
        assert_eq!(app.query.get(), "Scarlet OS");
        refresh_window(&app, &mut pipeline, 800.0, 600.0);
        pipeline.handle_event(&key(KeyCode::Enter, true));
        refresh_window(&app, &mut pipeline, 800.0, 600.0);
        pipeline.handle_event(&key(KeyCode::Enter, false));
        refresh_window(&app, &mut pipeline, 800.0, 600.0);
        pipeline.handle_event(&key(KeyCode::Enter, true));
        assert_eq!(app.screen.get(), 0, "native submit returns to results");
        assert!(app.results.get().is_empty());
        pipeline.handle_event(&key(KeyCode::Enter, true));
        assert!(
            !*YT_GUI_PLAYBACK_ACTIVE.lock().unwrap(),
            "repeat submit cannot launch a video"
        );
        pipeline.handle_event(&key(KeyCode::Enter, false));
        refresh_window(&app, &mut pipeline, 800.0, 600.0);
        assert!(
            pipeline.element_tree().focused_text_input_state().is_none(),
            "submission gives focus back to browsing"
        );
        pipeline.teardown();
    }

    #[test]
    fn render_every_screen_at_console_tablet_and_small_sizes() {
        let app = fixture();
        let directory = std::env::var("YT_QA_DIR").ok();
        for (width, height) in [
            (1024, 680),
            (1280, 720),
            (768, 1024),
            (390, 844),
            (844, 390),
            (320, 240),
        ] {
            for screen in 0..3 {
                app.bounds
                    .set(content_size(Size::new(width as f32, height as f32)));
                app.screen.set(screen);
                let mut pipeline = RenderingPipeline::new();
                for _ in 0..3 {
                    let mut scenes = SceneBuilder::new();
                    app.scenes().build(&mut scenes);
                    pipeline.set_root(scenes.into_declarations().remove(0).view.create_element());
                    pipeline.layout_initial();
                    pipeline.resize(Size::new(width as f32, height as f32));
                    pipeline.render();
                }
                pipeline.request_redraw();
                let buffer = pipeline.render().expect("CPU rendering");
                assert!(buffer.as_slice().iter().any(|p| *p != 0));
                if let Some(directory) = &directory {
                    let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
                    for pixel in buffer.as_slice() {
                        bytes.extend_from_slice(&[
                            ((pixel >> 16) & 255) as u8,
                            ((pixel >> 8) & 255) as u8,
                            (pixel & 255) as u8,
                        ]);
                    }
                    std::fs::write(
                        format!("{directory}/pipeline-{width}x{height}-{screen}.ppm"),
                        bytes,
                    )
                    .unwrap();
                }
                pipeline.teardown();
            }
        }
    }
}
