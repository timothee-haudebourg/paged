use proc_macro2::{Ident, Span, TokenStream, TokenTree};
use quote::{format_ident, quote, ToTokens};
use syn::{punctuated::Punctuated, spanned::Spanned, Token};

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error(transparent)]
	Parse(#[from] syn::parse::Error),
}

impl Error {
	pub fn span(&self) -> Span {
		match self {
			Self::Parse(e) => e.span(),
		}
	}
}

fn extend_generics(
	generics: &syn::Generics,
	context: Option<&syn::TypeParam>,
	additional_bounds: Vec<syn::WherePredicate>,
) -> syn::Generics {
	let mut result = generics.clone();

	if let Some(context) = context {
		result.params.push(syn::GenericParam::Type(context.clone()));
	}

	result
		.make_where_clause()
		.predicates
		.extend(additional_bounds);

	result
}

pub enum FieldIdentOrIndex<'a> {
	Ident(&'a TokenStream, &'a Ident),
	Index(&'a TokenStream, syn::Index),
}

impl<'a> FieldIdentOrIndex<'a> {
	pub fn new(prefix: &'a TokenStream, f: &'a syn::Field, i: usize) -> Self {
		match &f.ident {
			Some(ident) => Self::Ident(prefix, ident),
			None => Self::Index(
				prefix,
				syn::Index {
					index: i as u32,
					span: f.span(),
				},
			),
		}
	}
}

impl<'a> ToTokens for FieldIdentOrIndex<'a> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Self::Ident(prefix, i) => tokens.extend(quote! {
				#prefix #i
			}),
			Self::Index(prefix, i) => tokens.extend(quote! {
				#prefix #i
			}),
		}
	}
}

pub enum FieldConstructor<'a> {
	Ident(&'a Ident),
	Index,
}

impl<'a> FieldConstructor<'a> {
	pub fn new(f: &'a syn::Field) -> Self {
		match &f.ident {
			Some(ident) => Self::Ident(ident),
			None => Self::Index,
		}
	}
}

impl<'a> ToTokens for FieldConstructor<'a> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self {
			Self::Ident(i) => tokens.extend(quote! {
				#i :
			}),
			Self::Index => (),
		}
	}
}

pub struct DecodeFieldsFromHeap<'a>(&'a syn::Fields, &'a Ident);

impl<'a> ToTokens for DecodeFieldsFromHeap<'a> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let context_ident = self.1;
		let args = self.0.iter().map(|f| {
			let ident = FieldConstructor::new(f);
			let ty = &f.ty;
			quote!(#ident <#ty as ::paged::DecodeFromHeap<#context_ident>>::decode_from_heap(input, context, heap)?)
		});

		match self.0 {
			syn::Fields::Unit => (),
			syn::Fields::Named(_) => tokens.extend(quote! {
				{ #(#args),* }
			}),
			syn::Fields::Unnamed(_) => tokens.extend(quote! {
				( #(#args),* )
			}),
		}
	}
}

pub struct DecodeFields<'a>(&'a syn::Fields, &'a Ident);

impl<'a> ToTokens for DecodeFields<'a> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		let context_ident = self.1;
		let args = self.0.iter().map(|f| {
			let ident = FieldConstructor::new(f);
			let ty = &f.ty;
			quote!(#ident <#ty as ::paged::Decode<#context_ident>>::decode(input, context)?)
		});

		match self.0 {
			syn::Fields::Unit => (),
			syn::Fields::Named(_) => tokens.extend(quote! {
				{ #(#args),* }
			}),
			syn::Fields::Unnamed(_) => tokens.extend(quote! {
				( #(#args),* )
			}),
		}
	}
}

pub fn paged(input: syn::DeriveInput) -> Result<TokenStream, Error> {
	let mut options = parse_attributes(input.attrs)?;
	let ident = input.ident;

	let context_ident;
	let context = match options.context {
		Some(p) => {
			context_ident = p.ident.clone();
			let already_exists = input.generics.params.iter().any(|g| match g {
				syn::GenericParam::Type(q) => q.ident == context_ident,
				_ => false,
			});

			if already_exists {
				let bounds = p.bounds.clone();
				let clause: syn::WherePredicate =
					syn::parse2(quote!(#context_ident: #bounds)).unwrap();
				options.encode_sized_bounds.push(clause.clone());
				options.encode_bounds.push(clause.clone());
				options.decode_bounds.push(clause);

				None
			} else {
				Some(p)
			}
		}
		None => {
			context_ident = format_ident!("_C");
			Some(syn::TypeParam {
				attrs: Vec::new(),
				ident: context_ident.clone(),
				colon_token: None,
				bounds: syn::punctuated::Punctuated::new(),
				eq_token: None,
				default: None,
			})
		}
	};

	let encode_sized_generics = extend_generics(&input.generics, None, options.encode_sized_bounds);
	let encode_generics = extend_generics(&input.generics, context.as_ref(), options.encode_bounds);
	let decode_generics = extend_generics(&input.generics, context.as_ref(), options.decode_bounds);

	let (encode_sized_impl_generics, _, encode_sized_where_clause) =
		encode_sized_generics.split_for_impl();
	let (encode_impl_generics, _, encode_where_clause) = encode_generics.split_for_impl();
	let (decode_impl_generics, _, decode_where_clause) = decode_generics.split_for_impl();

	let (_, type_generics, _) = input.generics.split_for_impl();

	match input.data {
		syn::Data::Struct(s) => {
			let encoded_size = fields_size(&s.fields);
			let field_prefix = quote!(&self.);

			let mut tokens = TokenStream::new();

			if !options.is_unsized {
				tokens.extend(quote! {
					impl #encode_sized_impl_generics ::paged::EncodeSized for #ident #type_generics #encode_sized_where_clause {
						const ENCODED_SIZE: u32 = #encoded_size;
					}
				});

				let encode_fields_to_heap = encode_fields_to_heap(
					&s.fields,
					&context_ident,
					|f, i| FieldIdentOrIndex::new(&field_prefix, f, i),
					true,
				);
				let decode_constructor_from_heap = DecodeFieldsFromHeap(&s.fields, &context_ident);

				tokens.extend(quote! {
					impl #encode_impl_generics ::paged::EncodeOnHeap<#context_ident> for #ident #type_generics #encode_where_clause {
						fn encode_on_heap(&self, context: &#context_ident, heap: &mut ::paged::Heap, output: &mut impl ::std::io::Write) -> ::std::io::Result<u32> {
							let mut len = 0;
							#encode_fields_to_heap
							Ok(len)
						}
					}

					impl #decode_impl_generics ::paged::DecodeFromHeap<#context_ident> for #ident #type_generics #decode_where_clause {
						fn decode_from_heap<_R: ::std::io::Seek + ::std::io::Read>(
							input: &mut ::paged::reader::Cursor<_R>,
							context: &mut #context_ident,
							heap: ::paged::HeapSection,
						) -> ::std::io::Result<Self> {
							Ok(Self #decode_constructor_from_heap)
						}
					}
				});
			}

			if !options.requires_heap {
				let encode_fields = encode_fields(
					&s.fields,
					&context_ident,
					|f, i| FieldIdentOrIndex::new(&field_prefix, f, i),
					true,
				);
				let decode_constructor = DecodeFields(&s.fields, &context_ident);

				tokens.extend(quote! {
					impl #encode_impl_generics ::paged::Encode<#context_ident> for #ident #type_generics #encode_where_clause {
						fn encode(&self, context: &#context_ident, output: &mut impl ::std::io::Write) -> ::std::io::Result<u32> {
							let mut len = 0;
							#encode_fields
							Ok(len)
						}
					}

					impl #decode_impl_generics ::paged::Decode<#context_ident> for #ident #type_generics #decode_where_clause {
						fn decode<_R: ::std::io::Read>(
							input: &mut _R,
							context: &mut #context_ident
						) -> ::std::io::Result<Self> {
							Ok(Self #decode_constructor)
						}
					}
				})
			}

			Ok(tokens)
		}
		syn::Data::Enum(e) => {
			let mut encoded_size = quote!(0u32);

			for v in &e.variants {
				let v_size = fields_size(&v.fields);
				encoded_size = quote!(::paged::utils::max(#encoded_size, #v_size))
			}

			let mut tokens = quote! {
				impl #encode_sized_impl_generics ::paged::EncodeSized for #ident #type_generics #encode_sized_where_clause {
					const ENCODED_SIZE: u32 = 1 + #encoded_size;
				}
			};

			let encode_cases = e.variants.iter().enumerate().map(|(i, v)| {
				let variant_ident = &v.ident;
				let inputs = VariantInputs(&v.fields);
				let encode_variant =
					encode_fields_to_heap(&v.fields, &context_ident, VariantInput, false);
				let discriminant = i as u8;
				quote!(Self::#variant_ident #inputs => {
					<u8 as ::paged::Encode<#context_ident>>::encode(&#discriminant, context, output)?;
					#encode_variant
				})
			});

			let decode_from_heap_cases = e.variants.iter().enumerate().map(|(i, v)| {
				let variant_ident = &v.ident;
				let discriminant = i as u8;
				let decode_variant = DecodeFieldsFromHeap(&v.fields, &context_ident);
				let variant_size = fields_size(&v.fields);
				let padding =
					quote!(<Self as ::paged::EncodeSized>::ENCODED_SIZE - (#variant_size));
				quote!(#discriminant => {
					let result = Self::#variant_ident #decode_variant ;
					input.pad(#padding)?;
					Ok(result)
				})
			});

			tokens.extend(quote! {
				impl #encode_impl_generics ::paged::EncodeOnHeap<#context_ident> for #ident #type_generics #encode_where_clause {
					fn encode_on_heap(&self, context: &#context_ident, heap: &mut ::paged::Heap, output: &mut impl ::std::io::Write) -> ::std::io::Result<u32> {
						match self {
							#(#encode_cases),*
						}

						Ok(<Self as ::paged::EncodeSized>::ENCODED_SIZE)
					}
				}

				impl #decode_impl_generics ::paged::DecodeFromHeap<#context_ident> for #ident #type_generics #decode_where_clause {
					fn decode_from_heap<_R: ::std::io::Seek + ::std::io::Read>(
						input: &mut ::paged::reader::Cursor<_R>,
						context: &mut #context_ident,
						heap: ::paged::HeapSection,
					) -> ::std::io::Result<Self> {
						let discriminant = <u8 as ::paged::Decode<#context_ident>>::decode(input, context)?;
						match discriminant {
							#(#decode_from_heap_cases,)*
							_ => Err(::std::io::ErrorKind::InvalidData.into())
						}
					}
				}
			});

			if !options.requires_heap {
				let encode_cases = e.variants.iter().enumerate().map(|(i, v)| {
					let variant_ident = &v.ident;
					let inputs = VariantInputs(&v.fields);
					let encode_variant =
						encode_fields(&v.fields, &context_ident, VariantInput, false);
					let discriminant = i as u8;
					quote!(Self::#variant_ident #inputs => {
						<u8 as ::paged::Encode<#context_ident>>::encode(&#discriminant, context, output)?;
						#encode_variant
					})
				});

				let decode_cases = e.variants.iter().enumerate().map(|(i, v)| {
					let variant_ident = &v.ident;
					let discriminant = i as u8;
					let decode_variant = DecodeFields(&v.fields, &context_ident);
					let variant_size = fields_size(&v.fields);
					let padding =
						quote!(<Self as ::paged::EncodeSized>::ENCODED_SIZE - (#variant_size));
					quote!(#discriminant => {
						let result = Self::#variant_ident #decode_variant ;
						let mut padding = [0; (#padding) as usize];
						input.read_exact(&mut padding)?;
						Ok(result)
					})
				});

				tokens.extend(quote! {
					impl #encode_impl_generics ::paged::Encode<#context_ident> for #ident #type_generics #encode_where_clause {
						fn encode(&self, context: &#context_ident, output: &mut impl ::std::io::Write) -> ::std::io::Result<u32> {
							match self {
								#(#encode_cases),*
							}
							Ok(<Self as ::paged::EncodeSized>::ENCODED_SIZE)
						}
					}

					impl #decode_impl_generics ::paged::Decode<#context_ident> for #ident #type_generics #decode_where_clause {
						fn decode<_R: ::std::io::Read>(
							input: &mut _R,
							context: &mut #context_ident
						) -> ::std::io::Result<Self> {
							let discriminant = <u8 as ::paged::Decode<#context_ident>>::decode(input, context)?;
							match discriminant {
								#(#decode_cases,)*
								_ => Err(::std::io::ErrorKind::InvalidData.into())
							}
						}
					}
				})
			}

			Ok(tokens)
		}
		_ => todo!(),
	}
}

fn fields_size(fields: &syn::Fields) -> TokenStream {
	let mut size = quote!(0u32);

	for f in fields {
		let ty = &f.ty;
		size = quote! {
			#size + <#ty as ::paged::EncodeSized>::ENCODED_SIZE
		}
	}

	size
}

fn encode_fields<'a, T: ToTokens>(
	fields: &'a syn::Fields,
	context_ident: &Ident,
	accessor: impl Fn(&'a syn::Field, usize) -> T,
	capture_len: bool,
) -> TokenStream {
	let mut result = TokenStream::new();

	for (i, f) in fields.iter().enumerate() {
		let accessor = accessor(f, i);
		let ty = &f.ty;
		if capture_len {
			result.extend(quote!(len += ));
		}
		result.extend(
			quote!(<#ty as ::paged::Encode<#context_ident>>::encode(#accessor, context, output)?;),
		)
	}

	result
}

fn encode_fields_to_heap<'a, T: ToTokens>(
	fields: &'a syn::Fields,
	context_ident: &Ident,
	accessor: impl Fn(&'a syn::Field, usize) -> T,
	capture_len: bool,
) -> TokenStream {
	let mut result = TokenStream::new();

	for (i, f) in fields.iter().enumerate() {
		let accessor = accessor(f, i);
		let ty = &f.ty;
		if capture_len {
			result.extend(quote!(len += ));
		}
		result.extend(quote!(<#ty as ::paged::EncodeOnHeap<#context_ident>>::encode_on_heap(#accessor, context, heap, output)?;))
	}

	result
}

struct VariantInputs<'a>(&'a syn::Fields);

impl<'a> ToTokens for VariantInputs<'a> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match self.0 {
			syn::Fields::Unit => (),
			syn::Fields::Named(fields) => {
				let fields = fields.named.iter().map(|f| &f.ident);
				tokens.extend(quote!({ #(#fields),* }))
			}
			syn::Fields::Unnamed(fields) => {
				let fields = (0..fields.unnamed.len()).map(|i| format_ident!("_arg{i}"));
				tokens.extend(quote!(( #(#fields),* )))
			}
		}
	}
}

struct VariantInput<'a>(&'a syn::Field, usize);

impl<'a> ToTokens for VariantInput<'a> {
	fn to_tokens(&self, tokens: &mut TokenStream) {
		match &self.0.ident {
			Some(ident) => ident.to_tokens(tokens),
			None => format_ident!("_arg{}", self.1).to_tokens(tokens),
		}
	}
}

#[derive(Default)]
pub struct Options {
	is_unsized: bool,
	requires_heap: bool,
	encode_bounds: Vec<syn::WherePredicate>,
	encode_sized_bounds: Vec<syn::WherePredicate>,
	decode_bounds: Vec<syn::WherePredicate>,
	context: Option<syn::TypeParam>,
}

pub struct BoundsAttribute {
	list: Punctuated<syn::WherePredicate, Token!(,)>,
}

impl syn::parse::Parse for BoundsAttribute {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		Ok(Self {
			list: Punctuated::parse_terminated(input)?,
		})
	}
}

fn parse_attributes(attributes: Vec<syn::Attribute>) -> Result<Options, Error> {
	let mut options = Options::default();

	for attr in attributes {
		if attr.path().is_ident("paged") {
			match attr.meta {
				syn::Meta::List(list) => {
					let mut tokens = list.tokens.into_iter();
					loop {
						match tokens.next() {
							Some(TokenTree::Ident(id)) => {
								if id == "unsized" {
									options.is_unsized = true
								} else if id == "heap" {
									options.requires_heap = true
								} else if id == "bounds" {
									match tokens.next() {
										Some(TokenTree::Group(group)) => {
											let bounds: BoundsAttribute =
												syn::parse2(group.stream())?;
											options
												.encode_bounds
												.extend(bounds.list.iter().cloned());
											options
												.encode_sized_bounds
												.extend(bounds.list.iter().cloned());
											options.decode_bounds.extend(bounds.list);
										}
										Some(_) => panic!("unexpected token"),
										None => panic!("missing bounds"),
									}
								} else if id == "decode_bounds" {
									match tokens.next() {
										Some(TokenTree::Group(group)) => {
											let bounds: BoundsAttribute =
												syn::parse2(group.stream())?;
											options.decode_bounds.extend(bounds.list);
										}
										Some(_) => panic!("unexpected token"),
										None => panic!("missing bounds"),
									}
								} else if id == "encode_sized_bounds" {
									match tokens.next() {
										Some(TokenTree::Group(group)) => {
											let bounds: BoundsAttribute =
												syn::parse2(group.stream())?;
											options.encode_sized_bounds.extend(bounds.list);
										}
										Some(_) => panic!("unexpected token"),
										None => panic!("missing bounds"),
									}
								} else if id == "encode_bounds" {
									match tokens.next() {
										Some(TokenTree::Group(group)) => {
											let bounds: BoundsAttribute =
												syn::parse2(group.stream())?;
											options.encode_bounds.extend(bounds.list);
										}
										Some(_) => panic!("unexpected token"),
										None => panic!("missing bounds"),
									}
								} else if id == "context" {
									match tokens.next() {
										Some(TokenTree::Group(group)) => {
											options.context = Some(syn::parse2(group.stream())?);
										}
										Some(_) => panic!("unexpected token"),
										None => panic!("missing bounds"),
									}
								} else {
									panic!("unknown `paged` attribute")
								}
							}
							Some(_) => panic!("unexpected token"),
							None => panic!("missing `paged` attribute name"),
						}

						match tokens.next() {
							Some(TokenTree::Punct(p)) if p.as_char() == ',' => (),
							Some(_) => panic!("unexpected token"),
							None => break,
						}
					}
				}
				_ => panic!("invalid attribute"),
			}
		}
	}

	Ok(options)
}
