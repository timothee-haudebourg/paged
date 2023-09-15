use proc_macro::TokenStream;
use proc_macro_error::{abort, proc_macro_error};

mod generate;

#[proc_macro_derive(Paged, attributes(paged))]
#[proc_macro_error]
pub fn derive_paged(input: TokenStream) -> TokenStream {
	let input = syn::parse_macro_input!(input as syn::DeriveInput);
	match generate::paged(input) {
		Ok(tokens) => tokens.into(),
		Err(e) => {
			abort!(e.span(), e)
		}
	}
}
