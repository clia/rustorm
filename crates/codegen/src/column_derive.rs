use proc_macro2::TokenStream;

pub fn impl_to_column_names(ast: &syn::MacroInput) -> TokenStream {
    let name = &ast.ident;
    let generics = &ast.generics;
    let fields: Vec<(&syn::Ident, &syn::Ty)> = match ast.body {
        syn::Body::Struct(ref data) => {
            match *data {
                syn::VariantData::Struct(ref fields) => {
                    fields
                        .iter()
                        .map(|f| {
                            let ident = f.ident.as_ref().unwrap();
                            let ty = &f.ty;
                            (ident, ty)
                        })
                        .collect::<Vec<_>>()
                }
                _ => panic!("Only struct is supported for #[derive(ToColumnNames)]"),
            }
        }
        syn::Body::Enum(_) => panic!("#[derive(ToColumnNames)] can only be used with structs"),
    };
    let from_fields: Vec<TokenStream> = fields
        .iter()
        .map(|&(field, _ty)| {
            quote! {
                rustorm_dao::ColumnName {
                    name: stringify!(#field).into(),
                    table: Some(stringify!(#name).to_lowercase().into()),
                    alias: None,
                },
            }
        })
        .collect();

    quote! {
        impl #generics rustorm_dao::ToColumnNames for #name #generics {
            fn to_column_names() -> Vec<rustorm_dao::ColumnName> {
                vec![
                    #(#from_fields)*
                ]
            }
        }
    }
}
