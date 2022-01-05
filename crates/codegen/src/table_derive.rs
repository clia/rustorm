use crate::util::find_crate_name;
use proc_macro2::TokenStream;

pub fn impl_to_table_name(ast: &syn::DeriveInput) -> TokenStream {
    let rustorm = find_crate_name();
    let name = &ast.ident;
    let generics = &ast.generics;

    quote! {
        impl #generics #rustorm::dao::ToTableName for #name #generics {
            fn to_table_name() -> #rustorm::TableName {
                #rustorm::TableName{
                    name: stringify!(#name).to_lowercase().into(),
                    schema: None,
                    alias: None,
                }
            }
        }
    }
}
