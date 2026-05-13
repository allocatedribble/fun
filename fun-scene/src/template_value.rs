#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FunTemplateValue<T> {
    pub value: T,
}

#[must_use]
pub const fn fun_value<T>(value: T) -> FunTemplateValue<T> {
    FunTemplateValue { value }
}
