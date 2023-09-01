use proc_macro::TokenStream;

mod generate;

#[proc_macro_derive(Paged)]
pub fn derive_paged(input: TokenStream) -> TokenStream {
	let input = syn::parse_macro_input!(input as syn::DeriveInput);
	generate::paged(input).into()
}
