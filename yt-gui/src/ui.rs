use super::*;
use scarlet_ui::{NavigationLink, NavigationView};

impl YtGuiApp {
    pub(super) fn video_card(
        &self,
        index: usize,
        row: GuiSearchResult,
        width: f32,
        compact: bool,
    ) -> impl View + Clone + use<> {
        let palette = ColorPalette::default();
        let active = index == self.selected.get() && self.control.get() == 1;
        let select = self.clone();
        let image_width = if compact {
            96.0
        } else {
            (width - 8.0).min(208.0)
        };
        let image_height = image_width * 9.0 / 16.0;
        let thumbnail = zstack! {
            thumbnail_image(row.thumbnail).fit_mode(ImageFit::Fill).frame(image_width,image_height).clip_radius(4.0),
            Text::new(row.duration.unwrap_or_default()).font_size(11.0).color(Color::WHITE).padding(3.0).background(Color::BLACK).clip_radius(3.0).alignment(Alignment::BottomTrailing).padding(5.0),
        }.frame(image_width,image_height);
        let text_width = if compact { width - 116.0 } else { width - 8.0 };
        let metadata = vstack! {
            Text::new(wrap_title(&row.title,text_width,14.0,2)).font_size(14.0).alignment(Alignment::TopLeading).frame(text_width,34.0),
            Text::new(compact_text(&row.channel.unwrap_or_default(),(text_width/7.0) as usize)).font_size(12.0).color(palette.text_secondary()).alignment(Alignment::Leading).frame(text_width,14.0),
        }.spacing(3.0);
        let content = if compact {
            Either::A(hstack! {thumbnail,metadata}.spacing(12.0).padding(4.0))
        } else {
            Either::B(vstack! {thumbnail,metadata}.spacing(4.0).padding(4.0))
        };
        Surface::new(content, SurfaceRole::Canvas)
            .fill(palette.background())
            .border_color(if active {
                palette.primary()
            } else {
                Color::CLEAR
            })
            .corner_radius(4.0)
            .frame(width, if compact { 64.0 } else { image_height + 63.0 })
            .on_click(move || select.open_details(index))
    }

    fn browse_view(&self, width: f32, height: f32) -> impl View + Clone + use<> {
        let columns = gallery_columns(width, height);
        let compact = compact_results(width, height);
        let cell_width = ((width - (columns - 1) as f32 * 12.0) / columns as f32).min(if compact {
            f32::INFINITY
        } else {
            216.0
        });
        let row_height = if compact {
            64.0
        } else {
            (cell_width - 8.0).min(208.0) * 9.0 / 16.0 + 63.0
        };
        let start = self.page.get() * PAGE_SIZE;
        let page_items: Vec<_> = self
            .results
            .get()
            .into_iter()
            .enumerate()
            .skip(start)
            .take(PAGE_SIZE)
            .collect();
        let rows = page_items.len().div_ceil(columns);
        let app = self.clone();
        let items = State::new(scarlet_ui::state::generate_state_id(), page_items);
        let selected = State::new(
            scarlet_ui::state::generate_state_id(),
            if self.control.get() == 1 {
                Some(self.selected.get().saturating_sub(start))
            } else {
                None
            },
        );
        let grid = GridView::new(
            items,
            selected,
            columns,
            row_height,
            move |_, (index, row), _| app.video_card(index, row, cell_width, compact),
        )
        .spacing(12.0)
        .row_spacing(12.0)
        .frame(width, (rows as f32 * (row_height + 12.0) - 12.0).max(0.0));
        let gallery = ScrollView::new(grid)
            .scroll_to_index(
                Some((self.selected.get() % PAGE_SIZE) / columns),
                row_height + 12.0,
            )
            .frame(width, height);
        if self.results.get().is_empty() {
            Either::A(vstack! {
                IconView::new(Icon::Search).size(IconSize::Large).color(ColorPalette::default().text_secondary()),
                Text::new(if self.loading_more.get() {"Searching YouTube…"} else {"Search YouTube"}).font_size(18.0),
                Text::new(if self.loading_more.get() {"Loading results"} else {"Enter a title, channel, or topic"}).font_size(14.0).color(muted()),
            }.spacing(14.0).frame(width,height))
        } else {
            Either::B(gallery)
        }
    }

    fn header_view(&self) -> impl View + Clone + use<> {
        let width = self.header_bounds.get().width;
        let button_width = control_height() + 4.0;
        let prev = self.clone();
        let next = self.clone();
        let back = self.clone();
        let submit = self.clone();
        let search = self.clone();
        let cancel = self.clone();
        // Navigation chrome owns search and paging. Playback belongs to video content.
        let controls = if self.screen.get() == 1 {
            Either::A(hstack! {
                native_button(String::new(),Icon::ArrowLeft,self.control.get()==0,button_width,move || {back.screen.set(0);back.control.set(1);}),
                Text::new("Video details").font_size(16.0).color(muted()),
                Spacer::new(),
            }.spacing(12.0))
        } else {
            Either::B(hstack! {
                TextField::new(self.query.clone())
                    .placeholder("Search YouTube")
                    .autofocus(self.screen.get()==2)
                    .font_size(control_font_size()).padding(6.0)
                    .blur_on_submit(!self.confirm_held.get())
                    .on_submit(move || submit.submit_search_from_key())
                    .on_cancel(move || cancel.cancel_search())
                    .border_color(if self.control.get()==0 {ColorPalette::default().primary()} else {ColorPalette::default().border()})
                    .frame((width-3.0*button_width-32.0).max(64.0),control_height()),
                native_button(String::new(),Icon::Search,false,button_width,move || {
                    if search.query.get().trim().is_empty() {search.begin_search();} else {search.submit_search();}
                }),
                native_button(String::new(),Icon::ChevronLeft,self.control.get()==2,button_width,move || previous_page(prev.results.clone(),prev.page.clone(),prev.selected.clone(),prev.status.clone(),prev.details.clone())),
                native_button(String::new(),Icon::ChevronRight,self.control.get()==3,button_width,move || next_page(next.results.clone(),next.page.clone(),next.selected.clone(),next.status.clone(),next.has_more.clone(),next.loading_more.clone(),next.details.clone())),
            }.spacing(8.0))
        };
        let bounds = self.header_bounds.clone();
        HeaderBar::new(controls.padding(4.0))
            .height(header_height())
            .on_geometry_change(
                |geometry| geometry.size(),
                move |size| {
                    if bounds.get() != size {
                        bounds.set(size);
                    }
                },
            )
    }

    pub(super) fn detail_view(&self, width: f32, height: f32) -> impl View + Clone + use<> {
        let row = self.selected_result();
        let (title, channel, id, thumb) = row
            .map(|r| {
                (
                    r.title,
                    r.channel.unwrap_or_default(),
                    r.video_id,
                    r.thumbnail,
                )
            })
            .unwrap_or((
                "Video details".into(),
                String::new(),
                String::new(),
                ThumbnailState::NotRequested,
            ));
        let (title, channel, description) =
            detail_pane_text(self.details.get(), &id, title, channel);
        let horizontal = width >= 440.0 || height < 320.0;
        let tiny = height < 180.0;
        let image_width = if tiny {
            96.0
        } else if horizontal {
            (width * 0.4).min(240.0)
        } else {
            width.min(320.0)
        };
        let image_height = image_width * 9.0 / 16.0;
        let meta_width = if horizontal {
            width - image_width - 16.0
        } else {
            width
        };
        let font = if tiny { 16.0 } else { 20.0 };
        let title = wrap_title(&title, meta_width, font, if tiny { 1 } else { 3 });
        let title_height = title.lines().count().max(1) as f32 * if tiny { 20.0 } else { 24.0 };
        let play = self.clone();
        let poster_play = self.clone();
        let poster = thumbnail_image(thumb)
            .fit_mode(ImageFit::Fill)
            .frame(image_width, image_height)
            .clip_radius(4.0)
            .on_click(move || poster_play.play_video(poster_play.selected.get()));
        let title_view = Text::new(title)
            .font_size(font)
            .alignment(Alignment::TopLeading)
            .frame(meta_width, title_height);
        let play_button = video_button(
            "Play video",
            Icon::PlayerPlay,
            self.control.get() == 1,
            104.0,
            move || play.play_video(play.selected.get()),
        )
        .alignment(Alignment::Leading)
        .frame(meta_width, control_height());
        let metadata = if tiny {
            Either::A(
                vstack! {title_view,play_button}
                    .spacing(6.0)
                    .frame(meta_width, title_height + control_height() + 6.0),
            )
        } else {
            Either::B(vstack! {
                title_view,
                Text::new(channel).font_size(14.0).color(muted()).alignment(Alignment::Leading).frame(meta_width,18.0),
                play_button,
            }.spacing(8.0).frame(meta_width,title_height+18.0+control_height()+16.0))
        };
        let overview = if horizontal {
            Either::A(
                hstack! {poster,metadata}
                    .spacing(16.0)
                    .alignment(Alignment::TopLeading)
                    .frame(
                        width,
                        image_height
                            .max(title_height + control_height() + if tiny { 6.0 } else { 34.0 }),
                    ),
            )
        } else {
            Either::B(
                vstack! {
                    poster.alignment(Alignment::Leading).frame(width,image_height),
                    metadata,
                }
                .spacing(12.0),
            )
        };
        let description = wrap_title(&description, width, 14.0, 24);
        let description_height = description.lines().count().max(1) as f32 * 18.0;
        vstack! {
            overview,
            Divider::new().frame(width,1.0),
            Text::new(description).font_size(14.0).color(ColorPalette::default().text_primary()).alignment(Alignment::TopLeading).frame(width,description_height),
        }.spacing(12.0).padding(if tiny {4.0} else {8.0})
    }

    fn search_content(&self, width: f32, height: f32) -> impl View + Clone + use<> {
        vstack! {
            IconView::new(Icon::Search).size(IconSize::Large).color(muted()),
            Text::new("Search YouTube").font_size(22.0),
            Text::new("Enter a title, channel, or topic above").font_size(14.0).color(muted()),
        }
        .spacing(14.0)
        .frame(width, height)
    }

    pub(super) fn page_view(&self) -> impl View + Clone + use<> {
        let layout = PageLayout::new(self.page_bounds.get());
        let width = layout.width;
        let body=match self.screen.get() {
            1=>Either3::A(ScrollView::new(self.detail_view((width-16.0).max(160.0),layout.content_height)).frame(width,layout.content_height)),
            2=>Either3::B(self.search_content(width,layout.content_height)),
            _=>Either3::C(vstack! {
                Text::new(if self.results.get().is_empty() {String::new()} else {format!("{} results{}  ·  Page {}",self.results.get().len(),if self.has_more.get() {"+"} else {""},self.page.get()+1)})
                    .font_size(13.0).color(muted())
                    .alignment(Alignment::Leading).frame(width,layout.caption_height),
                self.browse_view(width,layout.gallery_height),
            }.spacing(if layout.caption_height>0.0 {8.0} else {0.0})),
        };
        let status = self.status.get();
        let footer = if self.loading_more.get()
            || status.contains("failed:")
            || status.starts_with("Starting playback")
        {
            status
        } else if self.screen.get() == 2 {
            "Confirm: search  ·  Cancel: videos".into()
        } else if self.screen.get() == 1 {
            if self.control.get() == 0 {
                "Confirm: videos  ·  Down: play".into()
            } else {
                "Confirm: play  ·  Cancel: videos".into()
            }
        } else {
            match self.control.get() {
                0 => "Confirm: search  ·  Down: videos".into(),
                2 => "Confirm: previous page  ·  Down: videos".into(),
                3 => "Confirm: next page  ·  Down: videos".into(),
                _ => "D-pad: move  ·  Confirm: open  ·  Cancel: search".into(),
            }
        };
        let bounds = self.page_bounds.clone();
        Surface::new(
            vstack! {
                body,
                Text::new(compact_text(&footer,(width/6.0) as usize))
                    .font_size(if layout.footer_height<16.0 {11.0} else {12.0}).color(muted())
                    .alignment(Alignment::Leading).frame(width,layout.footer_height),
            }
            .spacing(layout.gap)
            .padding(layout.padding)
            .frame(f32::INFINITY, f32::INFINITY),
            SurfaceRole::Canvas,
        )
        .on_geometry_change(
            |geometry| geometry.size(),
            move |size| {
                if bounds.get() != size {
                    bounds.set(size);
                }
            },
        )
    }

    pub(super) fn window_view(&self) -> impl Scene {
        let videos = self.clone();
        let search = self.clone();
        let header = self.clone();
        let select_videos = self.clone();
        let select_search = self.clone();
        let navigation = NavigationView::new_with_state_key(
            (
                NavigationLink::new("Videos", move || videos.page_view())
                    .icon(Icon::PlayerPlay)
                    .on_select(move || {
                        select_videos.screen.set(0);
                        select_videos.control.set(1);
                        select_videos.list_focused.set(true);
                    }),
                NavigationLink::new("Search", move || search.page_view())
                    .icon(Icon::Search)
                    .on_select(move || select_search.begin_search()),
            ),
            20_010,
        )
        .shows_icons(true)
        .sidebar_width(116.0)
        .header(move || header.header_view())
        .header_height(header_height());
        let destination = if self.screen.get() == 2 { 1 } else { 0 };
        if navigation.selected_index_state().get() != destination {
            navigation.selected_index_state().set(destination);
        }
        let app = self.clone();
        Window::new(
            "YouTube",
            navigation
                .frame(f32::INFINITY, f32::INFINITY)
                .focusable(self.list_focused.clone())
                .on_key(move |event| app.handle_key(event)),
        )
        .shadow(false)
        .app_id("org.scarlet-os.yt-gui")
    }
}

fn muted() -> Color {
    ColorPalette::default().text_secondary()
}
fn native_button<F: Fn() + 'static>(
    label: String,
    icon: Icon,
    active: bool,
    width: f32,
    action: F,
) -> impl View + Clone + use<F> {
    let palette = ColorPalette::default();
    Button::new(label)
        .on_click(action)
        .icon(icon)
        .icon_size(IconSize::Medium)
        .header_style()
        .font_size(control_font_size())
        .padding(6.0)
        .background_color(if active {
            focus_fill()
        } else {
            palette.background_secondary()
        })
        .border_color(if active {
            palette.primary()
        } else {
            Color::CLEAR
        })
        .frame(width, control_height())
}
fn focus_fill() -> Color {
    let palette = ColorPalette::default();
    palette
        .primary()
        .lighten(0.08)
        .with_opacity(0.14)
        .blend_over(palette.background())
}
fn video_button<F: Fn() + 'static>(
    label: &str,
    icon: Icon,
    active: bool,
    width: f32,
    action: F,
) -> impl View + Clone + use<F> {
    let palette = ColorPalette::default();
    Button::new(label)
        .on_click(action)
        .icon(icon)
        .icon_size(IconSize::Medium)
        .font_size(14.0)
        .background_color(if active {
            focus_fill()
        } else {
            palette.background_secondary()
        })
        .border_color(if active {
            palette.primary()
        } else {
            palette.border()
        })
        .frame(width, control_height())
}
fn control_height() -> f32 {
    if scarlet_ui::current_input_environment().interaction_mode()
        == scarlet_ui::InteractionMode::Touch
    {
        44.0
    } else {
        28.0
    }
}
fn control_font_size() -> f32 {
    if control_height() == 44.0 { 16.0 } else { 14.0 }
}
fn header_height() -> f32 {
    control_height() + 12.0
}
fn compact_results(width: f32, height: f32) -> bool {
    width < 440.0
        || (height < 250.0
            && scarlet_ui::current_input_environment().windowing_mode()
                != Some(scarlet_ui::WindowingMode::Focused))
}
pub(super) fn gallery_columns(width: f32, height: f32) -> usize {
    if width < 440.0 {
        1
    } else if compact_results(width, height) {
        ((width + 16.0) / 256.0).floor().clamp(1.0, 4.0) as usize
    } else {
        ((width + 16.0) / 192.0).floor().clamp(1.0, 6.0) as usize
    }
}
fn wrap_title(text: &str, width: f32, font: f32, max_lines: usize) -> String {
    let mut remaining = text.trim().replace('\r', "");
    let mut lines = Vec::new();
    while !remaining.is_empty() && lines.len() < max_lines {
        let mut end = 0;
        let mut word_break = None;
        for (offset, ch) in remaining.char_indices() {
            if ch == '\n' {
                break;
            }
            let next = offset + ch.len_utf8();
            if graphics::measure_text_sized(&remaining[..next], font).0 as f32 > width && end > 0 {
                break;
            }
            end = next;
            if ch.is_whitespace() {
                word_break = Some(next);
            }
        }
        if end == 0 {
            remaining = remaining[1..].trim_start().to_string();
            continue;
        }
        if end < remaining.len() && !remaining[end..].starts_with(char::is_whitespace) {
            if let Some(boundary) = word_break {
                end = boundary;
            }
        }
        let mut line = remaining[..end].trim().to_string();
        remaining = remaining[end..].trim_start().to_string();
        if lines.len() + 1 == max_lines && !remaining.is_empty() {
            while !line.is_empty()
                && graphics::measure_text_sized(&format!("{}…", line), font).0 as f32 > width
            {
                line.pop();
            }
            line.push('…');
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// Geometry received from NavigationView excludes its header and navigation rail.
/// Only the caption, footer and page padding are reserved here.
#[derive(Clone, Copy, Debug)]
pub(super) struct PageLayout {
    pub width: f32,
    pub content_height: f32,
    pub gallery_height: f32,
    pub padding: f32,
    pub caption_height: f32,
    pub footer_height: f32,
    pub gap: f32,
}
impl PageLayout {
    pub fn new(size: Size) -> Self {
        let tiny = size.height < 180.0;
        let padding = if tiny {
            4.0
        } else if size.height < 300.0 {
            8.0
        } else {
            12.0
        };
        let footer_height = if tiny { 12.0 } else { 16.0 };
        let gap = if tiny { 4.0 } else { 8.0 };
        let caption_height = if tiny { 0.0 } else { 20.0 };
        let content_height = (size.height - padding * 2.0 - footer_height - gap).max(48.0);
        Self {
            width: (size.width - padding * 2.0).max(200.0),
            content_height,
            gallery_height: (content_height - caption_height - if tiny { 0.0 } else { 8.0 })
                .max(48.0),
            padding,
            caption_height,
            footer_height,
            gap,
        }
    }
}

#[cfg(test)]
mod text_tests {
    use super::*;

    #[test]
    fn titles_wrap_at_words_and_truncate_with_visible_ellipsis() {
        let width = graphics::measure_text_sized("Install IPA Files", 14.0).0 as f32;
        let title = wrap_title("Install IPA Files on a Tablet Computer", width, 14.0, 2);
        assert_eq!(title.lines().count(), 2);
        assert!(title.starts_with("Install IPA Files\n"));
        assert!(title.ends_with('…'));
        for line in title.lines() {
            assert!(graphics::measure_text_sized(line, 14.0).0 as f32 <= width);
        }
        assert_eq!(
            wrap_title("First paragraph\nSecond paragraph", 500.0, 14.0, 4),
            "First paragraph\nSecond paragraph"
        );
    }
}
