use crate::util::find_crate_name;
use proc_macro2::TokenStream;

pub fn impl_to_column_names(ast: &syn::DeriveInput) -> TokenStream {
    let rustorm = find_crate_name();
    let name = &ast.ident;
    let generics = &ast.generics;
    let fields: Vec<(&syn::Ident, &syn::Type)> = match ast.data {
        syn::Data::Struct(ref data) => {
            data.fields
                .iter()
                .map(|f| {
                    let ident = f.ident.as_ref().unwrap();
                    let ty = &f.ty;
                    (ident, ty)
                })
                .collect::<Vec<_>>()
        }
        syn::Data::Enum(_) | syn::Data::Union(_) => {
            panic!("#[derive(ToColumnNames)] can only be used with structs")
        }
    };
    let from_fields: Vec<TokenStream> = fields
        .iter()
        .map(|&(field, _ty)| {
            quote! {
                #rustorm::ColumnName {
                    name: stringify!(#field).into(),
                    table: Some(stringify!(#name).to_lowercase().into()),
                    alias: None,
                },
            }
        })
        .collect();

    quote! {
        impl #generics #rustorm::dao::ToColumnNames for #name #generics {
            fn to_column_names() -> Vec<#rustorm::ColumnName> {
                vec![
                    #(#from_fields)*
                ]
            }
        }
    }
}
