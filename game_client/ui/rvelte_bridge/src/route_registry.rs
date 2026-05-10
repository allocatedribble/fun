//! Pass-72 native route registry.
//!
//! Maps stable product-side [`ProductRouteKind`] identifiers to the
//! rvelte-side [`fun_native_app::NativeRouteKind`]. The registry is
//! the only place the bridge crate translates between the two
//! identities; every other call site routes through
//! [`ProductRouteRegistry::native_kind_of`] /
//! [`ProductRouteRegistry::product_kind_of`].

use std::collections::BTreeMap;

use fun_native_app::NativeRouteKind;
use serde::{Deserialize, Serialize};

/// Stable identifier for a mountable product route. Variants
/// mirror `fun_native_app::NativeRouteKind` plus a reserved range
/// for product-only routes added under a later pass.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Deserialize, Serialize)]
pub enum ProductRouteKind {
    /// Pass-58 launcher shell.
    LauncherShell,
    /// Pass-59 HUD overlay.
    HudOverlay,
    /// Pass-60 pause menu.
    PauseMenu,
    /// Pass-61 diagnostics list.
    DiagnosticsList,
    /// Pass-62 command bar.
    CommandBar,
    /// Pass-62 settings shell.
    SettingsShell,
    /// Reserved for product-only routes added under a later pass.
    /// The `code` field is the stable u32 identifier; pass 72 only
    /// declares that the variant exists, no codes are assigned.
    ProductReserved {
        /// Stable u32 identifier declared by the product registry.
        code: u32,
    },
}

impl ProductRouteKind {
    /// Stable machine label.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LauncherShell => "launcher_shell",
            Self::HudOverlay => "hud_overlay",
            Self::PauseMenu => "pause_menu",
            Self::DiagnosticsList => "diagnostics_list",
            Self::CommandBar => "command_bar",
            Self::SettingsShell => "settings_shell",
            Self::ProductReserved { .. } => "product_reserved",
        }
    }

    /// Returns a stable u32 code that can be transported through
    /// telemetry without leaking string identity.
    #[must_use]
    pub const fn code(self) -> u32 {
        match self {
            Self::LauncherShell => 1,
            Self::HudOverlay => 2,
            Self::PauseMenu => 3,
            Self::DiagnosticsList => 4,
            Self::CommandBar => 5,
            Self::SettingsShell => 6,
            Self::ProductReserved { code } => code,
        }
    }

    /// Returns every well-known route. Product-reserved variants
    /// are not enumerated here.
    #[must_use]
    pub const fn well_known() -> &'static [Self] {
        &[
            Self::LauncherShell,
            Self::HudOverlay,
            Self::PauseMenu,
            Self::DiagnosticsList,
            Self::CommandBar,
            Self::SettingsShell,
        ]
    }
}

/// Stable lookup table mapping product-side route IDs to the
/// rvelte-side [`NativeRouteKind`]. The registry is a closed enum
/// at construction time; product-only routes that later land are
/// added through a typed builder rather than runtime reflection.
///
/// Pass-72 keeps the registry non-serializable: the rvelte-side
/// `NativeRouteKind` does not yet implement `Serialize` /
/// `Deserialize`, and the design cites no transport boundary that
/// requires the registry to be wire-shaped.
#[derive(Clone, Debug, Default)]
pub struct ProductRouteRegistry {
    forward: BTreeMap<u32, NativeRouteKind>,
    reverse: BTreeMap<&'static str, ProductRouteKind>,
}

impl ProductRouteRegistry {
    /// Creates an empty registry. Use [`Self::with_well_known`] to
    /// pre-load the six routes shipped by passes 58–62.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a registry pre-loaded with the six well-known
    /// product routes.
    #[must_use]
    pub fn with_well_known() -> Self {
        let mut registry = Self::new();
        for route in ProductRouteKind::well_known() {
            // Map every well-known route to its rvelte-side
            // counterpart. The mapping is exhaustive by
            // construction.
            let native = native_for(*route);
            if let Some(native) = native {
                registry.forward.insert(route.code(), native);
                registry.reverse.insert(native.as_str(), *route);
            }
        }
        registry
    }

    /// Returns the rvelte-side `NativeRouteKind` for `route`, when
    /// the registry knows about it.
    #[must_use]
    pub fn native_kind_of(&self, route: ProductRouteKind) -> Option<NativeRouteKind> {
        self.forward.get(&route.code()).copied()
    }

    /// Returns the product-side `ProductRouteKind` for an underlying
    /// rvelte-side `NativeRouteKind`, when the registry knows about
    /// it.
    #[must_use]
    pub fn product_kind_of(&self, native: NativeRouteKind) -> Option<ProductRouteKind> {
        self.reverse.get(native.as_str()).copied()
    }

    /// Returns the number of routes the registry tracks.
    #[must_use]
    pub fn len(&self) -> usize {
        self.forward.len()
    }

    /// Returns true when the registry has no routes.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.forward.is_empty()
    }
}

fn native_for(route: ProductRouteKind) -> Option<NativeRouteKind> {
    match route {
        ProductRouteKind::LauncherShell => Some(NativeRouteKind::LauncherShell),
        ProductRouteKind::HudOverlay => Some(NativeRouteKind::HudOverlay),
        ProductRouteKind::PauseMenu => Some(NativeRouteKind::PauseMenu),
        ProductRouteKind::DiagnosticsList => Some(NativeRouteKind::DiagnosticsList),
        ProductRouteKind::CommandBar => Some(NativeRouteKind::CommandBar),
        ProductRouteKind::SettingsShell => Some(NativeRouteKind::SettingsShell),
        ProductRouteKind::ProductReserved { .. } => None,
    }
}
