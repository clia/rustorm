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
    Lit,
    LitStr,
};

pub fn impl_from_dao(ast: &DeriveInput) -> TokenStream {
    let rustorm = find_crate_name();
    let name = &ast.ident;

    let fields = match ast.data {
        Data::Struct(ref data) => data.fields.iter().map(parse_field),
        Data::Enum(_) | Data::Union(_) => {
            panic!("#[derive(ToDao)] can only be used with structs")
        }
    };

    let get_fields = fields.map(|(column_name, field_name)| {
        quote! { #field_name: dao.get(#column_name).unwrap(),}
    });

    quote! {
        impl #rustorm::dao::FromDao for  #name {

            fn from_dao(dao: &#rustorm::Dao) -> Self {
                #name {
                    #(#get_fields)*
                }

            }
        }
    }
}

pub fn impl_to_dao(ast: &DeriveInput) -> TokenStream {
    let rustorm = find_crate_name();
    let name = &ast.ident;
    let generics = &ast.generics;

    let fields = match ast.data {
        Data::Struct(ref data) => data.fields.iter().map(parse_field),
        Data::Enum(_) | Data::Union(_) => {
            panic!("#[derive(ToDao)] can only be used with structs")
        }
    };

    let insert_fields = fields.map(|(column_name, field_name)| {
        quote! { dao.insert(#column_name, &self.#field_name);}
    });

    quote! {
        impl #generics #rustorm::dao::ToDao for #name #generics {
            fn to_dao(&self) -> #rustorm::Dao {
                let mut dao = #rustorm::Dao::new();
                #(#insert_fields)*
                dao
            }
        }

    }
}

fn parse_field(field: &Field) -> (Lit, &Ident) {
    let field_name = field.ident.as_ref().unwrap();
    let column_name = find_attribute_value(&field.attrs, "column_name")
        .unwrap_or_else(|| LitStr::new(&field_name.to_string(), field_name.span()))
        .into();

    (column_name, field_name)
}
