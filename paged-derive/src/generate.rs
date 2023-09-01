use proc_macro2::TokenStream;
use quote::{format_ident, quote};

pub fn paged(input: syn::DeriveInput) -> TokenStream {
	match input.data {
		syn::Data::Struct(s) => {
			let ident = input.ident;

			let size = s.fields.iter().map(|f| {
				let ty = &f.ty;
				quote!(+ <#ty as ::paged::EncodeSized>::ENCODED_SIZE)
			});

			let context_ident = format_ident!("C");

			let encode_fields = s.fields.iter().map(|f| {
				let ident = &f.ident;
				let ty = &f.ty;
				quote!(<#ty as ::paged::EncodeOnHeap<#context_ident>>::encode_on_heap(&self.#ident, context, heap, output)?;)
			});

			let decode_fields = s.fields.iter().map(|f| {
				let ident = &f.ident;
				let ty = &f.ty;
				quote!(#ident: <#ty as ::paged::DecodeFromHeap<#context_ident>>::decode_from_heap(input, context, heap)?)
			});

			let mut generics_with_context = input.generics.clone();
			generics_with_context
				.params
				.push(syn::GenericParam::Type(syn::TypeParam {
					attrs: Vec::new(),
					ident: context_ident.clone(),
					colon_token: None,
					bounds: syn::punctuated::Punctuated::new(),
					eq_token: None,
					default: None,
				}));

			let (impl_generics_with_context, _, _) = generics_with_context.split_for_impl();

			let (impl_generics, type_generics, where_clause) = input.generics.split_for_impl();

			quote! {
				impl #impl_generics ::paged::EncodeSized for #ident #type_generics #where_clause {
					const ENCODED_SIZE: u32 = 0u32 #(#size)*;
				}

				impl #impl_generics_with_context ::paged::EncodeOnHeap<C> for #ident #type_generics #where_clause {
					fn encode_on_heap(&self, context: &#context_ident, heap: &mut ::paged::Heap, output: &mut impl ::std::io::Write) -> ::std::io::Result<u32> {
						#(#encode_fields)*
						Ok(<Self as ::paged::EncodeSized>::ENCODED_SIZE)
					}
				}

				impl #impl_generics_with_context ::paged::DecodeFromHeap<C> for #ident #type_generics #where_clause {
					fn decode_from_heap<_R: ::std::io::Seek + ::std::io::Read>(
						input: &mut ::paged::reader::Cursor<_R>,
						context: &mut #context_ident,
						heap: &::paged::HeapSection,
					) -> ::std::io::Result<Self> {
						Ok(Self {
							#(#decode_fields),*
						})
					}
				}
			}
		}
		_ => todo!(),
	}
}
