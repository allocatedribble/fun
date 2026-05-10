//! Pass-72 runtime owner.
//!
//! Pass 72 keeps the runtime as a thin re-export of the underlying
//! [`fun_native_app::FunNativeApp`] so the adapter does not pull in
//! a second pipeline implementation. Pass 74 will replace this with
//! a host-transport-aware runtime.

/// Stable type alias for the runtime the adapter owns. Pass 72
/// relies on the rvelte-side app shell; later passes may extend or
/// replace this without changing the public adapter surface.
pub type FunNativeUiRuntime<R = fun_native_app::DefaultFixtureResolver> =
    fun_native_app::FunNativeApp<R>;
