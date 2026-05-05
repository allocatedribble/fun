pub mod children;
pub mod expressions;
pub mod inheritance;
pub mod named_entities;
pub mod props;
pub mod tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ParserContract {
    pub authoring_macro: &'static str,
    pub list_authoring_macro: &'static str,
    pub template_value: &'static str,
    pub children: &'static str,
    pub expressions: &'static str,
    pub dynamic_expressions_require_validation: bool,
    pub inheritance: &'static str,
    pub named_entities: &'static str,
    pub props: &'static str,
}

pub(crate) fn parser_contract() -> ParserContract {
    ParserContract {
        authoring_macro: tokens::AUTHORING_MACRO,
        list_authoring_macro: tokens::LIST_AUTHORING_MACRO,
        template_value: tokens::TEMPLATE_VALUE,
        children: children::CHILDREN_BLOCK,
        expressions: expressions::EXPRESSIONS,
        dynamic_expressions_require_validation: expressions::DYNAMIC_EXPRESSIONS_REQUIRE_VALIDATION,
        inheritance: inheritance::INHERITANCE_MARKER,
        named_entities: named_entities::NAMED_ENTITIES,
        props: props::PROPS_BLOCK,
    }
}
