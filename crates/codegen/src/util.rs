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
    Ident,
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
