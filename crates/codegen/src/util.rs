use find_crate::{
    find_crate,
    Manifest,
};
use proc_macro2::{
    Span,
    TokenStream,
};
use quote::ToTokens;
use syn::{
    Attribute,
    DeriveInput,
    Ident,
    Lit,
    LitStr,
    Meta,
    MetaNameValue,
    Token,
};

/// Find the name of the `rustorm` dependency.
///
/// Detects if the project using the derive macro has aliased `rustorm` to be something else.
///
/// Returns a [`proc_macro2::TokenStream`] so that it can be used directly inside `quote!`.
///
/// Note that this function doesn't work for the examples in the `examples` directory, because this
/// function ends up resolving into `crate` for accessing the `rustorm`, but that's incorrect. The
/// workaround there was to import the necessary types, so that `crate::dao` refers to
/// `rustorm::dao`, for example.
pub fn find_crate_name() -> TokenStream {
    find_crate(|name| name == "rustorm")
        .map(|package| Ident::new(&package.name, Span::call_site()).into_token_stream())
        .unwrap_or_else(|error| {
            if !matches!(error, find_crate::Error::NotFound) {
                panic!("`rustorm` dependency not found: {}", error);
            }

            let this_crate = Manifest::new()
                .expect("failed to read crate manifest")
                .crate_package()
                .expect("failed to read the name of this crate");

            if this_crate.name == "rustorm" {
                Token![crate](Span::call_site()).into_token_stream()
            } else {
                panic!("`rustorm` dependency not found");
            }
        })
}

/// Find an attribute of the form `#[key = "value"]` for the specified `key`.
///
/// Returns the `value`, if the attribute is found.
///
/// # Panics
///
/// The function will panic if the attribute is found but it does not follow the expected form.
pub fn find_attribute_value(attributes: &[Attribute], key: &str) -> Option<LitStr> {
    attributes
        .iter()
        .find(|attribute| attribute.path.is_ident(key))
        .map(|attribute| {
            match attribute.parse_meta() {
                Ok(Meta::NameValue(MetaNameValue {
                    lit: Lit::Str(value),
                    ..
                })) => value,
                _ => panic!("invalid `{}` attribute", key),
            }
        })
}

/// Discover the table name to use for the [`DeriveInput`].
///
/// This is either the lower case type name or a custom name specified using a
/// `#[table_name = "custom_name"]` attribute.
///
/// # Panics
///
/// If there's an invalid `table_name` attribute.
pub fn parse_table_name(input: &DeriveInput) -> LitStr {
    find_attribute_value(&input.attrs, "table_name")
        .unwrap_or_else(|| LitStr::new(&input.ident.to_string().to_lowercase(), input.ident.span()))
}
