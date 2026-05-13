use proc_macro::{Delimiter, TokenStream};

pub(crate) const FUN_MACRO_TARGET: &str = "::fun_scene::fun_scene_macro_expand!";

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    crate::wrap_scene_macro(FUN_MACRO_TARGET, Delimiter::Brace, input)
}
