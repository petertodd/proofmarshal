extern crate proc_macro;

mod commit;
mod verbatim;

#[proc_macro_derive(Commit)]
pub fn derive_commit(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    self::commit::derive_commit(input)
}

#[proc_macro_derive(Verbatim)]
pub fn derive_verbatim(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    self::verbatim::derive_verbatim(input)
}
