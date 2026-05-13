use proc_macro::{Delimiter, TokenStream};

pub(crate) const FUN_LIST_MACRO_TARGET: &str = "::fun_scene::fun_scene_list_macro_expand!";

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    crate::wrap_scene_macro(FUN_LIST_MACRO_TARGET, Delimiter::Bracket, input)
}
