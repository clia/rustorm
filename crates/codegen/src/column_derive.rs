use crate::util::{
    find_attribute_value,
    find_crate_name,
};
use proc_macro2::TokenStream;
use syn::{
    Data,
    DeriveInput,
    Field,
    Ident,
    LitStr,
};

pub fn impl_to_column_names(ast: &DeriveInput) -> TokenStream {
    let rustorm = find_crate_name();
    let name = &ast.ident;
    let generics = &ast.generics;

    let from_fields = match ast.data {
        Data::Struct(ref data) => {
            data.fields
                .iter()
                .map(|field| generate_from_field(&rustorm, name, field))
        }
        Data::Enum(_) | Data::Union(_) => {
            panic!("#[derive(ToColumnNames)] can only be used with structs")
        }
    };

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

fn generate_from_field(rustorm: &TokenStream, table_name: &Ident, field: &Field) -> TokenStream {
    let field_name = field.ident.as_ref().unwrap();
    let column_name = find_attribute_value(&field.attrs, "column_name")
        .unwrap_or_else(|| LitStr::new(&field_name.to_string(), field_name.span()));

    quote! {
        #rustorm::ColumnName {
            name: #column_name.into(),
            table: Some(stringify!(#table_name).to_lowercase().into()),
            alias: None,
        },
    }
}
