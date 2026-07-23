//! WP16/UIO-6 — platform boundary for Steamworks and reviewed references.
//!
//! Body-reference commands carry catalog ids rather than URLs. This module
//! alone resolves validated catalog targets and invokes the host browser or
//! clipboard; deterministic headless/golden/replay paths install the no-op
//! adapter. With `steam`, this remains the only module that calls Steamworks.

#[cfg(feature = "steam")]
use crate::steam_app_id::STEAM_APP_ID;
use crate::{
    advance_simulation_frame, record_architecture_plugin, settings::PlatformRuntimePlugin,
    smoke_exit, LoadedCatalog, SimCommand, SimulationSet,
};
use bevy::app::AppExit;
use bevy::prelude::*;
use sim_core::catalog::is_valid_wikipedia_url;
use std::io::Write;
use std::process::{Command as ProcessCommand, Stdio};
use std::sync::Mutex;

/// The complete platform surface visible to application code.
///
/// Keeping this deliberately small prevents Steam-specific types from leaking
/// into simulation, UI, or shutdown logic.
pub trait PlatformServices: Send + Sync + 'static {
    /// Pump platform callbacks and refresh capabilities.
    fn update(&mut self) {}

    /// Whether the platform reports that its in-game overlay can be used.
    fn overlay_available(&self) -> bool;

    /// Open one already-validated external URL.
    fn open_url(&mut self, _url: &str) -> Result<(), String> {
        Ok(())
    }

    /// Copy one already-validated external URL to the system clipboard.
    fn copy_text(&mut self, _text: &str) -> Result<(), String> {
        Ok(())
    }

    /// Release platform resources before the process exits.
    fn shutdown(&mut self);
}

/// Default platform implementation for non-Steam builds.
#[derive(Debug, Default)]
pub struct NoopPlatformServices;

impl PlatformServices for NoopPlatformServices {
    fn overlay_available(&self) -> bool {
        false
    }

    fn shutdown(&mut self) {}
}

/// Dependency-free host integration for ordinary non-golden desktop runs.
#[derive(Debug, Default)]
pub struct NativePlatformServices;

impl PlatformServices for NativePlatformServices {
    fn overlay_available(&self) -> bool {
        false
    }

    fn open_url(&mut self, url: &str) -> Result<(), String> {
        open_url_with_host(url)
    }

    fn copy_text(&mut self, text: &str) -> Result<(), String> {
        copy_text_with_host(text)
    }

    fn shutdown(&mut self) {}
}

#[cfg(target_os = "macos")]
fn open_url_with_host(url: &str) -> Result<(), String> {
    command_succeeded(ProcessCommand::new("/usr/bin/open").arg(url), "open")
}

#[cfg(target_os = "windows")]
fn open_url_with_host(url: &str) -> Result<(), String> {
    command_succeeded(
        ProcessCommand::new("cmd").args(["/C", "start", "", url]),
        "cmd /C start",
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn open_url_with_host(url: &str) -> Result<(), String> {
    command_succeeded(ProcessCommand::new("xdg-open").arg(url), "xdg-open")
}

fn command_succeeded(command: &mut ProcessCommand, label: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("{label} could not start: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label} exited with {status}"))
    }
}

#[cfg(target_os = "macos")]
fn copy_text_with_host(text: &str) -> Result<(), String> {
    write_child_stdin(&mut ProcessCommand::new("/usr/bin/pbcopy"), text, "pbcopy")
}

#[cfg(target_os = "windows")]
fn copy_text_with_host(text: &str) -> Result<(), String> {
    write_child_stdin(
        ProcessCommand::new("cmd").args(["/C", "clip"]),
        text,
        "clip",
    )
}

#[cfg(all(unix, not(target_os = "macos")))]
fn copy_text_with_host(text: &str) -> Result<(), String> {
    let mut wl_copy = ProcessCommand::new("wl-copy");
    match write_child_stdin(&mut wl_copy, text, "wl-copy") {
        Ok(()) => Ok(()),
        Err(wl_error) => {
            let mut xclip = ProcessCommand::new("xclip");
            xclip.args(["-selection", "clipboard"]);
            write_child_stdin(&mut xclip, text, "xclip")
                .map_err(|x_error| format!("{wl_error}; fallback failed: {x_error}"))
        }
    }
}

fn write_child_stdin(command: &mut ProcessCommand, text: &str, label: &str) -> Result<(), String> {
    let mut child = command
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|error| format!("{label} could not start: {error}"))?;
    child
        .stdin
        .take()
        .ok_or_else(|| format!("{label} did not expose stdin"))?
        .write_all(text.as_bytes())
        .map_err(|error| format!("{label} could not receive text: {error}"))?;
    let status = child
        .wait()
        .map_err(|error| format!("{label} could not be awaited: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("{label} exited with {status}"))
    }
}

/// Steamworks-backed platform adapter installed only by `SteamPlugin`.
#[cfg(feature = "steam")]
pub struct SteamPlatformServices {
    client: Option<steamworks::Client>,
}

#[cfg(feature = "steam")]
impl SteamPlatformServices {
    fn initialize() -> steamworks::SIResult<Self> {
        steamworks::Client::init_app(STEAM_APP_ID).map(|client| Self {
            client: Some(client),
        })
    }
}

#[cfg(feature = "steam")]
impl PlatformServices for SteamPlatformServices {
    fn update(&mut self) {
        if let Some(client) = &self.client {
            client.run_callbacks();
        }
    }

    fn overlay_available(&self) -> bool {
        self.client
            .as_ref()
            .is_some_and(|client| client.utils().is_overlay_enabled())
    }

    fn open_url(&mut self, url: &str) -> Result<(), String> {
        open_url_with_host(url)
    }

    fn copy_text(&mut self, text: &str) -> Result<(), String> {
        copy_text_with_host(text)
    }

    fn shutdown(&mut self) {
        // Dropping the final Client calls SteamAPI_Shutdown. Keeping the take
        // here makes repeated Bevy exit messages harmless.
        drop(self.client.take());
    }
}

/// Snapshot exposed to app logic without exposing the platform implementation.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlatformStatus {
    pub overlay_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum BodyReferenceRequest {
    Open(String),
    Copy(String),
}

/// Side-effect requests emitted only after semantic commands have been
/// recorded. Raw URLs are intentionally not representable here.
#[derive(Resource, Debug, Default)]
pub(crate) struct BodyReferenceRequestQueue(Vec<BodyReferenceRequest>);

impl BodyReferenceRequestQueue {
    pub(crate) fn enqueue_command(&mut self, command: &SimCommand) {
        match command {
            SimCommand::OpenBodyReference(body_id) => {
                self.0.push(BodyReferenceRequest::Open(body_id.clone()));
            }
            SimCommand::CopyBodyReference(body_id) => {
                self.0.push(BodyReferenceRequest::Copy(body_id.clone()));
            }
            _ => {}
        }
    }
}

/// UI-facing outcome from the platform adapter. Failures retain the exact
/// validated URL so the toast can offer a deterministic copy fallback.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub(crate) enum BodyReferenceNotice {
    OpenFailed {
        body_id: String,
        body_index: usize,
        body_name: String,
        url: String,
        error: String,
    },
    Copied {
        body_name: String,
        url: String,
    },
    CopyFailed {
        body_id: String,
        body_index: usize,
        body_name: String,
        url: String,
        error: String,
    },
}

#[derive(Resource)]
struct PlatformServicesState {
    services: Box<dyn PlatformServices>,
    shut_down: bool,
}

impl PlatformServicesState {
    fn new(services: Box<dyn PlatformServices>) -> (Self, PlatformStatus) {
        let status = PlatformStatus {
            overlay_available: services.overlay_available(),
        };
        (
            Self {
                services,
                shut_down: false,
            },
            status,
        )
    }

    fn shutdown_once(&mut self) {
        if !self.shut_down {
            self.services.shutdown();
            self.shut_down = true;
        }
    }
}

impl Drop for PlatformServicesState {
    fn drop(&mut self) {
        self.shutdown_once();
    }
}

/// Installs one platform implementation and owns its exit lifecycle.
pub struct PlatformServicesPlugin {
    services: Mutex<Option<Box<dyn PlatformServices>>>,
}

impl PlatformServicesPlugin {
    pub fn new(services: impl PlatformServices) -> Self {
        Self {
            services: Mutex::new(Some(Box::new(services))),
        }
    }

    pub fn native() -> Self {
        Self::new(NativePlatformServices)
    }
}

impl Default for PlatformServicesPlugin {
    fn default() -> Self {
        Self::new(NoopPlatformServices)
    }
}

impl Plugin for PlatformServicesPlugin {
    fn build(&self, app: &mut App) {
        let mut services = match self.services.lock() {
            Ok(services) => services,
            Err(poisoned) => poisoned.into_inner(),
        };
        let services = services
            .take()
            .unwrap_or_else(|| Box::new(NoopPlatformServices));
        let (services, status) = PlatformServicesState::new(services);
        if !app
            .world()
            .contains_resource::<Messages<BodyReferenceNotice>>()
        {
            app.add_message::<BodyReferenceNotice>();
        }
        app.insert_resource(services)
            .insert_resource(status)
            .init_resource::<BodyReferenceRequestQueue>()
            .add_systems(First, update_platform_services)
            .add_systems(
                Update,
                process_body_reference_requests
                    .after(SimulationSet::Commands)
                    .before(SimulationSet::Clock),
            )
            .add_systems(Last, shutdown_platform_services);
    }
}

/// Architecture-facing owner of window/runtime policy, renderer recovery,
/// and application frame/exit lifecycle.
pub struct PlatformPlugin;

impl Plugin for PlatformPlugin {
    fn build(&self, app: &mut App) {
        record_architecture_plugin(app, "PlatformPlugin");
        app.add_plugins(PlatformRuntimePlugin).add_systems(
            Update,
            (advance_simulation_frame, smoke_exit)
                .chain()
                .in_set(SimulationSet::Render),
        );
    }
}

/// Initializes Steamworks for the committed App ID and owns its lifecycle.
///
/// A missing Steam client disables platform services but never prevents the
/// simulator from launching; overlay availability is an optional capability.
#[cfg(feature = "steam")]
pub struct SteamPlugin;

#[cfg(feature = "steam")]
impl Plugin for SteamPlugin {
    fn build(&self, app: &mut App) {
        record_architecture_plugin(app, "SteamPlugin");
        match SteamPlatformServices::initialize() {
            Ok(services) => {
                let overlay_available = services.overlay_available();
                println!(
                    "steam: initialized app_id={STEAM_APP_ID} overlay_available={overlay_available}"
                );
                app.add_plugins(PlatformServicesPlugin::new(services));
            }
            Err(error) => {
                eprintln!(
                    "steam: initialization failed app_id={STEAM_APP_ID}; continuing with overlay unavailable: {error}"
                );
                app.add_plugins(PlatformServicesPlugin::native());
            }
        }
    }
}

fn process_body_reference_requests(
    mut requests: ResMut<BodyReferenceRequestQueue>,
    loaded: Option<Res<LoadedCatalog>>,
    mut platform: ResMut<PlatformServicesState>,
    mut notices: MessageWriter<BodyReferenceNotice>,
) {
    let pending = std::mem::take(&mut requests.0);
    let Some(loaded) = loaded else {
        return;
    };
    for request in pending {
        let (body_id, copy) = match request {
            BodyReferenceRequest::Open(body_id) => (body_id, false),
            BodyReferenceRequest::Copy(body_id) => (body_id, true),
        };
        let Some(body_index) = loaded.index_of(&body_id) else {
            continue;
        };
        let Some(body) = loaded.catalog.bodies.get(body_index) else {
            continue;
        };
        let Some(url) = body.wikipedia_url.as_deref() else {
            continue;
        };
        // Catalog loading already validates this, but repeat the exact guard
        // at the side-effect boundary to prevent future call-site drift.
        if !is_valid_wikipedia_url(url) {
            continue;
        }
        if copy {
            match platform.services.copy_text(url) {
                Ok(()) => {
                    notices.write(BodyReferenceNotice::Copied {
                        body_name: body.name.clone(),
                        url: url.to_string(),
                    });
                }
                Err(error) => {
                    notices.write(BodyReferenceNotice::CopyFailed {
                        body_id,
                        body_index,
                        body_name: body.name.clone(),
                        url: url.to_string(),
                        error,
                    });
                }
            }
        } else if let Err(error) = platform.services.open_url(url) {
            notices.write(BodyReferenceNotice::OpenFailed {
                body_id,
                body_index,
                body_name: body.name.clone(),
                url: url.to_string(),
                error,
            });
        }
    }
}

fn update_platform_services(
    mut platform: ResMut<PlatformServicesState>,
    mut status: ResMut<PlatformStatus>,
) {
    if platform.shut_down {
        return;
    }
    platform.services.update();
    let overlay_available = platform.services.overlay_available();
    if status.overlay_available != overlay_available {
        status.overlay_available = overlay_available;
        println!("platform: overlay_available={overlay_available}");
    }
}

fn shutdown_platform_services(
    mut exits: MessageReader<AppExit>,
    mut platform: ResMut<PlatformServicesState>,
) {
    if exits.read().next().is_some() {
        platform.shutdown_once();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::load_catalog_text;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    struct MockPlatformServices {
        overlay_available: bool,
        updates: Arc<AtomicUsize>,
        shutdowns: Arc<AtomicUsize>,
    }

    impl PlatformServices for MockPlatformServices {
        fn update(&mut self) {
            self.updates.fetch_add(1, Ordering::SeqCst);
        }

        fn overlay_available(&self) -> bool {
            self.overlay_available
        }

        fn shutdown(&mut self) {
            self.shutdowns.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn app_lifecycle_uses_the_mock_and_does_not_require_an_overlay() {
        let updates = Arc::new(AtomicUsize::new(0));
        let shutdowns = Arc::new(AtomicUsize::new(0));
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(PlatformServicesPlugin::new(MockPlatformServices {
                overlay_available: false,
                updates: updates.clone(),
                shutdowns: shutdowns.clone(),
            }));

        assert_eq!(
            *app.world().resource::<PlatformStatus>(),
            PlatformStatus {
                overlay_available: false,
            }
        );

        app.world_mut().write_message(AppExit::Success);
        app.update();
        app.update();
        assert_eq!(updates.load(Ordering::SeqCst), 1);
        assert_eq!(shutdowns.load(Ordering::SeqCst), 1);

        drop(app);
        assert_eq!(shutdowns.load(Ordering::SeqCst), 1);
    }

    struct DelayedOverlayServices {
        overlay_available: bool,
    }

    impl PlatformServices for DelayedOverlayServices {
        fn update(&mut self) {
            self.overlay_available = true;
        }

        fn overlay_available(&self) -> bool {
            self.overlay_available
        }

        fn shutdown(&mut self) {}
    }

    #[test]
    fn delayed_overlay_status_refreshes_after_platform_update() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(PlatformServicesPlugin::new(DelayedOverlayServices {
                overlay_available: false,
            }));

        assert!(!app.world().resource::<PlatformStatus>().overlay_available);
        app.update();
        assert!(app.world().resource::<PlatformStatus>().overlay_available);
    }

    struct ReferenceMockServices {
        opened: Arc<Mutex<Vec<String>>>,
        copied: Arc<Mutex<Vec<String>>>,
        fail_open: bool,
    }

    impl PlatformServices for ReferenceMockServices {
        fn overlay_available(&self) -> bool {
            false
        }

        fn open_url(&mut self, url: &str) -> Result<(), String> {
            self.opened.lock().unwrap().push(url.to_string());
            if self.fail_open {
                Err("synthetic opener failure".into())
            } else {
                Ok(())
            }
        }

        fn copy_text(&mut self, text: &str) -> Result<(), String> {
            self.copied.lock().unwrap().push(text.to_string());
            Ok(())
        }

        fn shutdown(&mut self) {}
    }

    #[test]
    fn body_reference_commands_resolve_only_validated_catalog_urls_and_fallback_to_copy() {
        let opened = Arc::new(Mutex::new(Vec::new()));
        let copied = Arc::new(Mutex::new(Vec::new()));
        let catalog =
            load_catalog_text(include_str!("../../../assets/catalog.ron")).expect("catalog");
        let earth_index = catalog
            .bodies
            .iter()
            .position(|body| body.id == "earth")
            .unwrap();
        let earth_url = catalog.bodies[earth_index].wikipedia_url.clone().unwrap();

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(PlatformServicesPlugin::new(ReferenceMockServices {
                opened: opened.clone(),
                copied: copied.clone(),
                fail_open: true,
            }))
            .insert_resource(LoadedCatalog::new(catalog));
        {
            let mut queue = app.world_mut().resource_mut::<BodyReferenceRequestQueue>();
            queue.enqueue_command(&SimCommand::OpenBodyReference("earth".into()));
            queue.enqueue_command(&SimCommand::OpenBodyReference("unknown_body".into()));
        }
        app.update();

        assert_eq!(
            opened.lock().unwrap().as_slice(),
            std::slice::from_ref(&earth_url)
        );
        let notices: Vec<_> = app
            .world()
            .resource::<Messages<BodyReferenceNotice>>()
            .iter_current_update_messages()
            .cloned()
            .collect();
        assert_eq!(
            notices,
            [BodyReferenceNotice::OpenFailed {
                body_id: "earth".into(),
                body_index: earth_index,
                body_name: "Earth".into(),
                url: earth_url.clone(),
                error: "synthetic opener failure".into(),
            }]
        );
        app.world_mut()
            .resource_mut::<Messages<BodyReferenceNotice>>()
            .clear();

        app.world_mut()
            .resource_mut::<BodyReferenceRequestQueue>()
            .enqueue_command(&SimCommand::CopyBodyReference("earth".into()));
        app.update();
        assert_eq!(
            copied.lock().unwrap().as_slice(),
            std::slice::from_ref(&earth_url)
        );
        let notices: Vec<_> = app
            .world()
            .resource::<Messages<BodyReferenceNotice>>()
            .iter_current_update_messages()
            .cloned()
            .collect();
        assert_eq!(
            notices,
            [BodyReferenceNotice::Copied {
                body_name: "Earth".into(),
                url: earth_url,
            }]
        );
    }

    #[cfg(feature = "steam")]
    #[test]
    fn steam_adapter_is_confined_to_the_platform_services_contract() {
        fn assert_platform_services<T: PlatformServices>() {}

        assert_platform_services::<SteamPlatformServices>();
    }
}
