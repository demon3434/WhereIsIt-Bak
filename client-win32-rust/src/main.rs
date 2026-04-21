
#![windows_subsystem = "windows"]

use reqwest::blocking::{multipart, Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;
use sha2::{Digest, Sha256};
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BACKGROUND_MODE, GetStockObject, HBRUSH, HDC, DEFAULT_GUI_FONT, WHITE_BRUSH, SetBkColor, SetBkMode, SetTextColor,
};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Controls::{
    InitCommonControlsEx, ICC_PROGRESS_CLASS, ICC_TAB_CLASSES, INITCOMMONCONTROLSEX, NMHDR,
    PBM_SETPOS, TCN_SELCHANGE, TCIF_TEXT, TCITEMW, TCM_GETCURSEL, TCM_INSERTITEMW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetDlgItem,
    GetMessageW, GetSystemMetrics, GetWindowTextLengthW, GetWindowTextW, LoadCursorW,
    MessageBoxW, PostQuitMessage, RegisterClassW, SendMessageW, SetForegroundWindow, SetTimer,
    SetWindowTextW, ShowWindow,
    TranslateMessage, BS_GROUPBOX, BS_PUSHBUTTON, CB_ADDSTRING, CB_GETCURSEL, CB_GETLBTEXT, CB_GETLBTEXTLEN,
    CB_SETCURSEL, CBS_DROPDOWNLIST, CS_HREDRAW, CS_VREDRAW, ES_AUTOHSCROLL,
    ES_AUTOVSCROLL, ES_LEFT, ES_MULTILINE, ES_PASSWORD, ES_READONLY, HMENU, IDC_ARROW, MB_ICONERROR,
    MB_ICONWARNING, MB_OK, MB_YESNO, MSG, SM_CXSCREEN, SM_CYSCREEN, SW_HIDE, SW_RESTORE, SW_SHOW,
    WINDOW_EX_STYLE, WINDOW_STYLE, WM_CLOSE, WM_COMMAND, WM_CREATE, WM_DESTROY, WM_NOTIFY,
    WM_SETFONT, WM_TIMER, WM_CTLCOLORSTATIC, WNDCLASSW, WS_BORDER, WS_CAPTION, WS_CHILD, WS_CLIPCHILDREN,
    WS_OVERLAPPED, WS_OVERLAPPEDWINDOW, WS_SYSMENU, WS_TABSTOP, WS_VISIBLE, WS_VSCROLL,
};

const LOGIN_CLASS: PCWSTR = w!("WhereIsItRustLogin");
const MAIN_CLASS: PCWSTR = w!("WhereIsItRustMain");
const TIMER_ID: usize = 1;

const IDC_LOGIN_SERVER: i32 = 1001;
const IDC_LOGIN_USER: i32 = 1002;
const IDC_LOGIN_PASS: i32 = 1003;
const IDC_LOGIN_TEST: i32 = 1004;
const IDC_LOGIN_GO: i32 = 1005;
const IDC_LOGIN_STATUS: i32 = 1006;
const IDC_LOGIN_ERROR: i32 = 1007;

const IDC_MAIN_STATUS: i32 = 2001;
const IDC_MAIN_TAB: i32 = 2002;

const IDC_DB_BACKUP_TITLE: i32 = 2109;
const IDC_DB_BACKUP_DIR: i32 = 2102;
const IDC_DB_BACKUP_BROWSE: i32 = 2103;
const IDC_DB_BACKUP_START: i32 = 2104;
const IDC_DB_BACKUP_PROGRESS: i32 = 2106;
const IDC_DB_BACKUP_STATUS: i32 = 2107;
const IDC_DB_BACKUP_LOG: i32 = 2108;
const IDC_UP_BACKUP_TITLE: i32 = 2111;
const IDC_UP_BACKUP_DIR: i32 = 2112;
const IDC_UP_BACKUP_BROWSE: i32 = 2113;
const IDC_UP_BACKUP_START: i32 = 2115;
const IDC_UP_BACKUP_PROGRESS: i32 = 2116;
const IDC_UP_BACKUP_STATUS: i32 = 2117;
const IDC_UP_BACKUP_LOG: i32 = 2118;

const IDC_DB_RESTORE_FILE: i32 = 2301;
const IDC_DB_RESTORE_TITLE: i32 = 2308;
const IDC_DB_RESTORE_MODE_LABEL: i32 = 2309;
const IDC_DB_RESTORE_BROWSE: i32 = 2302;
const IDC_DB_RESTORE_MODE: i32 = 2303;
const IDC_DB_RESTORE_START: i32 = 2304;
const IDC_DB_RESTORE_PROGRESS: i32 = 2305;
const IDC_DB_RESTORE_STATUS: i32 = 2306;
const IDC_DB_RESTORE_LOG: i32 = 2307;
const IDC_UP_RESTORE_TITLE: i32 = 2310;
const IDC_UP_RESTORE_DIR: i32 = 2311;
const IDC_UP_RESTORE_BROWSE: i32 = 2312;
const IDC_UP_RESTORE_MODE_LABEL: i32 = 2313;
const IDC_UP_RESTORE_MODE: i32 = 2314;
const IDC_UP_RESTORE_SCAN: i32 = 2315;
const IDC_UP_RESTORE_CREATE: i32 = 2316;
const IDC_UP_RESTORE_START: i32 = 2317;
const IDC_UP_RESTORE_PROGRESS: i32 = 2318;
const IDC_UP_RESTORE_STATUS: i32 = 2319;
const IDC_UP_RESTORE_LOG: i32 = 2320;

static APP: OnceLock<Arc<Mutex<AppModel>>> = OnceLock::new();
static MAIN_HWND: AtomicIsize = AtomicIsize::new(0);
static SWITCHING: AtomicBool = AtomicBool::new(false);
static UI_CACHE: OnceLock<Mutex<UiRenderCache>> = OnceLock::new();
static BACKUP_CTRL_HWNDS: OnceLock<Mutex<Vec<isize>>> = OnceLock::new();
static RESTORE_CTRL_HWNDS: OnceLock<Mutex<Vec<isize>>> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppConfig {
    server_base_url: String,
    admin_username: String,
    token: String,
    timeout_seconds: u64,
    default_backup_root: String,
    #[serde(default)]
    modules: AppModules,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AppModules {
    #[serde(default)]
    database: AppDatabaseModule,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AppDatabaseModule {
    db_name: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server_base_url: "http://127.0.0.1:3000".into(),
            admin_username: "admin".into(),
            token: String::new(),
            timeout_seconds: 60,
            default_backup_root: "D:\\system-backups".into(),
            modules: AppModules::default(),
        }
    }
}

impl AppConfig {
    fn effective_db_name(&self) -> String {
        self.modules
            .database
            .db_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("whereisit")
            .to_string()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiEnvelope<T> {
    message: Option<String>,
    data: Option<T>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    token: Option<String>,
    access_token: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthMeResponse {
    username: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TaskProgress {
    percent: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TaskItem {
    status: Option<String>,
    progress: Option<TaskProgress>,
    message: Option<String>,
    error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct DbPreflightResponse {
    server_version: Option<String>,
    resolved_major: Option<i64>,
    selected_strategy: Option<String>,
    selected_tools_image: Option<String>,
    warnings: Option<Vec<String>>,
    can_proceed: Option<bool>,
    blocking_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct DbBackupMetadata {
    file_name: Option<String>,
}

#[derive(Default, Debug)]
struct LoginState {
    connection_status: String,
    error_message: String,
    login_success: bool,
}

#[derive(Default, Debug)]
struct DbBackupState {
    backup_format: String,
    dir: String,
    task_id: String,
    status: String,
    progress: f64,
    log: String,
    legacy: bool,
}

#[derive(Default, Debug)]
struct DbRestoreState {
    file: String,
    mode: String,
    task_id: String,
    status: String,
    progress: f64,
    log: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct UploadManifest {
    scope: Option<String>,
    file_count: Option<u64>,
    total_bytes: Option<u64>,
    files: Option<Vec<UploadFileItem>>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct UploadFileItem {
    file_id: Option<String>,
    relative_path: Option<String>,
    download_url: Option<String>,
}

#[derive(Default, Debug)]
struct UploadBackupState {
    dir: String,
    status: String,
    progress: f64,
    log: String,
}

#[derive(Default, Debug)]
struct UploadRestoreState {
    dir: String,
    overwrite_mode: String,
    task_id: String,
    scanned_file_count: usize,
    scanned_total_bytes: u64,
    scanned_files: Vec<String>,
    completed_count: usize,
    skipped_count: usize,
    overwritten_count: usize,
    failed_count: usize,
    current_file: String,
    status: String,
    progress: f64,
    log: String,
}

#[derive(Debug)]
struct AppModel {
    config_path: PathBuf,
    config: AppConfig,
    status: String,
    login: LoginState,
    backup: DbBackupState,
    restore: DbRestoreState,
    uploads_backup: UploadBackupState,
    uploads_restore: UploadRestoreState,
}

#[derive(Default, Debug)]
struct UiRenderCache {
    main_status: String,
    backup_dir: String,
    backup_status: String,
    backup_log: String,
    restore_file: String,
    restore_status: String,
    restore_log: String,
    uploads_backup_dir: String,
    uploads_backup_status: String,
    uploads_backup_log: String,
    uploads_restore_dir: String,
    uploads_restore_status: String,
    uploads_restore_log: String,
    login_status: String,
    login_error: String,
}

fn model() -> Arc<Mutex<AppModel>> {
    APP.get().unwrap().clone()
}

fn ui_cache() -> &'static Mutex<UiRenderCache> {
    UI_CACHE.get_or_init(|| Mutex::new(UiRenderCache::default()))
}

fn backup_ctrl_hwnds() -> &'static Mutex<Vec<isize>> {
    BACKUP_CTRL_HWNDS.get_or_init(|| Mutex::new(Vec::new()))
}

fn restore_ctrl_hwnds() -> &'static Mutex<Vec<isize>> {
    RESTORE_CTRL_HWNDS.get_or_init(|| Mutex::new(Vec::new()))
}

fn track_backup(hwnd: HWND) {
    if hwnd.0 != 0 {
        backup_ctrl_hwnds().lock().unwrap().push(hwnd.0);
    }
}

fn track_restore(hwnd: HWND) {
    if hwnd.0 != 0 {
        restore_ctrl_hwnds().lock().unwrap().push(hwnd.0);
    }
}

fn append_log(log: &mut String, msg: &str) {
    log.push_str(msg);
    log.push_str("\r\n");
}

fn base_url(c: &AppConfig) -> String {
    format!("{}/", c.server_base_url.trim_end_matches('/'))
}

fn build_client(c: &AppConfig, with_token: bool) -> Result<Client, String> {
    let mut headers = reqwest::header::HeaderMap::new();
    if with_token && !c.token.trim().is_empty() {
        headers.insert(
            reqwest::header::AUTHORIZATION,
            reqwest::header::HeaderValue::from_str(&format!("Bearer {}", c.token))
                .map_err(|e| e.to_string())?,
        );
    }

    Client::builder()
        .timeout(Duration::from_secs(c.timeout_seconds))
        .default_headers(headers)
        .build()
        .map_err(|e| e.to_string())
}

fn parse_data<T: for<'de> Deserialize<'de>>(text: &str) -> Option<T> {
    if let Ok(env) = serde_json::from_str::<ApiEnvelope<T>>(text) {
        if let Some(data) = env.data {
            return Some(data);
        }
    }
    serde_json::from_str::<T>(text).ok()
}

fn parse_envelope_message(text: &str) -> Option<String> {
    serde_json::from_str::<ApiEnvelope<Value>>(text)
        .ok()?
        .message
}

fn http_error_with_body(status: reqwest::StatusCode, body: String) -> String {
    let body_short: String = body.chars().take(300).collect();
    if let Some(message) = parse_envelope_message(&body) {
        if !message.trim().is_empty() {
            return format!("HTTP {} message={} body={}", status, message, body_short);
        }
    }
    format!("HTTP {} body={}", status, body_short)
}

fn api_test(c: &AppConfig) -> Result<bool, String> {
    let cli = build_client(c, true)?;
    if let Ok(r) = cli.get(format!("{}api/health", base_url(c))).send() {
        if r.status().is_success() {
            return Ok(true);
        }
    }
    let r = cli
        .get(format!("{}health", base_url(c)))
        .send()
        .map_err(|e| e.to_string())?;
    Ok(r.status().is_success())
}

fn api_login(c: &AppConfig, username: &str, password: &str) -> Result<String, String> {
    let r = build_client(c, false)?
        .post(format!("{}api/auth/login", base_url(c)))
        .json(&serde_json::json!({"username": username, "password": password}))
        .send()
        .map_err(|e| e.to_string())?;

    if !r.status().is_success() {
        return Err(format!("HTTP {}", r.status()));
    }

    let body = r.text().map_err(|e| e.to_string())?;
    if let Some(parsed) = parse_data::<LoginResponse>(&body) {
        if let Some(token) = parsed.access_token.or(parsed.token) {
            if !token.trim().is_empty() {
                return Ok(token);
            }
        }
    }

    if let Ok(value) = serde_json::from_str::<Value>(&body) {
        let candidates = [
            value.get("accessToken"),
            value.get("access_token"),
            value.get("token"),
            value.get("jwt"),
            value.get("jwtToken"),
            value.get("data").and_then(|x| x.get("accessToken")),
            value.get("data").and_then(|x| x.get("access_token")),
            value.get("data").and_then(|x| x.get("token")),
            value.get("data").and_then(|x| x.get("jwt")),
            value.get("data").and_then(|x| x.get("jwtToken")),
            value.get("result").and_then(|x| x.get("accessToken")),
            value.get("result").and_then(|x| x.get("access_token")),
            value.get("result").and_then(|x| x.get("token")),
            value.get("result").and_then(|x| x.get("jwt")),
            value.get("result").and_then(|x| x.get("jwtToken")),
        ];
        for candidate in candidates {
            if let Some(token) = candidate.and_then(|x| x.as_str()) {
                if !token.trim().is_empty() {
                    return Ok(token.to_string());
                }
            }
        }
    }

    Err("missing token".into())
}

fn api_me(c: &AppConfig) -> Option<String> {
    let r = build_client(c, true)
        .ok()?
        .get(format!("{}api/auth/me", base_url(c)))
        .send()
        .ok()?;
    if !r.status().is_success() {
        return None;
    }
    parse_data::<AuthMeResponse>(&r.text().ok()?)?.username
}
fn api_task(c: &AppConfig, task_id: &str) -> Result<TaskItem, String> {
    let r = build_client(c, true)?
        .get(format!("{}api/tasks/{}", base_url(c), task_id))
        .send()
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("HTTP {}", r.status()));
    }
    parse_data(&r.text().map_err(|e| e.to_string())?).ok_or("parse task failed".into())
}

fn api_backup_preflight(c: &AppConfig) -> Result<DbPreflightResponse, String> {
    let db_name = c.effective_db_name();
    let r = build_client(c, true)?
        .post(format!("{}api/backup/database/preflight", base_url(c)))
        .json(&serde_json::json!({ "dbName": db_name }))
        .send()
        .map_err(|e| e.to_string())?;

    if !r.status().is_success() {
        let status = r.status();
        let body = r.text().unwrap_or_default();
        return Err(http_error_with_body(status, body));
    }

    parse_data(&r.text().map_err(|e| e.to_string())?).ok_or("parse backup preflight failed".into())
}

fn api_backup_create(c: &AppConfig, fmt: &str) -> Result<String, String> {
    let db_name = c.effective_db_name();
    let r = build_client(c, true)?
        .post(format!("{}api/backup/database", base_url(c)))
        .json(&serde_json::json!({"dbName":db_name,"format":fmt}))
        .send()
        .map_err(|e| e.to_string())?;

    if !r.status().is_success() {
        let status = r.status();
        let body = r.text().unwrap_or_default();
        return Err(http_error_with_body(status, body));
    }

    let env: ApiEnvelope<Value> =
        serde_json::from_str(&r.text().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

    env.data
        .and_then(|x| x.get("taskId").and_then(|v| v.as_str()).map(str::to_string))
        .ok_or("missing taskId".into())
}

fn api_backup_download(c: &AppConfig, task_id: &str, output: &Path) -> Result<(), String> {
    let mut r = build_client(c, true)?
        .get(format!("{}api/backup/database/{}/download", base_url(c), task_id))
        .send()
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        let status = r.status();
        let body = r.text().unwrap_or_default();
        return Err(http_error_with_body(status, body));
    }
    let mut f = File::create(output).map_err(|e| e.to_string())?;
    r.copy_to(&mut f).map_err(|e| e.to_string())?;
    Ok(())
}

fn api_backup_metadata(c: &AppConfig, task_id: &str) -> Result<DbBackupMetadata, String> {
    let r = build_client(c, true)?
        .get(format!("{}api/backup/database/{}/metadata", base_url(c), task_id))
        .send()
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        let status = r.status();
        let body = r.text().unwrap_or_default();
        return Err(http_error_with_body(status, body));
    }
    parse_data(&r.text().map_err(|e| e.to_string())?).ok_or("parse backup metadata failed".into())
}

fn api_backup_legacy(c: &AppConfig, output: &Path) -> Result<(), String> {
    let mut r = build_client(c, true)?
        .get(format!("{}api/admin/data/export/db", base_url(c)))
        .send()
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("HTTP {}", r.status()));
    }
    let mut f = File::create(output).map_err(|e| e.to_string())?;
    r.copy_to(&mut f).map_err(|e| e.to_string())?;
    Ok(())
}

fn api_restore_upload(c: &AppConfig, file: &Path) -> Result<String, String> {
    let form = multipart::Form::new().file("file", file).map_err(|e| e.to_string())?;
    let r = build_client(c, true)?
        .post(format!("{}api/restore/database/upload", base_url(c)))
        .multipart(form)
        .send()
        .map_err(|e| e.to_string())?;

    if !r.status().is_success() {
        let status = r.status();
        let body = r.text().unwrap_or_default();
        return Err(http_error_with_body(status, body));
    }

    let env: ApiEnvelope<Value> =
        serde_json::from_str(&r.text().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

    env.data
        .and_then(|x| x.get("uploadFileId").and_then(|v| v.as_str()).map(str::to_string))
        .ok_or("missing uploadFileId".into())
}

fn api_restore_preflight(c: &AppConfig) -> Result<DbPreflightResponse, String> {
    let db_name = c.effective_db_name();
    let r = build_client(c, true)?
        .post(format!("{}api/restore/database/preflight", base_url(c)))
        .json(&serde_json::json!({ "targetDbName": db_name }))
        .send()
        .map_err(|e| e.to_string())?;

    if !r.status().is_success() {
        let status = r.status();
        let body = r.text().unwrap_or_default();
        return Err(http_error_with_body(status, body));
    }

    parse_data(&r.text().map_err(|e| e.to_string())?).ok_or("parse restore preflight failed".into())
}

fn api_restore_create(c: &AppConfig, upload_id: &str, mode: &str, confirm_text: &str) -> Result<String, String> {
    let db_name = c.effective_db_name();
    let r = build_client(c, true)?
        .post(format!("{}api/restore/database", base_url(c)))
        .json(&serde_json::json!({
            "uploadFileId": upload_id,
            "targetDbName": db_name,
            "restoreMode": mode,
            "confirmText": confirm_text
        }))
        .send()
        .map_err(|e| e.to_string())?;

    if !r.status().is_success() {
        let status = r.status();
        let body = r.text().unwrap_or_default();
        return Err(http_error_with_body(status, body));
    }

    let env: ApiEnvelope<Value> =
        serde_json::from_str(&r.text().map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;

    env.data
        .and_then(|x| x.get("taskId").and_then(|v| v.as_str()).map(str::to_string))
        .ok_or("missing taskId".into())
}

fn api_restore_legacy(c: &AppConfig, file: &Path, mode: &str) -> Result<String, String> {
    let url = format!("{}api/admin/data/import/db", base_url(c));
    let form = multipart::Form::new()
        .text("restore_mode", mode.to_string())
        .text("restoreMode", mode.to_string())
        .text("mode", mode.to_string())
        .file("file", file)
        .map_err(|e| e.to_string())?;

    let r = build_client(c, true)?
        .post(url.clone())
        .multipart(form)
        .send()
        .map_err(|e| e.to_string())?;

    if r.status().is_success() {
        return Ok(format!("legacy restore endpoint succeeded: {}", url));
    }
    let status = r.status();
    let body = r.text().unwrap_or_default();
    let body_short: String = body.chars().take(300).collect();
    Err(format!("{} -> HTTP {} body={}", url, status, body_short))
}

fn api_uploads_manifest(c: &AppConfig) -> Result<UploadManifest, String> {
    let r = build_client(c, true)?
        .post(format!("{}api/backup/uploads/create-manifest", base_url(c)))
        .json(&serde_json::json!({
            "scope": "uploads",
            "incremental": false,
            "modifiedAfter": null
        }))
        .send()
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        let status = r.status();
        let body = r.text().unwrap_or_default();
        let body_short: String = body.chars().take(300).collect();
        return Err(format!("HTTP {} body={}", status, body_short));
    }
    parse_data::<UploadManifest>(&r.text().map_err(|e| e.to_string())?).ok_or("parse uploads manifest failed".into())
}

fn api_uploads_download_file(c: &AppConfig, file: &UploadFileItem, local_root: &Path) -> Result<(), String> {
    let rel = file
        .relative_path
        .as_deref()
        .ok_or("missing relativePath")?;
    let file_id = file.file_id.as_deref().unwrap_or("");
    let remote = if let Some(url) = file.download_url.as_deref() {
        if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            format!("{}{}", base_url(c), url.trim_start_matches('/'))
        }
    } else {
        format!("{}api/backup/uploads/file/{}", base_url(c), file_id)
    };

    let target = local_root.join(rel.replace('/', "\\"));
    if let Some(parent) = target.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut r = build_client(c, true)?
        .get(remote)
        .send()
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        let status = r.status();
        let body = r.text().unwrap_or_default();
        let body_short: String = body.chars().take(300).collect();
        return Err(format!("HTTP {} body={}", status, body_short));
    }
    let mut f = File::create(&target).map_err(|e| e.to_string())?;
    r.copy_to(&mut f).map_err(|e| e.to_string())?;
    Ok(())
}

fn api_uploads_restore_create_task(
    c: &AppConfig,
    scope: &str,
    overwrite_mode: &str,
    file_count: usize,
    total_bytes: u64,
) -> Result<String, String> {
    let r = build_client(c, true)?
        .post(format!("{}api/restore/uploads/create-task", base_url(c)))
        .json(&serde_json::json!({
            "scope": scope,
            "overwriteMode": overwrite_mode,
            "fileCount": file_count,
            "totalBytes": total_bytes
        }))
        .send()
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("HTTP {}", r.status()));
    }
    let body = r.text().map_err(|e| e.to_string())?;
    if let Some(v) = parse_data::<Value>(&body) {
        if let Some(task_id) = v.get("taskId").and_then(|x| x.as_str()) {
            return Ok(task_id.to_string());
        }
    }
    Err("missing taskId".into())
}

fn api_uploads_restore_upload_file(
    c: &AppConfig,
    task_id: &str,
    relative_path: &str,
    sha256: &str,
    size: u64,
    file_path: &Path,
) -> Result<String, String> {
    let form = multipart::Form::new()
        .text("relativePath", relative_path.to_string())
        .text("sha256", sha256.to_string())
        .text("size", size.to_string())
        .file("file", file_path)
        .map_err(|e| e.to_string())?;
    let r = build_client(c, true)?
        .post(format!("{}api/restore/uploads/{}/upload-file", base_url(c), task_id))
        .multipart(form)
        .send()
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("HTTP {}", r.status()));
    }
    let body = r.text().map_err(|e| e.to_string())?;
    if let Some(v) = parse_data::<Value>(&body) {
        if let Some(status) = v.get("status").and_then(|x| x.as_str()) {
            return Ok(status.to_string());
        }
    }
    Ok("completed".to_string())
}

fn api_uploads_restore_complete(c: &AppConfig, task_id: &str) -> Result<(), String> {
    let r = build_client(c, true)?
        .post(format!("{}api/restore/uploads/{}/complete", base_url(c), task_id))
        .json(&serde_json::json!({ "finalize": true }))
        .send()
        .map_err(|e| e.to_string())?;
    if !r.status().is_success() {
        return Err(format!("HTTP {}", r.status()));
    }
    Ok(())
}

fn load_config(path: &Path) -> AppConfig {
    if let Ok(t) = fs::read_to_string(path) {
        if let Ok(c) = serde_json::from_str::<AppConfig>(&t) {
            return c;
        }
    }
    let c = AppConfig::default();
    let _ = save_config(path, &c);
    c
}

fn save_config(path: &Path, c: &AppConfig) -> Result<(), String> {
    fs::write(path, serde_json::to_string_pretty(c).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

unsafe fn set_text(hwnd: HWND, s: &str) {
    let ws = to_wide(s);
    let _ = SetWindowTextW(hwnd, PCWSTR(ws.as_ptr()));
}

unsafe fn get_text(hwnd: HWND) -> String {
    let len = GetWindowTextLengthW(hwnd);
    if len <= 0 {
        return String::new();
    }
    let mut buf = vec![0u16; (len + 1) as usize];
    let _ = GetWindowTextW(hwnd, &mut buf);
    String::from_utf16_lossy(&buf[..len as usize])
}

unsafe fn set_text_if_changed(hwnd: HWND, text: &str) {
    if hwnd.0 != 0 && get_text(hwnd) != text {
        set_text(hwnd, text);
    }
}

unsafe fn ctlcolor_same_as_window(w: WPARAM) -> LRESULT {
    let hdc = HDC(w.0 as isize);
    let _ = SetBkMode(hdc, BACKGROUND_MODE(2));
    let _ = SetBkColor(hdc, COLORREF(0x00FFFFFF));
    let _ = SetTextColor(hdc, COLORREF(0x00000000));
    let brush = GetStockObject(WHITE_BRUSH);
    LRESULT(brush.0)
}

fn zh_status(s: &str) -> &str {
    match s {
        "queued" => "排队中",
        "running" => "执行中",
        "completed" => "已完成",
        "failed" => "失败",
        "cancelled" => "已取消",
        "unknown" => "未知",
        "disconnected" => "未连接",
        "connected" => "已连接",
        _ => s,
    }
}

unsafe fn set_text_if_cache_changed(hwnd: HWND, cache: &mut String, text: &str) {
    if hwnd.0 == 0 {
        return;
    }
    if cache != text {
        set_text(hwnd, text);
        cache.clear();
        cache.push_str(text);
    }
}

unsafe fn ctrl(hwnd: HWND, id: i32) -> HWND { GetDlgItem(hwnd, id) }
unsafe fn set_default_font(hwnd: HWND) { let f = GetStockObject(DEFAULT_GUI_FONT); let _ = SendMessageW(hwnd, WM_SETFONT, WPARAM(f.0 as usize), LPARAM(1)); }

unsafe fn label(parent: HWND, text: PCWSTR, x: i32, y: i32, wv: i32, hv: i32, id: i32) -> HWND {
    let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), w!("STATIC"), text, WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0), x, y, wv, hv, parent, HMENU(id as isize), HINSTANCE(0), None);
    set_default_font(hwnd);
    hwnd
}

unsafe fn button(parent: HWND, text: PCWSTR, x: i32, y: i32, wv: i32, hv: i32, id: i32) -> HWND {
    let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), w!("BUTTON"), text, WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | BS_PUSHBUTTON as u32), x, y, wv, hv, parent, HMENU(id as isize), HINSTANCE(0), None);
    set_default_font(hwnd);
    hwnd
}

unsafe fn group_box(parent: HWND, text: PCWSTR, x: i32, y: i32, wv: i32, hv: i32, id: i32) -> HWND {
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        w!("BUTTON"),
        text,
        WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | BS_GROUPBOX as u32),
        x,
        y,
        wv,
        hv,
        parent,
        HMENU(id as isize),
        HINSTANCE(0),
        None,
    );
    set_default_font(hwnd);
    hwnd
}

unsafe fn edit(parent: HWND, x: i32, y: i32, wv: i32, hv: i32, id: i32, readonly: bool, multiline: bool, password: bool) -> HWND {
    let mut style = WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | WS_BORDER.0 | ES_LEFT as u32;
    if multiline { style |= ES_MULTILINE as u32 | ES_AUTOVSCROLL as u32 | WS_VSCROLL.0; } else { style |= ES_AUTOHSCROLL as u32; }
    if readonly { style |= ES_READONLY as u32; }
    if password { style |= ES_PASSWORD as u32; }
    let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), w!("EDIT"), w!(""), WINDOW_STYLE(style), x, y, wv, hv, parent, HMENU(id as isize), HINSTANCE(0), None);
    set_default_font(hwnd);
    hwnd
}

unsafe fn combo(parent: HWND, x: i32, y: i32, wv: i32, hv: i32, id: i32) -> HWND {
    let hwnd = CreateWindowExW(WINDOW_EX_STYLE(0), w!("COMBOBOX"), w!(""), WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | CBS_DROPDOWNLIST as u32), x, y, wv, hv, parent, HMENU(id as isize), HINSTANCE(0), None);
    set_default_font(hwnd);
    hwnd
}

unsafe fn progress_bar(parent: HWND, x: i32, y: i32, wv: i32, hv: i32, id: i32) -> HWND {
    CreateWindowExW(WINDOW_EX_STYLE(0), w!("msctls_progress32"), w!(""), WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0), x, y, wv, hv, parent, HMENU(id as isize), HINSTANCE(0), None)
}

unsafe fn progress_set(hwnd: HWND, value: f64) { let _ = SendMessageW(hwnd, PBM_SETPOS, WPARAM(value.clamp(0.0, 100.0) as usize), LPARAM(0)); }
unsafe fn combo_add(hwnd: HWND, text: &str) { let ws = to_wide(text); let _ = SendMessageW(hwnd, CB_ADDSTRING, WPARAM(0), LPARAM(ws.as_ptr() as isize)); }
unsafe fn combo_current_text(hwnd: HWND) -> String {
    let idx = SendMessageW(hwnd, CB_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;
    if idx < 0 { return String::new(); }
    let len = SendMessageW(hwnd, CB_GETLBTEXTLEN, WPARAM(idx as usize), LPARAM(0)).0 as usize;
    let mut buf = vec![0u16; len + 1];
    let _ = SendMessageW(hwnd, CB_GETLBTEXT, WPARAM(idx as usize), LPARAM(buf.as_mut_ptr() as isize));
    String::from_utf16_lossy(&buf[..len])
}

fn restore_mode(text: &str) -> &'static str {
    if text.contains("restore_data_only") { "restore_data_only" }
    else if text.contains("restore_schema_only") { "restore_schema_only" }
    else { "drop_and_restore" }
}

fn upload_overwrite_mode(text: &str) -> &'static str {
    if text.contains("overwrite_if_exists") { "overwrite_if_exists" }
    else if text.contains("overwrite_if_newer") { "overwrite_if_newer" }
    else { "skip_if_exists" }
}

fn collect_files_recursive(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::new();
    if !root.exists() {
        return Ok(out);
    }
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let iter = fs::read_dir(&dir).map_err(|e| e.to_string())?;
        for entry in iter {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.is_file() {
                out.push(p);
            }
        }
    }
    Ok(out)
}

fn sha256_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 1024 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn start_test() {
    let app = model();
    thread::spawn(move || {
        let cfg = { app.lock().unwrap().config.clone() };
        let mut m = app.lock().unwrap();
        match api_test(&cfg) {
            Ok(true) => { m.login.connection_status = "已连接".into(); m.login.error_message.clear(); }
            Ok(false) => { m.login.connection_status = "未连接".into(); m.login.error_message = "连接测试失败".into(); }
            Err(e) => { m.login.connection_status = "未连接".into(); m.login.error_message = format!("连接测试失败: {}", e); }
        }
    });
}

fn start_login(username: String, password: String) {
    let app = model();
    thread::spawn(move || {
        let cfg = { app.lock().unwrap().config.clone() };
        match api_login(&cfg, &username, &password) {
            Ok(token) => {
                let mut m = app.lock().unwrap();
                m.config.token = token;
                m.login.error_message.clear();
                m.login.connection_status = "已连接".into();
                if let Some(name) = api_me(&m.config) { m.status = format!("当前用户: {}", name); }
                m.login.login_success = true;
                let _ = save_config(&m.config_path, &m.config);
            }
            Err(e) => {
                let mut m = app.lock().unwrap();
                m.login.connection_status = "未连接".into();
                m.login.error_message = format!("登录失败: {}", e);
                m.login.login_success = false;
            }
        }
    });
}
fn start_backup() {
    let app = model();
    thread::spawn(move || {
        let (cfg, fmt, out_dir) = {
            let m = app.lock().unwrap();
            (m.config.clone(), m.backup.backup_format.clone(), m.backup.dir.clone())
        };
        let db_name = cfg.effective_db_name();
        if out_dir.trim().is_empty() {
            let mut m = app.lock().unwrap();
            append_log(&mut m.backup.log, "备份目录为空");
            m.backup.status = "失败".into();
            return;
        }
        if let Err(e) = fs::create_dir_all(&out_dir) {
            let mut m = app.lock().unwrap();
            append_log(&mut m.backup.log, &format!("创建备份目录失败: {}", e));
            m.backup.status = "失败".into();
            return;
        }
        {
            let mut m = app.lock().unwrap();
            append_log(&mut m.backup.log, &format!("开始数据库备份: dbName={}, 格式={}", db_name, fmt));
            m.backup.status = "排队中".into();
            m.backup.progress = 0.0;
            m.backup.legacy = false;
        }

        let preflight = match api_backup_preflight(&cfg) {
            Ok(v) => v,
            Err(e) => {
                let mut m = app.lock().unwrap();
                append_log(&mut m.backup.log, &format!("备份预检失败: {}", e));
                m.backup.status = "失败".into();
                return;
            }
        };
        {
            let mut m = app.lock().unwrap();
            append_log(
                &mut m.backup.log,
                &format!(
                    "预检通过: serverVersion={}, resolvedMajor={}, strategy={}",
                    preflight.server_version.as_deref().unwrap_or("-"),
                    preflight
                        .resolved_major
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into()),
                    preflight.selected_strategy.as_deref().unwrap_or("-")
                ),
            );
            if let Some(image) = preflight.selected_tools_image.as_deref() {
                if !image.trim().is_empty() {
                    append_log(&mut m.backup.log, &format!("预检工具镜像: {}", image));
                }
            }
            if let Some(warnings) = preflight.warnings.as_ref() {
                for warning in warnings {
                    append_log(&mut m.backup.log, &format!("预检警告: {}", warning));
                }
            }
            if preflight.can_proceed == Some(false) {
                append_log(
                    &mut m.backup.log,
                    &format!(
                        "预检阻止执行: {}",
                        preflight.blocking_reason.as_deref().unwrap_or("unknown")
                    ),
                );
                m.backup.status = "失败".into();
                return;
            }
        }

        let task_id = match api_backup_create(&cfg, &fmt) {
            Ok(v) => v,
            Err(e) => {
                let mut m = app.lock().unwrap();
                append_log(&mut m.backup.log, &format!("创建任务失败: {}", e));
                m.backup.status = "失败".into();
                return;
            }
        };

        {
            let mut m = app.lock().unwrap();
            m.backup.task_id = task_id.clone();
            append_log(&mut m.backup.log, &format!("备份任务已创建: {}", task_id));
        }

        loop {
            match api_task(&cfg, &task_id) {
                Ok(task) => {
                    let status = task.status.unwrap_or_else(|| "unknown".into());
                    let pct = task.progress.and_then(|p| p.percent).unwrap_or(0.0);
                    { let mut m = app.lock().unwrap(); m.backup.status = zh_status(&status).to_string(); m.backup.progress = pct; }

                    if matches!(status.as_str(), "completed" | "failed" | "cancelled") {
                        let mut m = app.lock().unwrap();
                        if status == "failed" {
                            let msg = task.error_message.or(task.message).unwrap_or_else(|| "unknown".into());
                            append_log(&mut m.backup.log, &format!("任务失败: {}", msg));
                        } else if status == "completed" {
                            let file_name = api_backup_metadata(&cfg, &task_id)
                                .ok()
                                .and_then(|meta| meta.file_name)
                                .filter(|name| !name.trim().is_empty())
                                .unwrap_or_else(|| format!("{}.{}", task_id, if fmt == "plain" { "sql" } else { "dump" }));
                            let out = PathBuf::from(&out_dir).join(file_name);
                            drop(m);
                            match api_backup_download(&cfg, &task_id, &out) {
                                Ok(_) => {
                                    let mut mm = app.lock().unwrap();
                                    mm.backup.status = "已完成".into();
                                    mm.backup.progress = 100.0;
                                    append_log(&mut mm.backup.log, &format!("任务完成: {}", zh_status(&status)));
                                    append_log(&mut mm.backup.log, &format!("已下载备份文件: {}", out.display()));
                                }
                                Err(e) => {
                                    let mut mm = app.lock().unwrap();
                                    mm.backup.status = "失败".into();
                                    append_log(&mut mm.backup.log, &format!("下载备份文件失败: {}", e));
                                }
                            }
                        } else {
                            append_log(&mut m.backup.log, &format!("任务完成: {}", zh_status(&status)));
                        }
                        break;
                    }
                }
                Err(e) => {
                    let mut m = app.lock().unwrap();
                    append_log(&mut m.backup.log, &format!("任务轮询失败: {}", e));
                    break;
                }
            }
            thread::sleep(Duration::from_millis(1500));
        }
    });
}

fn start_restore() {
    let app = model();
    thread::spawn(move || {
        let (cfg, file, mode) = { let m = app.lock().unwrap(); (m.config.clone(), m.restore.file.clone(), m.restore.mode.clone()) };
        let db_name = cfg.effective_db_name();
        if !Path::new(&file).exists() {
            let mut m = app.lock().unwrap();
            append_log(&mut m.restore.log, "恢复文件不存在");
            m.restore.status = "失败".into();
            return;
        }

        {
            let mut m = app.lock().unwrap();
            m.restore.status = "执行中".into();
            m.restore.progress = 10.0;
            append_log(&mut m.restore.log, &format!("开始数据库恢复: targetDbName={}, restoreMode={}", db_name, mode));
            m.restore.progress = 20.0;
        }

        let upload_id = match api_restore_upload(&cfg, Path::new(&file)) {
            Ok(v) => v,
            Err(e) => {
                let mut m = app.lock().unwrap();
                m.restore.status = "失败".into();
                append_log(&mut m.restore.log, &format!("上传恢复文件失败: {}", e));
                return;
            }
        };
        {
            let mut m = app.lock().unwrap();
            append_log(&mut m.restore.log, &format!("恢复文件上传完成: {}", upload_id));
            m.restore.progress = 35.0;
        }

        let preflight = match api_restore_preflight(&cfg) {
            Ok(v) => v,
            Err(e) => {
                let mut m = app.lock().unwrap();
                m.restore.status = "失败".into();
                append_log(&mut m.restore.log, &format!("恢复预检失败: {}", e));
                return;
            }
        };
        {
            let mut m = app.lock().unwrap();
            append_log(
                &mut m.restore.log,
                &format!(
                    "恢复预检通过: serverVersion={}, resolvedMajor={}, strategy={}",
                    preflight.server_version.as_deref().unwrap_or("-"),
                    preflight
                        .resolved_major
                        .map(|v| v.to_string())
                        .unwrap_or_else(|| "-".into()),
                    preflight.selected_strategy.as_deref().unwrap_or("-")
                ),
            );
            if let Some(image) = preflight.selected_tools_image.as_deref() {
                if !image.trim().is_empty() {
                    append_log(&mut m.restore.log, &format!("恢复预检工具镜像: {}", image));
                }
            }
            if let Some(warnings) = preflight.warnings.as_ref() {
                for warning in warnings {
                    append_log(&mut m.restore.log, &format!("恢复预检警告: {}", warning));
                }
            }
            if preflight.can_proceed == Some(false) {
                append_log(
                    &mut m.restore.log,
                    &format!(
                        "恢复预检阻止执行: {}",
                        preflight.blocking_reason.as_deref().unwrap_or("unknown")
                    ),
                );
                m.restore.status = "失败".into();
                return;
            }
            m.restore.progress = 50.0;
        }

        let confirm = if mode == "drop_and_restore" { "CONFIRM RESTORE" } else { "" };
        let task_id = match api_restore_create(&cfg, &upload_id, &mode, confirm) {
            Ok(v) => v,
            Err(e) => {
                let mut m = app.lock().unwrap();
                m.restore.status = "失败".into();
                append_log(&mut m.restore.log, &format!("创建恢复任务失败: {}", e));
                return;
            }
        };

        {
            let mut m = app.lock().unwrap();
            m.restore.task_id = task_id.clone();
            append_log(&mut m.restore.log, &format!("恢复任务已创建: {}", task_id));
        }

        loop {
            match api_task(&cfg, &task_id) {
                Ok(task) => {
                    let status = task.status.unwrap_or_else(|| "unknown".into());
                    let pct = task.progress.and_then(|p| p.percent).unwrap_or(0.0);
                    { let mut m = app.lock().unwrap(); m.restore.status = zh_status(&status).to_string(); m.restore.progress = pct; }

                    if matches!(status.as_str(), "completed" | "failed" | "cancelled") {
                        let mut m = app.lock().unwrap();
                        if status == "failed" {
                            let msg = task.error_message.or(task.message).unwrap_or_else(|| "unknown".into());
                            append_log(&mut m.restore.log, &format!("恢复任务失败: {}", msg));
                            if msg.contains("transaction_timeout") {
                                append_log(&mut m.restore.log, "检测到服务端 pg_restore 与目标数据库版本兼容问题(transaction_timeout)。");
                            }
                        } else {
                            append_log(&mut m.restore.log, &format!("恢复任务完成: {}", zh_status(&status)));
                        }
                        break;
                    }
                }
                Err(e) => {
                    let mut m = app.lock().unwrap();
                    append_log(&mut m.restore.log, &format!("任务轮询失败: {}", e));
                    break;
                }
            }
            thread::sleep(Duration::from_millis(1500));
        }
    });
}

unsafe fn show_tab(hwnd: HWND, index: i32) {
    let show_backup = if index == 0 { SW_SHOW } else { SW_HIDE };
    let show_restore = if index == 1 { SW_SHOW } else { SW_HIDE };

    let backup = backup_ctrl_hwnds().lock().unwrap().clone();
    for raw in backup {
        let _ = ShowWindow(HWND(raw), show_backup);
    }

    let restore = restore_ctrl_hwnds().lock().unwrap().clone();
    for raw in restore {
        let _ = ShowWindow(HWND(raw), show_restore);
    }
}

fn start_uploads_backup() {
    let app = model();
    thread::spawn(move || {
        let (cfg, dir) = {
            let m = app.lock().unwrap();
            (m.config.clone(), m.uploads_backup.dir.clone())
        };
        {
            let mut m = app.lock().unwrap();
            m.uploads_backup.progress = 0.0;
            m.uploads_backup.status = "正在获取清单".into();
            append_log(&mut m.uploads_backup.log, "开始图片备份");
        }

        let manifest = match api_uploads_manifest(&cfg) {
            Ok(v) => v,
            Err(e) => {
                let mut m = app.lock().unwrap();
                m.uploads_backup.status = "失败".into();
                append_log(&mut m.uploads_backup.log, &format!("获取清单失败: {}", e));
                return;
            }
        };

        let files = manifest.files.unwrap_or_default();
        let total = files.len();
        let total_hint = manifest.file_count.unwrap_or(total as u64);
        {
            let mut m = app.lock().unwrap();
            m.uploads_backup.status = format!("下载中: 0/{}", total_hint);
            append_log(&mut m.uploads_backup.log, &format!("清单已加载: 文件数={}", total_hint));
        }

        let root = PathBuf::from(dir);
        let _ = fs::create_dir_all(&root);
        let mut ok_count = 0usize;
        let mut fail_count = 0usize;
        for (i, f) in files.iter().enumerate() {
            match api_uploads_download_file(&cfg, f, &root) {
                Ok(_) => ok_count += 1,
                Err(e) => {
                    fail_count += 1;
                    let mut m = app.lock().unwrap();
                    append_log(
                        &mut m.uploads_backup.log,
                        &format!(
                            "下载失败: {} ({})",
                            f.relative_path.clone().unwrap_or_default(),
                            e
                        ),
                    );
                }
            }
            let pct = if total == 0 { 100.0 } else { ((i + 1) as f64 / total as f64) * 100.0 };
            let mut m = app.lock().unwrap();
            m.uploads_backup.progress = pct;
            m.uploads_backup.status = format!("下载中: {}/{}", i + 1, total);
        }

        let mut m = app.lock().unwrap();
        m.uploads_backup.progress = 100.0;
        m.uploads_backup.status = if fail_count == 0 { "已完成".into() } else { "完成(有失败)".into() };
        append_log(&mut m.uploads_backup.log, &format!("图片备份完成: 成功={}, 失败={}", ok_count, fail_count));
    });
}

fn start_uploads_restore_scan() {
    let app = model();
    thread::spawn(move || {
        let root = { app.lock().unwrap().uploads_restore.dir.clone() };
        {
            let mut m = app.lock().unwrap();
            m.uploads_restore.status = "扫描目录中".into();
            m.uploads_restore.progress = 0.0;
            append_log(&mut m.uploads_restore.log, &format!("开始扫描目录: {}", root));
        }

        let files = match collect_files_recursive(Path::new(&root)) {
            Ok(v) => v,
            Err(e) => {
                let mut m = app.lock().unwrap();
                m.uploads_restore.status = "失败".into();
                append_log(&mut m.uploads_restore.log, &format!("扫描失败: {}", e));
                return;
            }
        };

        let mut total_bytes = 0u64;
        let mut file_list = Vec::with_capacity(files.len());
        for p in files {
            if let Ok(meta) = fs::metadata(&p) {
                total_bytes = total_bytes.saturating_add(meta.len());
            }
            file_list.push(p.to_string_lossy().to_string());
        }

        let mut m = app.lock().unwrap();
        m.uploads_restore.scanned_file_count = file_list.len();
        m.uploads_restore.scanned_total_bytes = total_bytes;
        m.uploads_restore.scanned_files = file_list;
        let scanned_count = m.uploads_restore.scanned_file_count;
        let scanned_bytes = m.uploads_restore.scanned_total_bytes;
        m.uploads_restore.status = format!("扫描完成: {} 个文件", scanned_count);
        append_log(
            &mut m.uploads_restore.log,
            &format!("扫描完成: 文件数={}, 总字节数={}", scanned_count, scanned_bytes),
        );
    });
}

fn start_uploads_restore_create_task() {
    let app = model();
    thread::spawn(move || {
        let (cfg, mode, count, total_bytes) = {
            let m = app.lock().unwrap();
            (
                m.config.clone(),
                m.uploads_restore.overwrite_mode.clone(),
                m.uploads_restore.scanned_file_count,
                m.uploads_restore.scanned_total_bytes,
            )
        };

        if count == 0 {
            let mut m = app.lock().unwrap();
            append_log(&mut m.uploads_restore.log, "请先扫描目录（未找到可上传文件）");
            m.uploads_restore.status = "空闲".into();
            return;
        }

        {
            let mut m = app.lock().unwrap();
            m.uploads_restore.status = "创建恢复任务中".into();
        }
        match api_uploads_restore_create_task(&cfg, "uploads", &mode, count, total_bytes) {
            Ok(task_id) => {
                let mut m = app.lock().unwrap();
                m.uploads_restore.task_id = task_id.clone();
                m.uploads_restore.status = format!("任务已创建: {}", task_id);
                append_log(&mut m.uploads_restore.log, &format!("图片恢复任务已创建: {}", task_id));
            }
            Err(e) => {
                let mut m = app.lock().unwrap();
                m.uploads_restore.status = "失败".into();
                append_log(&mut m.uploads_restore.log, &format!("创建图片恢复任务失败: {}", e));
            }
        }
    });
}

fn start_uploads_restore_upload() {
    let app = model();
    thread::spawn(move || {
        let (cfg, root, mut task_id, mode, mut files, count, total_bytes) = {
            let m = app.lock().unwrap();
            (
                m.config.clone(),
                m.uploads_restore.dir.clone(),
                m.uploads_restore.task_id.clone(),
                m.uploads_restore.overwrite_mode.clone(),
                m.uploads_restore.scanned_files.clone(),
                m.uploads_restore.scanned_file_count,
                m.uploads_restore.scanned_total_bytes,
            )
        };

        if files.is_empty() {
            files = match collect_files_recursive(Path::new(&root)) {
                Ok(v) => v.into_iter().map(|p| p.to_string_lossy().to_string()).collect(),
                Err(e) => {
                    let mut m = app.lock().unwrap();
                    m.uploads_restore.status = "失败".into();
                    append_log(&mut m.uploads_restore.log, &format!("上传前扫描失败: {}", e));
                    return;
                }
            };
        }
        if files.is_empty() {
            let mut m = app.lock().unwrap();
            append_log(&mut m.uploads_restore.log, "没有可上传的文件");
            return;
        }

        if task_id.trim().is_empty() {
            match api_uploads_restore_create_task(&cfg, "uploads", &mode, count.max(files.len()), total_bytes) {
                Ok(new_task_id) => {
                    task_id = new_task_id.clone();
                    let mut m = app.lock().unwrap();
                    m.uploads_restore.task_id = new_task_id.clone();
                    append_log(&mut m.uploads_restore.log, &format!("已自动创建图片恢复任务: {}", new_task_id));
                }
                Err(e) => {
                    let mut m = app.lock().unwrap();
                    m.uploads_restore.status = "失败".into();
                    append_log(&mut m.uploads_restore.log, &format!("自动创建图片恢复任务失败: {}", e));
                    return;
                }
            }
        }

        {
            let mut m = app.lock().unwrap();
            m.uploads_restore.progress = 0.0;
            m.uploads_restore.completed_count = 0;
            m.uploads_restore.skipped_count = 0;
            m.uploads_restore.overwritten_count = 0;
            m.uploads_restore.failed_count = 0;
            m.uploads_restore.status = "上传中".into();
            append_log(&mut m.uploads_restore.log, "开始图片恢复上传");
        }

        let mut ok_count = 0usize;
        let mut skipped_count = 0usize;
        let mut overwritten_count = 0usize;
        let mut fail_count = 0usize;
        let total = files.len();
        for (idx, full_path) in files.iter().enumerate() {
            let p = PathBuf::from(full_path);
            let relative = match p.strip_prefix(&root) {
                Ok(v) => v.to_string_lossy().replace('\\', "/"),
                Err(_) => {
                    fail_count += 1;
                    let mut m = app.lock().unwrap();
                    append_log(&mut m.uploads_restore.log, &format!("跳过非法路径: {}", p.display()));
                    continue;
                }
            };
            let size = match fs::metadata(&p) {
                Ok(meta) => meta.len(),
                Err(e) => {
                    fail_count += 1;
                    let mut m = app.lock().unwrap();
                    append_log(&mut m.uploads_restore.log, &format!("读取文件信息失败: {} ({})", relative, e));
                    continue;
                }
            };
            let hash = match sha256_file(&p) {
                Ok(v) => v,
                Err(e) => {
                    fail_count += 1;
                    let mut m = app.lock().unwrap();
                    append_log(&mut m.uploads_restore.log, &format!("计算哈希失败: {} ({})", relative, e));
                    continue;
                }
            };

            let mut last_err = String::new();
            let mut upload_status = String::new();
            for _ in 0..=2 {
                match api_uploads_restore_upload_file(&cfg, &task_id, &relative, &hash, size, &p) {
                    Ok(s) => {
                        upload_status = s;
                        break;
                    }
                    Err(e) => {
                        last_err = e;
                        thread::sleep(Duration::from_millis(500));
                    }
                }
            }

            if upload_status == "skipped" {
                skipped_count += 1;
            } else if !upload_status.is_empty() {
                ok_count += 1;
                overwritten_count += 1;
            } else {
                fail_count += 1;
                let mut m = app.lock().unwrap();
                append_log(&mut m.uploads_restore.log, &format!("上传失败: {} ({})", relative, last_err));
            }

            let mut m = app.lock().unwrap();
            m.uploads_restore.current_file = relative.clone();
            m.uploads_restore.completed_count = ok_count;
            m.uploads_restore.skipped_count = skipped_count;
            m.uploads_restore.overwritten_count = overwritten_count;
            m.uploads_restore.failed_count = fail_count;
            m.uploads_restore.progress = ((idx + 1) as f64 / total as f64) * 100.0;
            m.uploads_restore.status = format!(
                "上传中: {}/{} (总计{} 跳过{} 覆盖{} 失败{})",
                idx + 1,
                total,
                total,
                skipped_count,
                overwritten_count,
                fail_count
            );
        }

        let complete_result = api_uploads_restore_complete(&cfg, &task_id);
        let mut m = app.lock().unwrap();
        if let Err(e) = complete_result {
            append_log(&mut m.uploads_restore.log, &format!("完成恢复任务失败: {}", e));
        }
        m.uploads_restore.progress = 100.0;
        m.uploads_restore.status = if fail_count == 0 { "已完成".into() } else { "完成(有失败)".into() };
        append_log(
            &mut m.uploads_restore.log,
            &format!(
                "图片恢复完成: 总计={}, 跳过={}, 覆盖={}, 失败={}",
                total, skipped_count, overwritten_count, fail_count
            ),
        );
    });
}

unsafe fn init_main_ui(hwnd: HWND) {
    backup_ctrl_hwnds().lock().unwrap().clear();
    restore_ctrl_hwnds().lock().unwrap().clear();

    label(hwnd, w!("WhereIsIt 备份恢复"), 12, 12, 260, 28, 0);
    label(hwnd, w!("状态:"), 280, 16, 60, 24, 0);
    label(hwnd, w!("就绪"), 344, 16, 560, 24, IDC_MAIN_STATUS);

    let tab = CreateWindowExW(WINDOW_EX_STYLE(0), w!("SysTabControl32"), w!(""), WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_CLIPCHILDREN.0), 12, 46, 892, 34, hwnd, HMENU(IDC_MAIN_TAB as isize), HINSTANCE(0), None);
    set_default_font(tab);

    let mut item1 = TCITEMW::default();
    item1.mask = TCIF_TEXT;
    let mut text1 = to_wide("备份");
    item1.pszText = PWSTR(text1.as_mut_ptr());
    let _ = SendMessageW(tab, TCM_INSERTITEMW, WPARAM(0), LPARAM((&item1 as *const TCITEMW) as isize));

    let mut item2 = TCITEMW::default();
    item2.mask = TCIF_TEXT;
    let mut text2 = to_wide("恢复");
    item2.pszText = PWSTR(text2.as_mut_ptr());
    let _ = SendMessageW(tab, TCM_INSERTITEMW, WPARAM(1), LPARAM((&item2 as *const TCITEMW) as isize));

    track_backup(group_box(hwnd, w!("数据库备份"), 20, 90, 840, 276, IDC_DB_BACKUP_TITLE));
    track_backup(edit(hwnd, 32, 154, 630, 28, IDC_DB_BACKUP_DIR, true, false, false));
    track_backup(button(hwnd, w!("选择目录"), 672, 154, 150, 28, IDC_DB_BACKUP_BROWSE));
    track_backup(button(hwnd, w!("备份数据库"), 32, 190, 120, 28, IDC_DB_BACKUP_START));
    track_backup(progress_bar(hwnd, 32, 226, 790, 18, IDC_DB_BACKUP_PROGRESS));
    track_backup(label(hwnd, w!("空闲"), 32, 250, 790, 22, IDC_DB_BACKUP_STATUS));
    track_backup(edit(hwnd, 32, 274, 790, 76, IDC_DB_BACKUP_LOG, true, true, false));

    track_backup(group_box(hwnd, w!("图片备份"), 20, 376, 840, 220, IDC_UP_BACKUP_TITLE));
    track_backup(edit(hwnd, 32, 408, 630, 28, IDC_UP_BACKUP_DIR, true, false, false));
    track_backup(button(hwnd, w!("选择目录"), 672, 408, 150, 28, IDC_UP_BACKUP_BROWSE));
    track_backup(button(hwnd, w!("备份图片"), 32, 444, 120, 28, IDC_UP_BACKUP_START));
    track_backup(progress_bar(hwnd, 32, 480, 790, 18, IDC_UP_BACKUP_PROGRESS));
    track_backup(label(hwnd, w!("空闲"), 32, 502, 790, 20, IDC_UP_BACKUP_STATUS));
    track_backup(edit(hwnd, 32, 524, 790, 64, IDC_UP_BACKUP_LOG, true, true, false));

    track_restore(group_box(hwnd, w!("数据库恢复"), 20, 90, 840, 276, IDC_DB_RESTORE_TITLE));
    track_restore(edit(hwnd, 32, 122, 630, 28, IDC_DB_RESTORE_FILE, true, false, false));
    track_restore(button(hwnd, w!("选择文件"), 672, 122, 150, 28, IDC_DB_RESTORE_BROWSE));
    track_restore(label(hwnd, w!("模式:"), 32, 160, 60, 24, IDC_DB_RESTORE_MODE_LABEL));
    let cr = combo(hwnd, 92, 156, 300, 220, IDC_DB_RESTORE_MODE);
    track_restore(cr);
    combo_add(cr, "删除并恢复 (drop_and_restore)");
    combo_add(cr, "仅恢复数据 (restore_data_only)");
    combo_add(cr, "仅恢复结构 (restore_schema_only)");
    let _ = SendMessageW(cr, CB_SETCURSEL, WPARAM(0), LPARAM(0));
    track_restore(button(hwnd, w!("开始恢复"), 32, 212, 120, 28, IDC_DB_RESTORE_START));
    track_restore(progress_bar(hwnd, 32, 246, 790, 18, IDC_DB_RESTORE_PROGRESS));
    track_restore(label(hwnd, w!("空闲"), 32, 270, 790, 20, IDC_DB_RESTORE_STATUS));
    track_restore(edit(hwnd, 32, 292, 790, 58, IDC_DB_RESTORE_LOG, true, true, false));

    track_restore(group_box(hwnd, w!("图片恢复"), 20, 376, 840, 220, IDC_UP_RESTORE_TITLE));
    track_restore(edit(hwnd, 32, 408, 630, 28, IDC_UP_RESTORE_DIR, true, false, false));
    track_restore(button(hwnd, w!("选择目录"), 672, 408, 150, 28, IDC_UP_RESTORE_BROWSE));
    track_restore(label(hwnd, w!("覆盖模式:"), 32, 444, 72, 24, IDC_UP_RESTORE_MODE_LABEL));
    let ur = combo(hwnd, 108, 440, 230, 220, IDC_UP_RESTORE_MODE);
    track_restore(ur);
    combo_add(ur, "跳过已存在 (skip_if_exists)");
    combo_add(ur, "覆盖已存在 (overwrite_if_exists)");
    combo_add(ur, "仅较新覆盖 (overwrite_if_newer)");
    let _ = SendMessageW(ur, CB_SETCURSEL, WPARAM(0), LPARAM(0));
    track_restore(button(hwnd, w!("扫描目录"), 348, 440, 100, 28, IDC_UP_RESTORE_SCAN));
    track_restore(button(hwnd, w!("创建恢复任务"), 458, 440, 120, 28, IDC_UP_RESTORE_CREATE));
    track_restore(button(hwnd, w!("开始上传"), 588, 440, 100, 28, IDC_UP_RESTORE_START));
    track_restore(progress_bar(hwnd, 32, 476, 790, 18, IDC_UP_RESTORE_PROGRESS));
    track_restore(label(hwnd, w!("空闲"), 32, 498, 790, 20, IDC_UP_RESTORE_STATUS));
    track_restore(edit(hwnd, 32, 520, 790, 68, IDC_UP_RESTORE_LOG, true, true, false));

    show_tab(hwnd, 0);
    let _ = SetTimer(hwnd, TIMER_ID, 250, None);
}

unsafe fn refresh_main(hwnd: HWND) {
    let app = model();
    let m = app.lock().unwrap();
    let mut c = ui_cache().lock().unwrap();

    set_text_if_cache_changed(ctrl(hwnd, IDC_MAIN_STATUS), &mut c.main_status, &m.status);
    set_text_if_cache_changed(ctrl(hwnd, IDC_DB_BACKUP_DIR), &mut c.backup_dir, &m.backup.dir);
    set_text_if_cache_changed(ctrl(hwnd, IDC_DB_BACKUP_STATUS), &mut c.backup_status, &m.backup.status);
    set_text_if_cache_changed(ctrl(hwnd, IDC_DB_BACKUP_LOG), &mut c.backup_log, &m.backup.log);
    progress_set(ctrl(hwnd, IDC_DB_BACKUP_PROGRESS), m.backup.progress);

    set_text_if_cache_changed(ctrl(hwnd, IDC_DB_RESTORE_FILE), &mut c.restore_file, &m.restore.file);
    set_text_if_cache_changed(ctrl(hwnd, IDC_DB_RESTORE_STATUS), &mut c.restore_status, &m.restore.status);
    set_text_if_cache_changed(ctrl(hwnd, IDC_DB_RESTORE_LOG), &mut c.restore_log, &m.restore.log);
    progress_set(ctrl(hwnd, IDC_DB_RESTORE_PROGRESS), m.restore.progress);

    set_text_if_cache_changed(ctrl(hwnd, IDC_UP_BACKUP_DIR), &mut c.uploads_backup_dir, &m.uploads_backup.dir);
    set_text_if_cache_changed(ctrl(hwnd, IDC_UP_BACKUP_STATUS), &mut c.uploads_backup_status, &m.uploads_backup.status);
    set_text_if_cache_changed(ctrl(hwnd, IDC_UP_BACKUP_LOG), &mut c.uploads_backup_log, &m.uploads_backup.log);
    progress_set(ctrl(hwnd, IDC_UP_BACKUP_PROGRESS), m.uploads_backup.progress);

    set_text_if_cache_changed(ctrl(hwnd, IDC_UP_RESTORE_DIR), &mut c.uploads_restore_dir, &m.uploads_restore.dir);
    set_text_if_cache_changed(ctrl(hwnd, IDC_UP_RESTORE_STATUS), &mut c.uploads_restore_status, &m.uploads_restore.status);
    set_text_if_cache_changed(ctrl(hwnd, IDC_UP_RESTORE_LOG), &mut c.uploads_restore_log, &m.uploads_restore.log);
    progress_set(ctrl(hwnd, IDC_UP_RESTORE_PROGRESS), m.uploads_restore.progress);
}

unsafe fn handle_main_command(hwnd: HWND, id: i32) {
    match id {
        IDC_DB_BACKUP_BROWSE => {
            if let Some(p) = rfd::FileDialog::new().pick_folder() {
                let path = p.to_string_lossy().to_string();
                let app = model();
                let mut m = app.lock().unwrap();
                m.backup.dir = path.clone();
                m.status = format!("已选择数据库备份目录: {}", path);
                drop(m);
                set_text(ctrl(hwnd, IDC_DB_BACKUP_DIR), &path);
            }
        }
        IDC_DB_BACKUP_START => {
            let app = model();
            let mut m = app.lock().unwrap();
            if m.backup.dir.trim().is_empty() {
                append_log(&mut m.backup.log, "请先选择数据库备份目录");
                m.backup.status = "失败".into();
                return;
            }
            m.backup.backup_format = "custom".to_string();
            if m.config.effective_db_name().trim().is_empty() {
                append_log(&mut m.backup.log, "配置中的 modules.database.dbName 为空");
                m.backup.status = "失败".into();
                return;
            }
            drop(m);
            start_backup();
        }
        IDC_DB_RESTORE_BROWSE => {
            if let Some(p) = rfd::FileDialog::new().pick_file() {
                let path = p.to_string_lossy().to_string();
                let app = model();
                let mut m = app.lock().unwrap();
                m.restore.file = path.clone();
                drop(m);
                set_text(ctrl(hwnd, IDC_DB_RESTORE_FILE), &path);
            }
        }
        IDC_DB_RESTORE_START => {
            let mode = restore_mode(&combo_current_text(ctrl(hwnd, IDC_DB_RESTORE_MODE))).to_string();
            if mode == "drop_and_restore" {
                let ret = MessageBoxW(hwnd, w!("该操作会覆盖目标数据库数据，是否继续？"), w!("确认"), MB_YESNO | MB_ICONWARNING);
                if ret.0 != 6 { return; }
            }
            let app = model();
            let mut m = app.lock().unwrap();
            if m.restore.file.trim().is_empty() {
                append_log(&mut m.restore.log, "请先选择恢复文件");
                m.restore.status = "失败".into();
                return;
            }
            if m.config.effective_db_name().trim().is_empty() {
                append_log(&mut m.restore.log, "配置中的 modules.database.dbName 为空");
                m.restore.status = "失败".into();
                return;
            }
            m.restore.mode = mode;
            drop(m);
            start_restore();
        }
        IDC_UP_BACKUP_BROWSE => {
            if let Some(p) = rfd::FileDialog::new().pick_folder() {
                let path = p.to_string_lossy().to_string();
                let app = model();
                let mut m = app.lock().unwrap();
                m.uploads_backup.dir = path.clone();
                m.status = format!("已选择图片备份目录: {}", path);
            }
        }
        IDC_UP_BACKUP_START => {
            start_uploads_backup();
        }
        IDC_UP_RESTORE_BROWSE => {
            if let Some(p) = rfd::FileDialog::new().pick_folder() {
                let path = p.to_string_lossy().to_string();
                let app = model();
                let mut m = app.lock().unwrap();
                m.uploads_restore.dir = path.clone();
                m.status = format!("已选择图片恢复目录: {}", path);
            }
        }
        IDC_UP_RESTORE_SCAN => {
            start_uploads_restore_scan();
        }
        IDC_UP_RESTORE_CREATE => {
            let mode = upload_overwrite_mode(&combo_current_text(ctrl(hwnd, IDC_UP_RESTORE_MODE))).to_string();
            let app = model();
            let mut m = app.lock().unwrap();
            m.uploads_restore.overwrite_mode = mode.clone();
            append_log(&mut m.uploads_restore.log, &format!("创建恢复任务请求, 模式={}", mode));
            drop(m);
            start_uploads_restore_create_task();
        }
        IDC_UP_RESTORE_START => {
            let mode = upload_overwrite_mode(&combo_current_text(ctrl(hwnd, IDC_UP_RESTORE_MODE))).to_string();
            let app = model();
            let mut m = app.lock().unwrap();
            m.uploads_restore.overwrite_mode = mode.clone();
            m.uploads_restore.progress = 0.0;
            append_log(&mut m.uploads_restore.log, &format!("开始图片恢复上传请求, 模式={}", mode));
            drop(m);
            start_uploads_restore_upload();
        }
        _ => {}
    }
}

unsafe extern "system" fn login_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    let _ = l;
    match msg {
        WM_CREATE => {
            let cfg = model().lock().unwrap().config.clone();
            label(hwnd, w!("服务器地址"), 24, 24, 120, 24, 0);
            let h_server = edit(hwnd, 24, 48, 560, 28, IDC_LOGIN_SERVER, false, false, false);
            set_text(h_server, &cfg.server_base_url);

            label(hwnd, w!("用户名"), 24, 88, 120, 24, 0);
            let h_user = edit(hwnd, 24, 112, 560, 28, IDC_LOGIN_USER, false, false, false);
            set_text(h_user, &cfg.admin_username);

            label(hwnd, w!("密码"), 24, 152, 120, 24, 0);
            edit(hwnd, 24, 176, 560, 28, IDC_LOGIN_PASS, false, false, true);

            button(hwnd, w!("测试连接"), 24, 220, 120, 30, IDC_LOGIN_TEST);
            button(hwnd, w!("登录"), 160, 220, 120, 30, IDC_LOGIN_GO);
            label(hwnd, w!("未连接"), 24, 266, 560, 24, IDC_LOGIN_STATUS);
            edit(hwnd, 24, 294, 560, 80, IDC_LOGIN_ERROR, true, true, false);
            let _ = SetTimer(hwnd, TIMER_ID, 250, None);
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (w.0 as u32 & 0xFFFF) as i32;
            let code = ((w.0 as u32 >> 16) & 0xFFFF) as u16;
            if code == 0 {
                match id {
                    IDC_LOGIN_TEST => {
                        let server = get_text(ctrl(hwnd, IDC_LOGIN_SERVER));
                        let user = get_text(ctrl(hwnd, IDC_LOGIN_USER));
                        let app = model();
                        let mut m = app.lock().unwrap();
                        m.config.server_base_url = server;
                        m.config.admin_username = user;
                        drop(m);
                        start_test();
                    }
                    IDC_LOGIN_GO => {
                        let server = get_text(ctrl(hwnd, IDC_LOGIN_SERVER));
                        let user = get_text(ctrl(hwnd, IDC_LOGIN_USER));
                        let pass = get_text(ctrl(hwnd, IDC_LOGIN_PASS));
                        let app = model();
                        let mut m = app.lock().unwrap();
                        m.config.server_base_url = server;
                        m.config.admin_username = user.clone();
                        drop(m);
                        start_login(user, pass);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            let app = model();
            let mut m = app.lock().unwrap();
            let mut c = ui_cache().lock().unwrap();
            set_text_if_cache_changed(ctrl(hwnd, IDC_LOGIN_STATUS), &mut c.login_status, &m.login.connection_status);
            set_text_if_cache_changed(ctrl(hwnd, IDC_LOGIN_ERROR), &mut c.login_error, &m.login.error_message);

            if m.login.login_success {
                m.login.login_success = false;
                SWITCHING.store(true, Ordering::SeqCst);
                drop(m);

                let mut main = HWND(MAIN_HWND.load(Ordering::SeqCst));
                if main.0 == 0 {
                    main = create_main();
                    if main.0 != 0 { MAIN_HWND.store(main.0, Ordering::SeqCst); }
                }
                let _ = ShowWindow(main, SW_SHOW);
                let _ = ShowWindow(main, SW_RESTORE);
                let _ = SetForegroundWindow(main);
                let _ = BringWindowToTop(main);
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => ctlcolor_same_as_window(w),
        WM_DESTROY => {
            if !SWITCHING.load(Ordering::SeqCst) { PostQuitMessage(0); }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}
unsafe extern "system" fn main_proc(hwnd: HWND, msg: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    match msg {
        WM_CREATE => { init_main_ui(hwnd); LRESULT(0) }
        WM_COMMAND => {
            let id = (w.0 as u32 & 0xFFFF) as i32;
            let code = ((w.0 as u32 >> 16) & 0xFFFF) as u16;
            if code == 0 { handle_main_command(hwnd, id); }
            LRESULT(0)
        }
        WM_NOTIFY => {
            let hdr = *(l.0 as *const NMHDR);
            if hdr.idFrom == IDC_MAIN_TAB as usize && hdr.code == TCN_SELCHANGE as u32 {
                let idx = SendMessageW(ctrl(hwnd, IDC_MAIN_TAB), TCM_GETCURSEL, WPARAM(0), LPARAM(0)).0 as i32;
                show_tab(hwnd, idx);
            }
            LRESULT(0)
        }
        WM_TIMER => { refresh_main(hwnd); LRESULT(0) }
        WM_CTLCOLORSTATIC => ctlcolor_same_as_window(w),
        WM_CLOSE => { let _ = DestroyWindow(hwnd); LRESULT(0) }
        WM_DESTROY => { PostQuitMessage(0); LRESULT(0) }
        _ => DefWindowProcW(hwnd, msg, w, l),
    }
}

unsafe fn create_main() -> HWND {
    let width = 940;
    let height = 740;
    let sw = GetSystemMetrics(SM_CXSCREEN);
    let sh = GetSystemMetrics(SM_CYSCREEN);
    let x = ((sw - width) / 2).max(0);
    let y = ((sh - height) / 2).max(0);
    CreateWindowExW(
        WINDOW_EX_STYLE(0),
        MAIN_CLASS,
        w!("WhereIsIt 备份恢复"),
        WINDOW_STYLE(WS_OVERLAPPEDWINDOW.0),
        x,
        y,
        width,
        height,
        HWND(0),
        HMENU(0),
        HINSTANCE(0),
        None,
    )
}

unsafe fn create_login() -> HWND {
    let width = 640;
    let height = 460;
    let sw = GetSystemMetrics(SM_CXSCREEN);
    let sh = GetSystemMetrics(SM_CYSCREEN);
    let x = ((sw - width) / 2).max(0);
    let y = ((sh - height) / 2).max(0);
    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE(0),
        LOGIN_CLASS,
        w!("WhereIsIt 登录"),
        WINDOW_STYLE(WS_OVERLAPPED.0 | WS_CAPTION.0 | WS_SYSMENU.0 | WS_VISIBLE.0),
        x,
        y,
        width,
        height,
        HWND(0),
        HMENU(0),
        HINSTANCE(0),
        None,
    );
    let _ = ShowWindow(hwnd, SW_SHOW);
    hwnd
}

unsafe fn register_classes() -> Result<(), String> {
    let cursor = LoadCursorW(None, IDC_ARROW).map_err(|e| e.to_string())?;
    let bg = HBRUSH(GetStockObject(WHITE_BRUSH).0);
    let login_cls = WNDCLASSW { style: CS_HREDRAW | CS_VREDRAW, lpfnWndProc: Some(login_proc), hCursor: cursor, hbrBackground: bg, lpszClassName: LOGIN_CLASS, ..Default::default() };
    let main_cls = WNDCLASSW { style: CS_HREDRAW | CS_VREDRAW, lpfnWndProc: Some(main_proc), hCursor: cursor, hbrBackground: bg, lpszClassName: MAIN_CLASS, ..Default::default() };
    if RegisterClassW(&login_cls) == 0 { return Err("register login class failed".into()); }
    if RegisterClassW(&main_cls) == 0 { return Err("register main class failed".into()); }
    Ok(())
}

unsafe fn run_loop() {
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, HWND(0), 0, 0).into() {
        TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }
}

fn init_model() -> Arc<Mutex<AppModel>> {
    let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("."));
    let base_dir = exe.parent().unwrap_or_else(|| Path::new(".")).to_path_buf();

    let cfg_path = base_dir.join("config.json");
    let cfg_example = base_dir.join("config.json.example");
    if !cfg_path.exists() && cfg_example.exists() { let _ = fs::copy(&cfg_example, &cfg_path); }

    let cfg = load_config(&cfg_path);

    Arc::new(Mutex::new(AppModel {
        config_path: cfg_path,
        config: cfg.clone(),
        status: "就绪".into(),
        login: LoginState { connection_status: "未连接".into(), error_message: String::new(), login_success: false },
        backup: DbBackupState { backup_format: "custom".into(), dir: cfg.default_backup_root.clone(), task_id: String::new(), status: "空闲".into(), progress: 0.0, log: String::new(), legacy: false },
        restore: DbRestoreState { file: String::new(), mode: "drop_and_restore".into(), task_id: String::new(), status: "空闲".into(), progress: 0.0, log: String::new() },
        uploads_backup: UploadBackupState { dir: PathBuf::from(&cfg.default_backup_root).join("uploads").to_string_lossy().to_string(), status: "空闲".into(), progress: 0.0, log: String::new() },
        uploads_restore: UploadRestoreState {
            dir: PathBuf::from(&cfg.default_backup_root).join("uploads").to_string_lossy().to_string(),
            overwrite_mode: "skip_if_exists".into(),
            task_id: String::new(),
            scanned_file_count: 0,
            scanned_total_bytes: 0,
            scanned_files: Vec::new(),
            completed_count: 0,
            skipped_count: 0,
            overwritten_count: 0,
            failed_count: 0,
            current_file: String::new(),
            status: "空闲".into(),
            progress: 0.0,
            log: String::new(),
        },
    }))
}

fn main() {
    APP.set(init_model()).ok();
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let icc = INITCOMMONCONTROLSEX { dwSize: std::mem::size_of::<INITCOMMONCONTROLSEX>() as u32, dwICC: ICC_TAB_CLASSES | ICC_PROGRESS_CLASS };
        let _ = InitCommonControlsEx(&icc);

        if let Err(e) = register_classes() {
            let text = to_wide(&e);
            let _ = MessageBoxW(HWND(0), PCWSTR(text.as_ptr()), w!("WhereIsIt 备份恢复"), MB_OK | MB_ICONERROR);
            CoUninitialize();
            return;
        }

        let _ = create_login();
        run_loop();
        CoUninitialize();
    }
}


