use proc_macro::{Delimiter, TokenStream};

pub(crate) const FUN_LIST_MACRO_TARGET: &str = "::fun_scene::bevy_scene::bsn_list!";

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    crate::wrap_scene_macro(FUN_LIST_MACRO_TARGET, Delimiter::Bracket, input)
}
