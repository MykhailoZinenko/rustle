use eframe::egui::{Color32, Visuals};

pub const EDITOR_FONT_SIZE: f32 = 14.0;
pub const EDITOR_CONTENT_PADDING: i8 = 6;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeMode {
    Dark,
    Light,
}

impl ThemeMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
        }
    }
}

#[derive(Clone, Copy)]
pub struct ThemePalette {
    pub gutter_text: Color32,
    pub gutter_bg: Color32,
    pub editor_bg: Color32,
    pub console_bg: Color32,
    pub tab_active_bg: Color32,
    pub tab_inactive_bg: Color32,
    pub status_bg: Color32,
    pub surface_stroke: Color32,
    pub active_tab_stroke: Color32,
    pub syntax_identifier: Color32,
    pub syntax_keyword: Color32,
    pub syntax_type: Color32,
    pub syntax_literal: Color32,
    pub syntax_operator: Color32,
    pub syntax_punctuation: Color32,
    pub syntax_function: Color32,
    pub preview_bg: Color32,
    pub muted_text: Color32,
    pub error_text: Color32,
}

pub fn visuals_for(mode: ThemeMode) -> Visuals {
    match mode {
        ThemeMode::Dark => Visuals::dark(),
        ThemeMode::Light => Visuals::light(),
    }
}

pub fn palette_for(mode: ThemeMode) -> ThemePalette {
    match mode {
        ThemeMode::Dark => dark_palette(),
        ThemeMode::Light => light_palette(),
    }
}

fn dark_palette() -> ThemePalette {
    ThemePalette {
        gutter_text: Color32::from_gray(120),
        gutter_bg: Color32::from_rgb(28, 31, 38),
        editor_bg: Color32::from_rgb(24, 27, 34),
        console_bg: Color32::from_rgb(20, 22, 28),
        tab_active_bg: Color32::from_rgb(37, 41, 51),
        tab_inactive_bg: Color32::from_rgb(30, 33, 40),
        status_bg: Color32::from_rgb(18, 99, 160),
        surface_stroke: Color32::from_rgb(49, 54, 63),
        active_tab_stroke: Color32::from_rgb(78, 136, 224),
        syntax_identifier: Color32::from_rgb(212, 212, 212),
        syntax_keyword: Color32::from_rgb(197, 134, 192),
        syntax_type: Color32::from_rgb(78, 201, 176),
        syntax_literal: Color32::from_rgb(206, 145, 120),
        syntax_operator: Color32::from_rgb(212, 212, 212),
        syntax_punctuation: Color32::from_rgb(212, 212, 212),
        syntax_function: Color32::from_rgb(220, 220, 170),
        preview_bg: Color32::from_rgb(28, 28, 32),
        muted_text: Color32::GRAY,
        error_text: Color32::from_rgb(220, 80, 80),
    }
}

fn light_palette() -> ThemePalette {
    ThemePalette {
        gutter_text: Color32::from_rgb(110, 110, 118),
        gutter_bg: Color32::from_rgb(240, 240, 244),
        editor_bg: Color32::from_rgb(250, 250, 252),
        console_bg: Color32::from_rgb(245, 245, 248),
        tab_active_bg: Color32::from_rgb(255, 255, 255),
        tab_inactive_bg: Color32::from_rgb(233, 235, 239),
        status_bg: Color32::from_rgb(0, 120, 212),
        surface_stroke: Color32::from_rgb(200, 205, 214),
        active_tab_stroke: Color32::from_rgb(0, 120, 212),
        syntax_identifier: Color32::from_rgb(30, 30, 30),
        syntax_keyword: Color32::from_rgb(175, 0, 219),
        syntax_type: Color32::from_rgb(38, 127, 153),
        syntax_literal: Color32::from_rgb(163, 21, 21),
        syntax_operator: Color32::from_rgb(30, 30, 30),
        syntax_punctuation: Color32::from_rgb(30, 30, 30),
        syntax_function: Color32::from_rgb(121, 94, 38),
        preview_bg: Color32::from_rgb(255, 255, 255),
        muted_text: Color32::from_rgb(120, 120, 126),
        error_text: Color32::from_rgb(196, 43, 28),
    }
}
