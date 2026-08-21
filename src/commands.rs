use curia::Logger;

use crate::WebviewRecord;

/// The webview's entry point into the logger.
///
/// A `#[tauri::command]` cannot hang off a struct, so this is one of the
/// framework exceptions to the no-free-functions rule. It holds no logic of its
/// own: the conversion belongs to [`WebviewRecord`].
#[tauri::command]
pub fn log(record: WebviewRecord) {
    Logger::emit(record.into_event());
}
