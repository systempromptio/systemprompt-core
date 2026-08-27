//! Winit application handler driving the GUI event loop.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::WindowId;

use crate::gui::events::UiEvent;
use crate::gui::state::{GatewayStatus, now_unix};
use crate::gui::{GuiApp, PROBE_INTERVAL_SECS, dispatch, emit, first_run, hosts, menu, tray};
use crate::obs::output::diag;

const PROXY_STATS_TICK_SECS: u64 = 1;

impl ApplicationHandler for GuiApp {
    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::Init) {
            return;
        }
        let drained: Vec<UiEvent> = self.tray.as_ref().map(tray::drain).unwrap_or_default();
        for ev in drained {
            dispatch::dispatch(self, event_loop, ev);
        }
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.tray.is_none() {
            let snap = self.state.snapshot();
            match tray::build(&snap) {
                Ok(mut handles) => {
                    if self.menu_bar.is_none() {
                        match menu::install(&mut handles.bindings) {
                            Ok(menu_handles) => self.menu_bar = Some(menu_handles),
                            Err(e) => diag(&format!("gui: menu bar init failed: {e}")),
                        }
                    }
                    self.tray = Some(handles);
                },
                Err(e) => diag(&format!("gui: tray init failed: {e}")),
            }
        }
        self.refresh_ui();
        dispatch::dispatch(self, event_loop, UiEvent::OpenSettings);
        self.proxy
            .send_event(UiEvent::GatewayProbeRequested { reply_to: None });

        hosts::tick::request_initial_probe(self);
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        for event in self.proxy.drain() {
            dispatch::dispatch(self, event_loop, event);
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &dyn ActiveEventLoop,
        id: WindowId,
        event: WindowEvent,
    ) {
        if let WindowEvent::ThemeChanged(theme) = &event {
            let label = match theme {
                winit::window::Theme::Light => "light",
                winit::window::Theme::Dark => "dark",
            };
            emit::emit_theme_changed(self, label);
            return;
        }
        if let WindowEvent::SurfaceResized(size) = &event
            && let Some(win) = &self.settings_window
            && win.id() == id
        {
            win.resize_webview(*size);
            return;
        }
        if event == WindowEvent::CloseRequested
            && let Some(win) = &self.settings_window
            && win.id() == id
        {
            win.hide();
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        if let Some(handles) = &self.tray {
            for ev in tray::drain(handles) {
                self.proxy.send_event(ev);
            }
        }
        let snap = self.state.snapshot();
        let needs_probe = matches!(snap.gateway_status, GatewayStatus::Unknown)
            || snap
                .last_probe_at_unix
                .is_none_or(|t| now_unix().saturating_sub(t) >= PROBE_INTERVAL_SECS);
        if needs_probe && !matches!(snap.gateway_status, GatewayStatus::Probing) {
            self.proxy
                .send_event(UiEvent::GatewayProbeRequested { reply_to: None });
        }

        if !self.did_initial_sync && snap.signed_in() && snap.pat_present && !snap.sync_in_flight {
            self.did_initial_sync = true;
            self.append_log("auto-sync on startup (rehydrating managed MCP registry)…");
            self.proxy
                .send_event(UiEvent::SyncRequested { reply_to: None });
        }

        hosts::tick::maybe_probe(self);
        first_run::tick(self);

        if self.last_proxy_stats_tick.elapsed() >= Duration::from_secs(PROXY_STATS_TICK_SECS) {
            self.last_proxy_stats_tick = Instant::now();
            self.proxy.send_event(UiEvent::ProxyStatsTick);
        }

        event_loop.set_control_flow(ControlFlow::wait_duration(Duration::from_secs(1)));
    }
}
