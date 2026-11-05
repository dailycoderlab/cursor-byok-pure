//! Minimal TAB protobuf contracts from protocols/cursor/aiserver_v1.proto.
//! Unknown request fields (including credentials and unrelated context) are skipped.

#[derive(Clone, PartialEq, prost::Message)]
pub struct StreamCppRequest {
    #[prost(message, optional, tag = "1")]
    pub current_file: Option<CurrentFileInfo>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct CurrentFileInfo {
    #[prost(string, tag = "1")]
    pub relative_workspace_path: String,
    #[prost(string, tag = "2")]
    pub contents: String,
    #[prost(message, optional, tag = "3")]
    pub cursor_position: Option<CursorPosition>,
    #[prost(string, tag = "5")]
    pub language_id: String,
    #[prost(int32, tag = "9")]
    pub contents_start_at_line: i32,
    #[prost(bool, tag = "18")]
    pub rely_on_filesync: bool,
    #[prost(string, optional, tag = "20")]
    pub line_ending: Option<String>,
}

#[derive(Clone, Copy, PartialEq, prost::Message)]
pub struct CursorPosition {
    #[prost(int32, tag = "1")]
    pub line: i32,
    #[prost(int32, tag = "2")]
    pub column: i32,
}

#[derive(Clone, Copy, PartialEq, prost::Message)]
pub struct LineRange {
    #[prost(int32, tag = "1")]
    pub start_line_number: i32,
    #[prost(int32, tag = "2")]
    pub end_line_number_inclusive: i32,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct StreamCppResponse {
    #[prost(string, tag = "1")]
    pub text: String,
    #[prost(int32, optional, tag = "2")]
    pub suggestion_start_line: Option<i32>,
    #[prost(bool, optional, tag = "4")]
    pub done_stream: Option<bool>,
    #[prost(message, optional, tag = "11")]
    pub range_to_replace: Option<LineRange>,
    #[prost(bool, optional, tag = "13")]
    pub done_edit: Option<bool>,
    #[prost(bool, optional, tag = "15")]
    pub begin_edit: Option<bool>,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct CppConfigResponse {
    #[prost(int32, optional, tag = "1")]
    pub above_radius: Option<i32>,
    #[prost(int32, optional, tag = "2")]
    pub below_radius: Option<i32>,
    #[prost(bool, optional, tag = "5")]
    pub is_on: Option<bool>,
    #[prost(bool, optional, tag = "6")]
    pub is_ghost_text: Option<bool>,
    #[prost(bool, optional, tag = "7")]
    pub should_let_user_enable_cpp_even_if_not_pro: Option<bool>,
    #[prost(bool, tag = "10")]
    pub enable_rvf_tracking: bool,
    #[prost(int32, tag = "11")]
    pub global_debounce_duration_millis: i32,
    #[prost(int32, tag = "12")]
    pub client_debounce_duration_millis: i32,
    #[prost(string, tag = "13")]
    pub cpp_url: String,
    #[prost(message, optional, tag = "15")]
    pub import_prediction_config: Option<ImportPredictionConfig>,
    #[prost(bool, tag = "16")]
    pub enable_filesync_debounce_skipping: bool,
    #[prost(float, tag = "17")]
    pub check_filesync_hash_percent: f32,
    #[prost(string, tag = "18")]
    pub geo_cpp_backend_url: String,
    #[prost(bool, tag = "20")]
    pub is_fused_cursor_prediction_model: bool,
    #[prost(bool, tag = "21")]
    pub include_unchanged_lines: bool,
    #[prost(bool, tag = "22")]
    pub should_fetch_rvf_text: bool,
    #[prost(bool, tag = "25")]
    pub allows_tab_chunks: bool,
}

#[derive(Clone, Copy, PartialEq, prost::Message)]
pub struct ImportPredictionConfig {
    #[prost(bool, tag = "1")]
    pub is_disabled_by_backend: bool,
    #[prost(bool, tag = "2")]
    pub should_turn_on_automatically: bool,
    #[prost(bool, tag = "3")]
    pub python_enabled: bool,
}

#[derive(Clone, PartialEq, prost::Message)]
pub struct AvailableCppModelsResponse {
    #[prost(string, repeated, tag = "1")]
    pub models: Vec<String>,
    #[prost(string, optional, tag = "2")]
    pub default_model: Option<String>,
}

#[derive(Clone, Copy, PartialEq, prost::Message)]
pub struct CppEditHistoryStatusResponse {
    #[prost(bool, tag = "1")]
    pub on: bool,
    #[prost(bool, tag = "2")]
    pub only_if_explicit: bool,
}

#[derive(Clone, Copy, PartialEq, prost::Message)]
pub struct CppAppendResponse {
    #[prost(bool, tag = "1")]
    pub success: bool,
}

#[derive(Clone, Copy, PartialEq, prost::Message)]
pub struct GetCppEditClassificationResponse {
    #[prost(bool, optional, tag = "3")]
    pub should_noop: Option<bool>,
}

#[derive(Clone, Copy, PartialEq, prost::Message)]
pub struct FsIsEnabledForUserResponse {
    #[prost(bool, tag = "1")]
    pub enabled: bool,
}

// Upload and sync use distinct enums with the same NON_EXISTANT = 1 wire value.
#[derive(Clone, Copy, PartialEq, prost::Message)]
pub struct FileSyncResponse {
    #[prost(int32, tag = "1")]
    pub error: i32,
}

#[derive(Clone, Copy, PartialEq, prost::Message)]
pub struct EmptyResponse {}
