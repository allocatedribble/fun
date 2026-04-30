#[macro_export]
macro_rules! fun_diag_enabled {
    () => {
        cfg!(all(feature = "diagnostics", debug_assertions))
    };
}

#[macro_export]
macro_rules! fun_diag_block {
    ({ $($body:tt)* }) => {{
        #[cfg(all(feature = "diagnostics", debug_assertions))]
        {
            $($body)*
        }
    }};
}

#[macro_export]
macro_rules! fun_diag_block_if {
    ($condition:expr, { $($body:tt)* }) => {{
        #[cfg(all(feature = "diagnostics", debug_assertions))]
        {
            if $condition {
                $($body)*
            }
        }
    }};
}

#[macro_export]
macro_rules! fun_diag_info {
    (target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::info!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::info!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_info_if {
    ($condition:expr, target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::info!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($condition:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::info!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_debug {
    (target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::debug!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::debug!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_debug_if {
    ($condition:expr, target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::debug!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($condition:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::debug!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_trace {
    (target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::trace!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::trace!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_trace_if {
    ($condition:expr, target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::trace!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($condition:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::trace!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_warn {
    (target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::warn!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($($args:tt)+) => {
        $crate::fun_diag_block!({
            ::tracing::warn!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}

#[macro_export]
macro_rules! fun_diag_warn_if {
    ($condition:expr, target: $target:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::warn!(
                target: $target,
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
    ($condition:expr, $($args:tt)+) => {
        $crate::fun_diag_block_if!($condition, {
            ::tracing::warn!(
                diag_file = file!(),
                diag_line = line!(),
                diag_module = module_path!(),
                $($args)+
            );
        });
    };
}
