#![allow(clippy::transmute_ptr_to_ptr)]

use std::os::raw::{c_int, c_ulong};
use std::rc::Rc;

use cef::*;

use super::context_menu::{MenuAnswer, MenuRequest, read_model, trim_model};
use super::cursor::css_name;
use super::download::{DownloadControl, DownloadProgress};
use super::file_dialog::{FileAnswer, FileRequest};
use super::host::{PageHost, PaintLayer, PixelFrame, PopupArea};
use super::page_message::{CHANNEL, PageMessage};
use crate::config::HOME_HOST;
use crate::navigation::{Destination, NavigationPolicy};

const DEFAULT_COMPANION_SIZE: (i32, i32) = (720, 640);

fn policy() -> NavigationPolicy {
    NavigationPolicy::new(HOME_HOST)
}

fn text(value: Option<&CefString>) -> String {
    value.map_or_default(ToString::to_string)
}

fn is_trusted(origin: &str) -> bool {
    policy().classify(origin) == Destination::Inside
}

fn opens_inside(address: &str) -> bool {
    address.is_empty() || address == "about:blank" || is_trusted(address)
}

wrap_client! {
    pub struct PageClient {
        host: Rc<dyn PageHost>,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(PageRenderer::new(Rc::clone(&self.host)))
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(PageDisplay::new(Rc::clone(&self.host)))
        }

        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(PageLifeSpan::new(Rc::clone(&self.host)))
        }

        fn request_handler(&self) -> Option<RequestHandler> {
            Some(PageRequests::new(Rc::clone(&self.host)))
        }

        fn permission_handler(&self) -> Option<PermissionHandler> {
            Some(PagePermissions::new(Rc::clone(&self.host)))
        }

        fn download_handler(&self) -> Option<DownloadHandler> {
            Some(PageDownloads::new(Rc::clone(&self.host)))
        }

        fn context_menu_handler(&self) -> Option<ContextMenuHandler> {
            Some(PageMenus::new(Rc::clone(&self.host)))
        }

        fn dialog_handler(&self) -> Option<DialogHandler> {
            Some(PageDialogs::new(Rc::clone(&self.host)))
        }

        fn load_handler(&self) -> Option<LoadHandler> {
            Some(PageLoading::new(Rc::clone(&self.host)))
        }

        fn on_process_message_received(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> c_int {
            let Some(message) = message else {
                return 0;
            };
            if CefString::from(&message.name()).to_string() != CHANNEL {
                return 0;
            }
            let payload = message
                .argument_list()
                .map_or_default(|arguments| CefString::from(&arguments.string(0)).to_string());
            if let Some(parsed) = PageMessage::parse(&payload) { self.host.page_message(parsed) } else { tracing::debug!("ignored malformed page message") }
            1
        }
    }
}

wrap_render_handler! {
    struct PageRenderer {
        host: Rc<dyn PageHost>,
    }

    impl RenderHandler {
        fn view_rect(&self, _browser: Option<&mut Browser>, rect: Option<&mut Rect>) {
            if let Some(rect) = rect {
                let viewport = self.host.viewport();
                *rect = Rect {
                    x: 0,
                    y: 0,
                    width: viewport.width.max(1),
                    height: viewport.height.max(1),
                };
            }
        }

        fn screen_info(
            &self,
            _browser: Option<&mut Browser>,
            screen_info: Option<&mut ScreenInfo>,
        ) -> c_int {
            let Some(screen_info) = screen_info else {
                return 0;
            };
            #[allow(clippy::cast_possible_truncation)]
            let scale = self.host.viewport().scale as f32;
            screen_info.device_scale_factor = scale;
            1
        }

        fn on_paint(
            &self,
            _browser: Option<&mut Browser>,
            type_: PaintElementType,
            _dirty_rects: Option<&[Rect]>,
            buffer: *const u8,
            width: c_int,
            height: c_int,
        ) {
            let (Ok(columns), Ok(rows)) = (usize::try_from(width), usize::try_from(height)) else {
                return;
            };
            if buffer.is_null() || columns == 0 || rows == 0 {
                return;
            }
            let pixels = unsafe { std::slice::from_raw_parts(buffer, columns * rows * 4) };
            let layer = if type_ == PaintElementType::POPUP {
                PaintLayer::Popup
            } else {
                PaintLayer::Page
            };
            self.host.paint(layer, &PixelFrame { pixels, width, height });
        }

        fn on_popup_show(&self, _browser: Option<&mut Browser>, show: c_int) {
            if show == 0 {
                self.host.show_popup(None);
            }
        }

        fn on_popup_size(&self, _browser: Option<&mut Browser>, rect: Option<&Rect>) {
            if let Some(rect) = rect {
                self.host.show_popup(Some(PopupArea {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                }));
            }
        }

        fn on_ime_composition_range_changed(
            &self,
            _browser: Option<&mut Browser>,
            _selected_range: Option<&Range>,
            character_bounds: Option<&[Rect]>,
        ) {
            if let Some(bound) = character_bounds.and_then(<[Rect]>::last) {
                self.host.set_caret(PopupArea {
                    x: bound.x,
                    y: bound.y,
                    width: bound.width,
                    height: bound.height,
                });
            }
        }
    }
}

wrap_display_handler! {
    struct PageDisplay {
        host: Rc<dyn PageHost>,
    }

    impl DisplayHandler {
        fn on_title_change(&self, _browser: Option<&mut Browser>, title: Option<&CefString>) {
            self.host.title_changed(&text(title));
        }

        fn on_cursor_change(
            &self,
            _browser: Option<&mut Browser>,
            _cursor: c_ulong,
            type_: CursorType,
            _custom_cursor_info: Option<&CursorInfo>,
        ) -> c_int {
            self.host.set_cursor(css_name(type_));
            1
        }

        fn on_console_message(
            &self,
            _browser: Option<&mut Browser>,
            _level: LogSeverity,
            message: Option<&CefString>,
            _source: Option<&CefString>,
            _line: c_int,
        ) -> c_int {
            tracing::trace!(target: "wazzup::page", "{}", text(message));
            0
        }
    }
}

wrap_life_span_handler! {
    struct PageLifeSpan {
        host: Rc<dyn PageHost>,
    }

    impl LifeSpanHandler {
        fn on_before_popup(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _popup_id: c_int,
            target_url: Option<&CefString>,
            _target_frame_name: Option<&CefString>,
            target_disposition: WindowOpenDisposition,
            _user_gesture: c_int,
            popup_features: Option<&PopupFeatures>,
            window_info: Option<&mut WindowInfo>,
            client: Option<&mut Option<Client>>,
            _settings: Option<&mut BrowserSettings>,
            _extra_info: Option<&mut Option<DictionaryValue>>,
            _no_javascript_access: Option<&mut c_int>,
        ) -> c_int {
            let address = text(target_url);
            tracing::debug!(%address, disposition = ?target_disposition, "popup requested");
            if !opens_inside(&address) {
                self.host.open_externally(&address);
                return 1;
            }
            let (width, height) = popup_features.map_or(DEFAULT_COMPANION_SIZE, |features| {
                (
                    if features.width_set == 1 { features.width } else { DEFAULT_COMPANION_SIZE.0 },
                    if features.height_set == 1 { features.height } else { DEFAULT_COMPANION_SIZE.1 },
                )
            });
            let (Some(window_info), Some(client)) = (window_info, client) else {
                return 1;
            };
            let Some(companion) = self.host.open_companion(width, height) else {
                return 1;
            };
            window_info.windowless_rendering_enabled = 1;
            window_info.runtime_style = RuntimeStyle::ALLOY;
            *client = Some(PageClient::new(companion));
            tracing::info!(%address, "companion window opened");
            0
        }

        fn on_after_created(&self, browser: Option<&mut Browser>) {
            if let Some(browser) = browser {
                self.host.attach(browser.clone());
            }
        }

        fn on_before_close(&self, _browser: Option<&mut Browser>) {
            self.host.detach();
        }
    }
}

wrap_request_handler! {
    struct PageRequests {
        host: Rc<dyn PageHost>,
    }

    impl RequestHandler {
        fn on_before_browse(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            request: Option<&mut Request>,
            user_gesture: c_int,
            _is_redirect: c_int,
        ) -> c_int {
            let address = request
                .map_or_default(|request| CefString::from(&request.url()).to_string());
            let main_frame = frame.is_some_and(|frame| frame.is_main() == 1);
            tracing::debug!(%address, main_frame, user_gesture, "navigation requested");
            match policy().classify(&address) {
                Destination::Inside => 0,
                Destination::Blocked => {
                    tracing::warn!(%address, "navigation blocked");
                    1
                }
                Destination::SystemBrowser if !main_frame => 0,
                Destination::SystemBrowser => {
                    if user_gesture == 1 {
                        self.host.open_externally(&address);
                    } else {
                        tracing::warn!(%address, "unrequested navigation away from WhatsApp blocked");
                    }
                    1
                }
            }
        }

        fn auth_credentials(
            &self,
            _browser: Option<&mut Browser>,
            _origin_url: Option<&CefString>,
            is_proxy: c_int,
            host: Option<&CefString>,
            _port: c_int,
            _realm: Option<&CefString>,
            _scheme: Option<&CefString>,
            callback: Option<&mut AuthCallback>,
        ) -> c_int {
            let Some(callback) = callback.filter(|_| is_proxy == 1) else {
                return 0;
            };
            match self.host.proxy_credentials() {
                Some((username, password)) => {
                    tracing::info!(proxy = %text(host), "answering proxy authentication");
                    callback.cont(
                        Some(&CefString::from(username.as_str())),
                        Some(&CefString::from(password.as_str())),
                    );
                    1
                }
                None => 0,
            }
        }

        fn on_open_urlfrom_tab(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            target_url: Option<&CefString>,
            _target_disposition: WindowOpenDisposition,
            _user_gesture: c_int,
        ) -> c_int {
            tracing::debug!(address = %text(target_url), "tab navigation requested");
            self.host.open_externally(&text(target_url));
            1
        }

        fn on_render_process_terminated(
            &self,
            _browser: Option<&mut Browser>,
            status: TerminationStatus,
            error_code: c_int,
            _error_string: Option<&CefString>,
        ) {
            tracing::error!(?status, error_code, "page process terminated");
            self.host.page_crashed();
        }
    }
}

wrap_permission_handler! {
    struct PagePermissions {
        host: Rc<dyn PageHost>,
    }

    impl PermissionHandler {
        fn on_request_media_access_permission(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut MediaAccessCallback>,
        ) -> c_int {
            let Some(callback) = callback else {
                return 0;
            };
            let origin = text(requesting_origin);
            let granted = if is_trusted(&origin) {
                self.host.media_allowed().restrict_mask(requested_permissions)
            } else {
                0
            };
            if granted == 0 {
                tracing::warn!(%origin, requested_permissions, "media access refused");
                callback.cancel();
            } else {
                tracing::info!(requested_permissions, granted, "media access granted");
                callback.cont(granted);
            }
            1
        }

        fn on_show_permission_prompt(
            &self,
            _browser: Option<&mut Browser>,
            _prompt_id: u64,
            requesting_origin: Option<&CefString>,
            requested_permissions: u32,
            callback: Option<&mut PermissionPromptCallback>,
        ) -> c_int {
            let Some(callback) = callback else {
                return 0;
            };
            let origin = text(requesting_origin);
            let verdict = if is_trusted(&origin) {
                PermissionRequestResult::ACCEPT
            } else {
                PermissionRequestResult::DENY
            };
            tracing::info!(%origin, requested_permissions, ?verdict, "permission prompt answered");
            callback.cont(verdict);
            1
        }
    }
}

wrap_download_handler! {
    struct PageDownloads {
        host: Rc<dyn PageHost>,
    }

    impl DownloadHandler {
        fn can_download(
            &self,
            _browser: Option<&mut Browser>,
            url: Option<&CefString>,
            request_method: Option<&CefString>,
        ) -> c_int {
            tracing::debug!(address = %text(url), method = %text(request_method), "download allowed");
            1
        }

        fn on_before_download(
            &self,
            _browser: Option<&mut Browser>,
            _download_item: Option<&mut DownloadItem>,
            suggested_name: Option<&CefString>,
            callback: Option<&mut BeforeDownloadCallback>,
        ) -> c_int {
            let Some(callback) = callback else {
                return 0;
            };
            tracing::debug!(name = %text(suggested_name), "download requested");
            let Some(target) = self.host.download_target(&text(suggested_name)) else {
                return 0;
            };
            callback.cont(Some(&CefString::from(target.to_string_lossy().as_ref())), 0);
            1
        }

        fn on_download_updated(
            &self,
            _browser: Option<&mut Browser>,
            download_item: Option<&mut DownloadItem>,
            callback: Option<&mut DownloadItemCallback>,
        ) {
            let Some(item) = download_item else {
                return;
            };
            let progress = DownloadProgress::read(item);
            if progress.path.as_os_str().is_empty() {
                return;
            }
            let control = callback.map(|callback| DownloadControl::new(callback.clone()));
            self.host.download_progress(progress, control);
        }
    }
}

wrap_load_handler! {
    struct PageLoading {
        host: Rc<dyn PageHost>,
    }

    impl LoadHandler {
        fn on_load_end(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            _http_status_code: c_int,
        ) {
            if frame.is_some_and(|frame| frame.is_main() == 1) {
                self.host.page_loaded();
            }
        }

        fn on_load_error(
            &self,
            _browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            error_code: Errorcode,
            error_text: Option<&CefString>,
            failed_url: Option<&CefString>,
        ) {
            let main_frame = frame.is_some_and(|frame| frame.is_main() == 1);
            if !main_frame || error_code == Errorcode::ABORTED {
                return;
            }
            let description = text(error_text);
            tracing::warn!(address = %text(failed_url), %description, "page failed to load");
            self.host.load_failed(&description);
        }
    }
}

wrap_dialog_handler! {
    struct PageDialogs {
        host: Rc<dyn PageHost>,
    }

    impl DialogHandler {
        fn on_file_dialog(
            &self,
            _browser: Option<&mut Browser>,
            mode: FileDialogMode,
            title: Option<&CefString>,
            default_file_path: Option<&CefString>,
            accept_filters: Option<&mut CefStringList>,
            _accept_extensions: Option<&mut CefStringList>,
            _accept_descriptions: Option<&mut CefStringList>,
            callback: Option<&mut FileDialogCallback>,
        ) -> c_int {
            let Some(callback) = callback else {
                return 0;
            };
            tracing::debug!(?mode, "file dialog requested");
            let request = FileRequest::new(
                mode,
                title,
                default_file_path,
                accept_filters.as_deref(),
            );
            self.host
                .choose_files(request, FileAnswer::new(callback.clone()));
            1
        }
    }
}

wrap_context_menu_handler! {
    struct PageMenus {
        host: Rc<dyn PageHost>,
    }

    impl ContextMenuHandler {
        fn on_before_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            _params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
        ) {
            if let Some(model) = model {
                trim_model(model);
            }
        }

        fn run_context_menu(
            &self,
            _browser: Option<&mut Browser>,
            _frame: Option<&mut Frame>,
            params: Option<&mut ContextMenuParams>,
            model: Option<&mut MenuModel>,
            callback: Option<&mut RunContextMenuCallback>,
        ) -> c_int {
            let (Some(params), Some(model), Some(callback)) = (params, model, callback) else {
                return 0;
            };
            let entries = read_model(model);
            if entries.is_empty() {
                callback.cancel();
                return 1;
            }
            let request = MenuRequest {
                x: params.xcoord(),
                y: params.ycoord(),
                entries,
            };
            self.host
                .show_context_menu(request, MenuAnswer::new(callback.clone()));
            1
        }
    }
}
