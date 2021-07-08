use std::thread::{self, JoinHandle};

use crate::{
    cmd,
    controller::InputController,
    data::{
        AppState, AudioQuality, Authentication, Config, Preferences, PreferencesTab, Promise, Theme,
    },
    ui::{icons::SvgIcon, theme, utils::Border},
    widget::{icons, Empty, LinkExt},
};
use druid::{
    commands,
    widget::{
        Button, Controller, CrossAxisAlignment, Flex, Label, LineBreaking, MainAxisAlignment,
        RadioGroup, TextBox, ViewSwitcher,
    },
    Env, Event, EventCtx, LifeCycle, LifeCycleCtx, Selector, Widget, WidgetExt,
};
use psst_core::connection::Credentials;

pub fn preferences_widget() -> impl Widget<AppState> {
    let tabs = tabs_widget()
        .padding(theme::grid(2.0))
        .background(theme::BACKGROUND_LIGHT);

    let active = ViewSwitcher::new(
        |state: &AppState, _env| state.preferences.active,
        |active: &PreferencesTab, _state, _env| match active {
            PreferencesTab::General => general_tab_widget().boxed(),
            PreferencesTab::Cache => cache_tab_widget().boxed(),
        },
    )
    .padding(theme::grid(4.0))
    .background(Border::Top.with_color(theme::GREY_500));

    Flex::column()
        .must_fill_main_axis(true)
        .cross_axis_alignment(CrossAxisAlignment::Fill)
        .with_child(tabs)
        .with_child(active)
}

fn tabs_widget() -> impl Widget<AppState> {
    let label = |text, icon: &SvgIcon, tab: PreferencesTab| {
        Flex::column()
            .with_child(icon.scale(theme::ICON_SIZE_LARGE))
            .with_default_spacer()
            .with_child(Label::new(text).with_font(theme::UI_FONT_MEDIUM))
            .padding(theme::grid(1.0))
            .link()
            .rounded(theme::BUTTON_BORDER_RADIUS)
            .env_scope({
                move |env, state: &AppState| {
                    if tab == state.preferences.active {
                        env.set(theme::LINK_COLD_COLOR, env.get(theme::BACKGROUND_DARK));
                        env.set(theme::TEXT_COLOR, env.get(theme::FOREGROUND_LIGHT));
                    } else {
                        env.set(theme::LINK_COLD_COLOR, env.get(theme::BACKGROUND_LIGHT));
                    }
                }
            })
            .on_click(move |_ctx, state: &mut AppState, _env| {
                state.preferences.active = tab;
            })
    };
    Flex::row()
        .must_fill_main_axis(true)
        .main_axis_alignment(MainAxisAlignment::Center)
        .with_child(label(
            "General",
            &icons::PREFERENCES,
            PreferencesTab::General,
        ))
        .with_default_spacer()
        .with_child(label("Cache", &icons::STORAGE, PreferencesTab::Cache))
}

fn general_tab_widget() -> impl Widget<AppState> {
    let mut col = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    // Theme
    col = col
        .with_child(Label::new("Theme").with_font(theme::UI_FONT_MEDIUM))
        .with_spacer(theme::grid(2.0))
        .with_child(
            RadioGroup::new(vec![("Light", Theme::Light), ("Dark", Theme::Dark)])
                .lens(Config::theme)
                .lens(AppState::config),
        );

    col = col.with_spacer(theme::grid(3.0));

    // Authentication
    col = col
        .with_child(Label::new("Credentials").with_font(theme::UI_FONT_MEDIUM))
        .with_spacer(theme::grid(2.0))
        .with_child(
            TextBox::new()
                .with_placeholder("Username")
                .controller(InputController::new())
                .env_scope(|env, _state| env.set(theme::WIDE_WIDGET_WIDTH, theme::grid(16.0)))
                .lens(Authentication::username)
                .lens(Preferences::auth)
                .lens(AppState::preferences),
        )
        .with_spacer(theme::grid(1.0))
        .with_child(
            TextBox::new()
                .with_placeholder("Password")
                .controller(InputController::new())
                .env_scope(|env, _state| env.set(theme::WIDE_WIDGET_WIDTH, theme::grid(16.0)))
                .lens(Authentication::password)
                .lens(Preferences::auth)
                .lens(AppState::preferences),
        )
        .with_spacer(theme::grid(1.0))
        .with_child(
            Flex::row()
                .with_child(Button::new("Log In").on_click(|ctx, _, _| {
                    ctx.submit_command(Authenticate::REQUEST);
                }))
                .with_spacer(theme::grid(1.0))
                .with_child(
                    ViewSwitcher::new(
                        |auth: &Authentication, _| auth.result.to_owned(),
                        |result, _, _| match result {
                            Promise::Empty => Empty.boxed(),
                            Promise::Deferred(_) => Label::new("Logging In...")
                                .with_text_size(theme::TEXT_SIZE_SMALL)
                                .boxed(),
                            Promise::Resolved(_) => Label::new("Success.")
                                .with_text_size(theme::TEXT_SIZE_SMALL)
                                .boxed(),
                            Promise::Rejected(message) => Label::new(message.to_owned())
                                .with_text_size(theme::TEXT_SIZE_SMALL)
                                .with_text_color(theme::RED)
                                .boxed(),
                        },
                    )
                    .lens(Preferences::auth)
                    .lens(AppState::preferences),
                ),
        );

    col = col.with_spacer(theme::grid(3.0));

    // Audio quality
    col = col
        .with_child(Label::new("Audio quality").with_font(theme::UI_FONT_MEDIUM))
        .with_spacer(theme::grid(2.0))
        .with_child(
            RadioGroup::new(vec![
                ("Low (96kbit)", AudioQuality::Low),
                ("Normal (160kbit)", AudioQuality::Normal),
                ("High (320kbit)", AudioQuality::High),
            ])
            .lens(Config::audio_quality)
            .lens(AppState::config),
        );

    col = col.with_spacer(theme::grid(3.0));

    // Save
    col = col.with_child(
        Button::new("Save")
            .on_click(move |ctx, config: &mut Config, _env| {
                config.save();
                ctx.submit_command(cmd::SESSION_CONNECT);
                ctx.submit_command(cmd::SHOW_MAIN);
                ctx.submit_command(commands::CLOSE_WINDOW);
            })
            .fix_width(theme::grid(10.0))
            .align_right()
            .lens(AppState::config),
    );

    col.controller(Authenticate::new())
}

struct Authenticate {
    thread: Option<JoinHandle<()>>,
}

impl Authenticate {
    fn new() -> Self {
        Self { thread: None }
    }
}

impl Authenticate {
    const REQUEST: Selector = Selector::new("app.preferences.authenticate-request");
    const RESPONSE: Selector<Result<Credentials, String>> =
        Selector::new("app.preferences.authenticate-response");
}

impl<W: Widget<AppState>> Controller<AppState, W> for Authenticate {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut AppState,
        env: &Env,
    ) {
        match event {
            Event::Command(cmd) if cmd.is(Self::REQUEST) => {
                let config = data.preferences.auth.session_config();
                let widget_id = ctx.widget_id();
                let event_sink = ctx.get_external_handle();
                let thread = thread::spawn(move || {
                    let response = Authentication::authenticate_and_get_credentials(config);
                    event_sink
                        .submit_command(Self::RESPONSE, response, widget_id)
                        .unwrap();
                });
                self.thread.replace(thread);
                ctx.set_handled();
            }
            Event::Command(cmd) if cmd.is(Self::RESPONSE) => {
                let result = cmd.get_unchecked(Self::RESPONSE);
                let result = result.to_owned().map(|credentials| {
                    data.config.store_credentials(credentials);
                });
                data.preferences.auth.result.resolve_or_reject(result);
                self.thread.take();
                ctx.set_handled();
            }
            _ => {
                child.event(ctx, event, data, env);
            }
        }
    }
}

fn cache_tab_widget() -> impl Widget<AppState> {
    let mut col = Flex::column().cross_axis_alignment(CrossAxisAlignment::Start);

    col = col
        .with_child(Label::new("Location").with_font(theme::UI_FONT_MEDIUM))
        .with_spacer(theme::grid(2.0))
        .with_child(
            Label::dynamic(|_, _| {
                Config::cache_dir()
                    .map(|path| path.to_string_lossy().to_string())
                    .unwrap_or_else(|| "None".to_string())
            })
            .with_line_break_mode(LineBreaking::WordWrap),
        );

    col = col.with_spacer(theme::grid(3.0));

    col = col
        .with_child(Label::new("Size").with_font(theme::UI_FONT_MEDIUM))
        .with_spacer(theme::grid(2.0))
        .with_child(Label::dynamic(
            |preferences: &Preferences, _| match preferences.cache_size {
                Promise::Empty | Promise::Rejected(_) => "Unknown".to_string(),
                Promise::Deferred(_) => "Computing".to_string(),
                Promise::Resolved(0) => "Empty".to_string(),
                Promise::Resolved(b) => {
                    format!("{:.2} MB", b as f64 / 1e6_f64)
                }
            },
        ));

    col.controller(MeasureCacheSize::new())
        .lens(AppState::preferences)
}

struct MeasureCacheSize {
    thread: Option<JoinHandle<()>>,
}

impl MeasureCacheSize {
    fn new() -> Self {
        Self { thread: None }
    }
}

impl MeasureCacheSize {
    const RESULT: Selector<Option<u64>> = Selector::new("app.preferences.measure-cache-size");
}

impl<W: Widget<Preferences>> Controller<Preferences, W> for MeasureCacheSize {
    fn event(
        &mut self,
        child: &mut W,
        ctx: &mut EventCtx,
        event: &Event,
        data: &mut Preferences,
        env: &Env,
    ) {
        match &event {
            Event::Command(cmd) if cmd.is(Self::RESULT) => {
                let result = cmd.get_unchecked(Self::RESULT).to_owned();
                data.cache_size.resolve_or_reject(result.ok_or(()));
                self.thread.take();
                ctx.set_handled();
            }
            _ => {
                child.event(ctx, event, data, env);
            }
        }
    }

    fn lifecycle(
        &mut self,
        child: &mut W,
        ctx: &mut LifeCycleCtx,
        event: &LifeCycle,
        data: &Preferences,
        env: &Env,
    ) {
        if let LifeCycle::WidgetAdded = &event {
            let handle = thread::spawn({
                let widget_id = ctx.widget_id();
                let event_sink = ctx.get_external_handle();
                move || {
                    let size = Preferences::measure_cache_usage();
                    event_sink
                        .submit_command(Self::RESULT, size, widget_id)
                        .unwrap();
                }
            });
            self.thread.replace(handle);
        }
        child.lifecycle(ctx, event, data, env);
    }
}
