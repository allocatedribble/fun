#![forbid(unsafe_code)]

mod derive_fun_from_template;
mod derive_fun_scene_component;
mod fun_list_macro;
mod fun_macro;
mod parser;

use proc_macro::{Delimiter, Group, TokenStream, TokenTree};
use std::str::FromStr;

#[proc_macro]
pub fn fun(input: TokenStream) -> TokenStream {
    let _ = parser::parser_contract();
    let _ = derive_fun_from_template::derive_name();
    let _ = derive_fun_scene_component::derive_name();
    fun_macro::expand(input)
}

#[proc_macro]
pub fn fun_list(input: TokenStream) -> TokenStream {
    let _ = parser::parser_contract();
    fun_list_macro::expand(input)
}

fn wrap_scene_macro(path: &str, delimiter: Delimiter, input: TokenStream) -> TokenStream {
    let mut output = TokenStream::from_str(path).expect("scene macro path must be valid tokens");
    output.extend([TokenTree::Group(Group::new(delimiter, input))]);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fun_macros_target_bevy_scene_during_transition() {
        assert_eq!(fun_macro::FUN_MACRO_TARGET, "::fun_scene::bevy_scene::bsn!");
        assert_eq!(
            fun_list_macro::FUN_LIST_MACRO_TARGET,
            "::fun_scene::bevy_scene::bsn_list!"
        );
    }

    #[test]
    fn parser_contract_names_fun_scene_features() {
        let contract = parser::parser_contract();

        assert_eq!(contract.authoring_macro, "fun");
        assert_eq!(contract.list_authoring_macro, "fun_list");
        assert_eq!(contract.template_value, "fun_value");
        assert_eq!(contract.children, "children");
        assert_eq!(contract.expressions, "expressions");
        assert!(contract.dynamic_expressions_require_validation);
        assert_eq!(contract.inheritance, "inheritance");
        assert_eq!(contract.named_entities, "named_entities");
        assert_eq!(contract.props, "props");
    }

    #[test]
    fn derive_contract_is_reserved_for_fun_owned_expansion() {
        assert_eq!(derive_fun_from_template::derive_name(), "FunFromTemplate");
        assert_eq!(
            derive_fun_scene_component::derive_name(),
            "FunSceneComponent"
        );
    }
}
