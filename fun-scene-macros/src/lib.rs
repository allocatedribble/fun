#![forbid(unsafe_code)]

use proc_macro::{Delimiter, Group, TokenStream, TokenTree};
use std::str::FromStr;

#[proc_macro]
pub fn fun(input: TokenStream) -> TokenStream {
    wrap_scene_macro("::fun_scene::bevy_scene::bsn!", Delimiter::Brace, input)
}

#[proc_macro]
pub fn fun_list(input: TokenStream) -> TokenStream {
    wrap_scene_macro(
        "::fun_scene::bevy_scene::bsn_list!",
        Delimiter::Bracket,
        input,
    )
}

fn wrap_scene_macro(path: &str, delimiter: Delimiter, input: TokenStream) -> TokenStream {
    let mut output = TokenStream::from_str(path).expect("scene macro path must be valid tokens");
    output.extend([TokenTree::Group(Group::new(delimiter, input))]);
    output
}
