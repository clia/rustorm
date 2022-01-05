use crate::util::{
    find_crate_name,
    parse_table_name,
};
use proc_macro2::TokenStream;

pub fn impl_to_table_name(ast: &syn::DeriveInput) -> TokenStream {
    let rustorm = find_crate_name();
    let name = &ast.ident;
    let table_name = parse_table_name(&ast);
    let generics = &ast.generics;

    quote! {
        impl #generics #rustorm::dao::ToTableName for #name #generics {
            fn to_table_name() -> #rustorm::TableName {
                #rustorm::TableName{
                    name: #table_name.to_owned(),
                    schema: None,
                    alias: None,
                }
            }
        }
    }
}
