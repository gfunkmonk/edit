// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

use std::borrow::Cow;
use std::ffi::{OsStr, OsString};
use std::mem;
use std::path::{Path, PathBuf};

use edit::framebuffer::IndexedColor;
use edit::helpers::*;
use edit::oklab::StraightRgba;
use edit::tui::*;
use edit::{apperr, buffer, icu, sys};

use crate::documents::DocumentManager;
use crate::localization::*;

#[repr(transparent)]
pub struct FormatApperr(apperr::Error);

impl From<apperr::Error> for FormatApperr {
    fn from(err: apperr::Error) -> Self {
        Self(err)
    }
}

impl std::fmt::Display for FormatApperr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.0 {
            apperr::APP_ICU_MISSING => f.write_str(loc(LocId::ErrorIcuMissing)),
            apperr::Error::App(code) => write!(f, "Unknown app error code: {code}"),
            apperr::Error::Icu(code) => icu::apperr_format(f, code),
            apperr::Error::Sys(code) => sys::apperr_format(f, code),
        }
    }
}

pub struct DisplayablePathBuf {
    value: PathBuf,
    str: Cow<'static, str>,
}

impl DisplayablePathBuf {
    #[allow(dead_code, reason = "only used on Windows")]
    pub fn from_string(string: String) -> Self {
        let str = Cow::Borrowed(string.as_str());
        let str = unsafe { mem::transmute::<Cow<'_, str>, Cow<'_, str>>(str) };
        let value = PathBuf::from(string);
        Self { value, str }
    }

    pub fn from_path(value: PathBuf) -> Self {
        let str = value.to_string_lossy();
        let str = unsafe { mem::transmute::<Cow<'_, str>, Cow<'_, str>>(str) };
        Self { value, str }
    }

    pub fn as_path(&self) -> &Path {
        &self.value
    }

    pub fn as_str(&self) -> &str {
        &self.str
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.value.as_os_str().as_encoded_bytes()
    }
}

impl Default for DisplayablePathBuf {
    fn default() -> Self {
        Self { value: Default::default(), str: Cow::Borrowed("") }
    }
}

impl Clone for DisplayablePathBuf {
    fn clone(&self) -> Self {
        Self::from_path(self.value.clone())
    }
}

impl From<OsString> for DisplayablePathBuf {
    fn from(s: OsString) -> Self {
        Self::from_path(PathBuf::from(s))
    }
}

impl<T: ?Sized + AsRef<OsStr>> From<&T> for DisplayablePathBuf {
    fn from(s: &T) -> Self {
        Self::from_path(PathBuf::from(s))
    }
}

pub struct StateSearch {
    pub kind: StateSearchKind,
    pub focus: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StateSearchKind {
    Hidden,
    Disabled,
    Search,
    Replace,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StateFilePicker {
    None,
    Open,
    SaveAs,

    SaveAsShown, // Transitioned from SaveAs
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum StateEncodingChange {
    None,
    Convert,
    Reopen,
}

#[derive(Default)]
pub struct OscTitleFileStatus {
    pub filename: String,
    pub dirty: bool,
}

pub struct State {
    pub menubar_color_bg: StraightRgba,
    pub menubar_color_fg: StraightRgba,
    pub menubar_color_choice: IndexedColor,

    pub highlight_color_rgba: StraightRgba,
    pub highlight_color_choice: IndexedColor,

    pub documents: DocumentManager,

    // A ring buffer of the last 10 errors.
    pub error_log: [String; 10],
    pub error_log_index: usize,
    pub error_log_count: usize,

    pub wants_file_picker: StateFilePicker,
    pub file_picker_pending_dir: DisplayablePathBuf,
    pub file_picker_pending_dir_revision: u64, // Bumped every time `file_picker_pending_dir` changes.
    pub file_picker_pending_name: PathBuf,
    pub file_picker_entries: Option<[Vec<DisplayablePathBuf>; 3]>, // ["..", directories, files]
    pub file_picker_overwrite_warning: Option<PathBuf>,            // The path the warning is about.
    pub file_picker_autocomplete: Vec<DisplayablePathBuf>,

    pub wants_search: StateSearch,
    pub search_needle: String,
    pub search_replacement: String,
    pub search_options: buffer::SearchOptions,
    pub search_success: bool,

    pub wants_encoding_picker: bool,
    pub wants_encoding_change: StateEncodingChange,
    pub encoding_picker_needle: String,
    pub encoding_picker_results: Option<Vec<icu::Encoding>>,

    pub wants_save: bool,
    pub wants_statusbar_focus: bool,
    pub wants_indentation_picker: bool,
    pub wants_menubar_color_picker: bool,
    pub wants_highlight_color_picker: bool,
    pub wants_go_to_file: bool,
    pub wants_about: bool,
    pub wants_close: bool,
    pub wants_exit: bool,
    pub wants_goto: bool,
    pub goto_target: String,
    pub goto_invalid: bool,

    pub osc_title_file_status: OscTitleFileStatus,
    pub osc_clipboard_sync: bool,
    pub osc_clipboard_always_send: bool,
    pub exit: bool,
}

impl State {
    pub fn new() -> apperr::Result<Self> {
        let saved_color = load_menubar_color();
        let saved_highlight_color = load_highlight_color();
        
        Ok(Self {
            menubar_color_bg: StraightRgba::zero(),
            menubar_color_fg: StraightRgba::zero(),
            menubar_color_choice: saved_color,

            highlight_color_rgba: StraightRgba::zero(),
            highlight_color_choice: saved_highlight_color,

            documents: Default::default(),

            error_log: [const { String::new() }; 10],
            error_log_index: 0,
            error_log_count: 0,

            wants_file_picker: StateFilePicker::None,
            file_picker_pending_dir: Default::default(),
            file_picker_pending_dir_revision: 0,
            file_picker_pending_name: Default::default(),
            file_picker_entries: None,
            file_picker_overwrite_warning: None,
            file_picker_autocomplete: Vec::new(),

            wants_search: StateSearch { kind: StateSearchKind::Hidden, focus: false },
            search_needle: Default::default(),
            search_replacement: Default::default(),
            search_options: Default::default(),
            search_success: true,

            wants_encoding_picker: false,
            encoding_picker_needle: Default::default(),
            encoding_picker_results: Default::default(),

            wants_save: false,
            wants_statusbar_focus: false,
            wants_encoding_change: StateEncodingChange::None,
            wants_indentation_picker: false,
            wants_menubar_color_picker: false,
            wants_highlight_color_picker: false,
            wants_go_to_file: false,
            wants_about: false,
            wants_close: false,
            wants_exit: false,
            wants_goto: false,
            goto_target: Default::default(),
            goto_invalid: false,

            osc_title_file_status: Default::default(),
            osc_clipboard_sync: false,
            osc_clipboard_always_send: false,
            exit: false,
        })
    }
}

pub fn draw_add_untitled_document(ctx: &mut Context, state: &mut State) {
    if let Err(err) = state.documents.add_untitled() {
        error_log_add(ctx, state, err);
    } else {
        // Set highlight color on the newly created document
        if let Some(doc) = state.documents.active() {
            let mut tb = doc.buffer.borrow_mut();
            tb.set_line_highlight_color(state.highlight_color_rgba);
        }
    }
}

pub fn error_log_add(ctx: &mut Context, state: &mut State, err: apperr::Error) {
    let msg = format!("{}", FormatApperr::from(err));
    if !msg.is_empty() {
        state.error_log[state.error_log_index] = msg;
        state.error_log_index = (state.error_log_index + 1) % state.error_log.len();
        state.error_log_count = state.error_log.len().min(state.error_log_count + 1);
        ctx.needs_rerender();
    }
}

pub fn draw_error_log(ctx: &mut Context, state: &mut State) {
    ctx.modal_begin("error", loc(LocId::ErrorDialogTitle));
    ctx.attr_background_rgba(ctx.indexed(IndexedColor::Red));
    ctx.attr_foreground_rgba(ctx.indexed(IndexedColor::BrightWhite));
    {
        ctx.block_begin("content");
        ctx.attr_padding(Rect::three(0, 2, 1));
        {
            let off = state.error_log_index + state.error_log.len() - state.error_log_count;

            for i in 0..state.error_log_count {
                let idx = (off + i) % state.error_log.len();
                let msg = &state.error_log[idx][..];

                if !msg.is_empty() {
                    ctx.next_block_id_mixin(i as u64);
                    ctx.label("error", msg);
                    ctx.attr_overflow(Overflow::TruncateTail);
                }
            }
        }
        ctx.block_end();

        if ctx.button("ok", loc(LocId::Ok), ButtonStyle::default()) {
            state.error_log_count = 0;
        }
        ctx.attr_position(Position::Center);
        ctx.inherit_focus();
    }
    if ctx.modal_end() {
        state.error_log_count = 0;
    }
}

fn get_config_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA")
            .map(|appdata| PathBuf::from(appdata).join("edit").join("config.txt"))
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Some(xdg_config) = std::env::var_os("XDG_CONFIG_HOME") {
            Some(PathBuf::from(xdg_config).join("edit").join("config.txt"))
        } else {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".config").join("edit").join("config.txt"))
        }
    }
}

pub fn load_menubar_color() -> IndexedColor {
    if let Some(config_path) = get_config_path() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            for line in content.lines() {
                if let Some(value) = line.strip_prefix("menubar_color=") {
                    return match value.trim() {
                        "BrightRed" => IndexedColor::BrightRed,
                        "BrightGreen" => IndexedColor::BrightGreen,
                        "BrightYellow" => IndexedColor::BrightYellow,
                        "BrightMagenta" => IndexedColor::BrightMagenta,
                        "BrightCyan" => IndexedColor::BrightCyan,
                        _ => IndexedColor::BrightBlue,
                    };
                }
            }
        }
    }
    IndexedColor::BrightBlue // Default
}

pub fn load_highlight_color() -> IndexedColor {
    if let Some(config_path) = get_config_path() {
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            for line in content.lines() {
                if let Some(value) = line.strip_prefix("highlight_color=") {
                    return match value.trim() {
                        "BrightRed" => IndexedColor::BrightRed,
                        "BrightGreen" => IndexedColor::BrightGreen,
                        "BrightYellow" => IndexedColor::BrightYellow,
                        "BrightMagenta" => IndexedColor::BrightMagenta,
                        "BrightCyan" => IndexedColor::BrightCyan,
                        _ => IndexedColor::BrightBlue,
                    };
                }
            }
        }
    }
    IndexedColor::BrightBlue // Default
}

pub fn save_menubar_color(color: IndexedColor) {
    save_config("menubar_color", color);
}

pub fn save_highlight_color(color: IndexedColor) {
    save_config("highlight_color", color);
}

fn save_config(key: &str, color: IndexedColor) {
    if let Some(config_path) = get_config_path() {
        // Try to create parent directory if it doesn't exist
        if let Some(parent) = config_path.parent() {
            if !parent.exists() {
                if let Err(_) = std::fs::create_dir_all(parent) {
                    // Failed to create config directory, cannot save
                    return;
                }
            }
        }
        
        let color_name = match color {
            IndexedColor::BrightBlue => "BrightBlue",
            IndexedColor::BrightRed => "BrightRed",
            IndexedColor::BrightGreen => "BrightGreen",
            IndexedColor::BrightYellow => "BrightYellow",
            IndexedColor::BrightMagenta => "BrightMagenta",
            IndexedColor::BrightCyan => "BrightCyan",
            _ => "BrightBlue",
        };
        
        // Read existing config
        let mut config_lines = std::collections::HashMap::new();
        if let Ok(content) = std::fs::read_to_string(&config_path) {
            for line in content.lines() {
                if let Some((k, v)) = line.split_once('=') {
                    config_lines.insert(k.to_string(), v.to_string());
                }
            }
        }
        
        // Update the specific key
        config_lines.insert(key.to_string(), color_name.to_string());
        
        // Write back
        let mut config_content = String::new();
        for (k, v) in config_lines {
            config_content.push_str(&format!("{}={}\n", k, v));
        }
        
        // Attempt to write config; if it fails, the color just won't persist
        let _ = std::fs::write(&config_path, config_content);
    }
}
