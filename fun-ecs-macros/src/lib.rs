#![forbid(unsafe_code)]

use proc_macro::TokenStream;
use quote::quote;
use syn::{DeriveInput, parse_macro_input};

#[proc_macro_derive(Component)]
pub fn derive_component(input: TokenStream) -> TokenStream {
    derive_marker(input, quote!(Component))
}

#[proc_macro_derive(Resource)]
pub fn derive_resource(input: TokenStream) -> TokenStream {
    derive_marker(input, quote!(Resource))
}

#[proc_macro_derive(Bundle)]
pub fn derive_bundle(input: TokenStream) -> TokenStream {
    derive_marker(input, quote!(Bundle))
}

#[proc_macro_derive(Message)]
pub fn derive_message(input: TokenStream) -> TokenStream {
    derive_marker(input, quote!(Message))
}

#[proc_macro_derive(SystemSet)]
pub fn derive_system_set(input: TokenStream) -> TokenStream {
    derive_marker(input, quote!(SystemSet))
}

#[proc_macro_derive(FunFromTemplate)]
pub fn derive_fun_from_template(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        impl #impl_generics ::fun_ecs::template::FromTemplate for #ident #ty_generics #where_clause {}
    }
    .into()
}

fn derive_marker(input: TokenStream, trait_name: proc_macro2::TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let ident = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    quote! {
        impl #impl_generics ::fun_ecs::#trait_name for #ident #ty_generics #where_clause {}
    }
    .into()
}
