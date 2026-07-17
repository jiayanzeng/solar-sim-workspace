//! WP16 — platform-service boundary for optional Steamworks integration.
//!
//! The default application owns a no-op implementation so rendering and
//! simulation never depend on an overlay or a running platform client. With
//! the `steam` cargo feature, `SteamPlugin` is the only module allowed to call
//! Steamworks and it falls back to the no-op service when the client is absent.

#[cfg(feature = "steam")]
use crate::steam_app_id::STEAM_APP_ID;
use crate::{
    advance_simulation_frame, record_architecture_plugin, settings::PlatformRuntimePlugin,
    smoke_exit, SimulationSet,
};
use bevy::app::AppExit;
use bevy::prelude::*;
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
        app.insert_resource(services)
            .insert_resource(status)
            .add_systems(First, update_platform_services)
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
                app.add_plugins(PlatformServicesPlugin::default());
            }
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

    #[cfg(feature = "steam")]
    #[test]
    fn steam_adapter_is_confined_to_the_platform_services_contract() {
        fn assert_platform_services<T: PlatformServices>() {}

        assert_platform_services::<SteamPlatformServices>();
    }
}
