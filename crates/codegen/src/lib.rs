#![deny(warnings)]
#![deny(clippy::all)]

extern crate proc_macro;
#[macro_use]
extern crate quote;
extern crate rustorm_dao;
extern crate syn;

#[macro_use]
mod column_derive;
#[macro_use]
mod dao_derive;
#[macro_use]
mod table_derive;
mod util;

use proc_macro::TokenStream;

#[proc_macro_derive(FromDao, attributes(column_name))]
pub fn from_dao(tokens: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(tokens as syn::DeriveInput);

    dao_derive::impl_from_dao(&input).into()
}

#[proc_macro_derive(ToDao, attributes(column_name))]
pub fn to_dao(tokens: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(tokens as syn::DeriveInput);

    dao_derive::impl_to_dao(&input).into()
}

#[proc_macro_derive(ToTableName)]
pub fn to_table_name(tokens: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(tokens as syn::DeriveInput);

    table_derive::impl_to_table_name(&input).into()
}

#[proc_macro_derive(ToColumnNames, attributes(column_name))]
pub fn to_column_names(tokens: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(tokens as syn::DeriveInput);

    column_derive::impl_to_column_names(&input).into()
}
