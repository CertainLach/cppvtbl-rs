use proc_macro::TokenStream as TokenStreamRaw;
use quote::{format_ident, quote, quote_spanned};
use syn::{
	parse::Parse, parse_macro_input, punctuated::Punctuated, spanned::Spanned, token::Comma, FnArg,
	Ident, Index, ItemStruct, ItemTrait,
};

#[proc_macro_attribute]
pub fn vtable(attr: TokenStreamRaw, item: TokenStreamRaw) -> TokenStreamRaw {
	if !attr.is_empty() {
		return (quote! { compile_error!("vtable attribute has no args"); }).into();
	}

	let item = parse_macro_input!(item as ItemTrait);

	for item in item.items.iter() {
		let m = match item {
			syn::TraitItem::Method(m) => m,
			_ => {
				return (quote_spanned! { item.span() => compile_error!("only methods allowed in trait"); })
					.into()
			}
		};
		let r = match m.sig.receiver() {
			Some(FnArg::Receiver(r)) => { r },
			Some(v) => {return (quote_spanned! { v.span() => compile_error!("only self receivers allowed"); }).into()}
			None => {return (quote_spanned! { m.sig.span() => compile_error!("expected receiver"); }).into()},
		};
		if r.reference.is_none() {
			return (quote_spanned! { r.span() => compile_error!("should be reference type"); })
				.into();
		}

		for arg in m.sig.inputs.iter().skip(1) {
			let arg = match arg {
				FnArg::Typed(arg) => arg,
				_ => unreachable!(),
			};

			match arg.pat.as_ref() {
				syn::Pat::Ident(_) => {},
				pat => return (quote_spanned! { pat.span() => compile_error!("only ident patterns allowed"); }).into(),
			}
		}
	}

	let methods = item
		.items
		.iter()
		.filter_map(|i| match i {
			syn::TraitItem::Method(m) => Some(m.sig.clone()),
			_ => unreachable!(),
		})
		.map(|sig| {
			let r = match sig.receiver() {
				Some(FnArg::Receiver(r)) => r.clone(),
				_ => unreachable!(),
			};
			(r, sig)
		})
		.collect::<Vec<_>>();

	let name = &item.ident;
	let vtable_name = format_ident!("{}Vtable", name);
	let vtable_impl_name = format_ident!("unsafe_impl_{}Vtable", name);

	let vtable_members = methods.iter().map(|(r, sig)| {
		let name = &sig.ident;
		let this = if r.mutability.is_some() {
			quote! {core::pin::Pin<&mut cppvtbl::VtableRef<Self>>}
		} else {
			quote! {&cppvtbl::VtableRef<Self>}
		};
		let inputs = sig.inputs.iter().skip(1);
		let output = &sig.output;
		quote! {
			pub #name: unsafe extern "C" fn(#this, #(#inputs,)*) #output
		}
	});
	let macro_vtable_fields = methods.iter().map(|(r, sig)| {
		let meth = &sig.ident;
		let this = if r.mutability.is_some() {
			quote! {core::pin::Pin<&mut cppvtbl::VtableRef<#vtable_name>>}
		} else {
			quote! {&cppvtbl::VtableRef<#vtable_name>}
		};
		let get_top = if r.mutability.is_some() {
			quote! {
				 let top: &mut cppvtbl::WithVtables<$this> = core::mem::transmute((core::pin::Pin::get_unchecked_mut(this) as *mut _ as *mut usize).offset($offset))
			}
		} else {
			quote! {
				let top: &cppvtbl::WithVtables<$this> = core::mem::transmute((this as *const _ as *const usize).offset($offset))
			}
		};
		let inputs = sig.inputs.iter().skip(1);
		let output = &sig.output;
		let args = sig
			.inputs
			.iter()
			.skip(1)
			.map(|a| match a {
				FnArg::Receiver(_) => unreachable!(),
				FnArg::Typed(t) => t,
			})
			.map(|a| match a.pat.as_ref() {
				syn::Pat::Ident(i) => &i.ident,
				_ => unreachable!(),
			});
		quote! {
			#meth: {
				unsafe extern "C" fn #meth(this: #this, #(#inputs,)*) #output {
					#get_top;
					<$this as #name>::#meth(top, #(#args,)*)
				}

				#meth
			}
		}
	});
	let impl_members = methods.iter().map(|(r, sig)| {
		let name = &sig.ident;
		let args = sig
			.inputs
			.iter()
			.skip(1)
			.map(|a| match a {
				FnArg::Receiver(_) => unreachable!(),
				FnArg::Typed(t) => t,
			})
			.map(|a| match a.pat.as_ref() {
				syn::Pat::Ident(i) => &i.ident,
				_ => unreachable!(),
			});
		let self_v = if r.mutability.is_some() {
			quote! {
				// Safety: creating mut reference to VtableRef is already unsafe
				// so we assuming pin is valid
				core::pin::Pin::new_unchecked(self)
			}
		} else {
			quote! {
				self
			}
		};
		quote! {
			#sig {
				unsafe { (self.table().#name)(#self_v, #(#args,)*) }
			}
		}
	});
	let impl_mut_members = methods.iter().map(|(r, sig)| {
		let name = &sig.ident;
		let args = sig
			.inputs
			.iter()
			.skip(1)
			.map(|a| match a {
				FnArg::Receiver(_) => unreachable!(),
				FnArg::Typed(t) => t,
			})
			.map(|a| match a.pat.as_ref() {
				syn::Pat::Ident(i) => &i.ident,
				_ => unreachable!(),
			});
		if r.mutability.is_some() {
			quote! {
				#sig {
					// Safety: we're not moving pinned value, nor giving inner code access to it
					let pin = unsafe { core::pin::Pin::get_unchecked_mut(core::pin::Pin::as_mut(self)) };
					unsafe { (pin.table().#name)(core::pin::Pin::new_unchecked(pin), #(#args,)*) }
				}
			}
		} else {
			quote! {
				#sig {
					let pin = core::pin::Pin::as_ref(self);
					unsafe { (pin.table().#name)(&pin, #(#args,)*) }
				}
			}
		}
	});

	(quote! {
		#item

		#[repr(C)]
		pub struct #vtable_name {
			#(#vtable_members,)*
		}
		#[allow(non_upper_case_globals, dead_code)]
		#[macro_export]
		macro_rules! #vtable_impl_name {
			($impl:ident, $this:ty, $offset:expr) => {
				#[allow(non_upper_case_globals)]
				const $impl: &'static #vtable_name = &#vtable_name {
					#(#macro_vtable_fields,)*
				};
			}
		}
		impl #name for cppvtbl::VtableRef<#vtable_name> {
			#(#impl_members)*
		}
		impl #name for core::pin::Pin<&mut cppvtbl::VtableRef<#vtable_name>> {
			#(#impl_mut_members)*
		}

	})
	.into()
}

struct VtablesInput {
	tables: Punctuated<Ident, Comma>,
}
impl Parse for VtablesInput {
	fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
		Ok(Self {
			tables: input.parse_terminated(Ident::parse)?,
		})
	}
}

#[proc_macro_attribute]
pub fn impl_vtables(attr: TokenStreamRaw, item: TokenStreamRaw) -> TokenStreamRaw {
	let input = parse_macro_input!(attr as VtablesInput);
	let this = parse_macro_input!(item as ItemStruct);

	let this_name = &this.ident;

	let impl_macro_calls = input.tables.iter().enumerate().map(|(i, name)| {
		let macro_name = format_ident!("unsafe_impl_{}Vtable", name);
		let const_name = format_ident!("{}VtableFor{}", name, this_name);
		let i = i as isize;
		quote! {
			#macro_name!(#const_name, #this_name, -#i)
		}
	});
	let type_tables = input.tables.iter().map(|name| {
		let vtable_name = format_ident!("{}Vtable", name);
		quote! {
			cppvtbl::VtableRef<#vtable_name>
		}
	});
	let impl_tables = input.tables.iter().map(|name| {
		let const_name = format_ident!("{}VtableFor{}", name, this_name);
		quote! {
			unsafe { cppvtbl::VtableRef::new(#const_name) }
		}
	});
	let has_vtable = input.tables.iter().enumerate().map(|(i, name)| {
		let vtable_name = format_ident!("{}Vtable", name);
		let index = Index::from(i);
		quote! {
			impl cppvtbl::HasVtable<#vtable_name> for #this_name {
				fn get(from: &cppvtbl::WithVtables<Self>) -> &cppvtbl::VtableRef<#vtable_name> {
					&from.vtables().#index
				}
				fn get_mut(from: &mut cppvtbl::WithVtables<Self>) -> core::pin::Pin<&mut cppvtbl::VtableRef<#vtable_name>> {
					unsafe { core::pin::Pin::new_unchecked(&mut (&mut *from.vtables_mut()).#index) }
				}
			}
		}
	});

	(quote! {
		#this
		#(#impl_macro_calls;)*
		unsafe impl cppvtbl::HasVtables for #this_name {
			type Tables = (#(#type_tables,)*);
			const TABLES: Self::Tables = (#(#impl_tables,)*);
		}
		#(#has_vtable)*
	})
	.into()
}
