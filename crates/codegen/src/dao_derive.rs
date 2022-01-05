use crate::util::find_crate_name;
use proc_macro2::TokenStream;

pub fn impl_from_dao(ast: &syn::DeriveInput) -> TokenStream {
    let rustorm = find_crate_name();
    let name = &ast.ident;
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
            panic!("#[derive(FromDao)] can only be used with structs")
        }
    };
    let from_fields: Vec<TokenStream> = fields
        .iter()
        .map(|&(field, _ty)| {
            quote! { #field: dao.get(stringify!(#field)).unwrap(),}
        })
        .collect();

    quote! {
        impl #rustorm::dao::FromDao for  #name {

            fn from_dao(dao: &#rustorm::Dao) -> Self {
                #name {
                    #(#from_fields)*
                }

            }
        }
    }
}

pub fn impl_to_dao(ast: &syn::DeriveInput) -> TokenStream {
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
            panic!("#[derive(ToDao)] can only be used with structs")
        }
    };
    let from_fields: &Vec<TokenStream> = &fields
        .iter()
        .map(|&(field, _ty)| {
            quote! { dao.insert(stringify!(#field), &self.#field);}
        })
        .collect();

    quote! {
        impl #generics #rustorm::dao::ToDao for #name #generics {
            fn to_dao(&self) -> #rustorm::Dao {
                let mut dao = #rustorm::Dao::new();
                #(#from_fields)*
                dao
            }
        }

    }
}
