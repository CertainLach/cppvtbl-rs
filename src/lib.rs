#![no_std]

extern crate self as cppvtbl;

use core::{
	ffi::c_void,
	marker::PhantomPinned,
	mem,
	ops::{Deref, DerefMut},
	pin::Pin,
};

#[cfg(feature = "macros")]
pub use cppvtbl_macros::{impl_vtables, vtable};

#[repr(C)]
pub struct WithVtables<T: HasVtables> {
	vtables: T::Tables,
	value: T,
}
impl<T: HasVtables> WithVtables<T> {
	pub fn new(value: T) -> Self {
		Self {
			vtables: T::TABLES,
			value,
		}
	}
	pub fn vtables(&self) -> &T::Tables {
		&self.vtables
	}
	/// Writing into vtables may cause UB
	pub fn vtables_mut(&mut self) -> *mut T::Tables {
		&mut self.vtables
	}
}
impl<T: HasVtables> From<T> for WithVtables<T> {
	fn from(value: T) -> Self {
		Self::new(value)
	}
}
impl<T: HasVtables> Deref for WithVtables<T> {
	type Target = T;

	fn deref(&self) -> &Self::Target {
		&self.value
	}
}
impl<T: HasVtables> DerefMut for WithVtables<T> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.value
	}
}

pub unsafe trait HasVtables {
	type Tables;
	const TABLES: Self::Tables;
}

#[repr(transparent)]
pub struct VtableRef<V: 'static>(&'static V, PhantomPinned);
impl<V: 'static> VtableRef<V> {
	/// Safety: constructed vtable should only be used by reference,
	/// inside of WithVtables wrapper
	pub const unsafe fn new(vtable: &'static V) -> Self {
		Self(vtable, PhantomPinned)
	}
	pub fn table(&self) -> &'static V {
		self.0
	}
	pub fn into_raw(v: &Self) -> *const c_void {
		v as *const _ as *const c_void
	}
	pub fn into_raw_mut(v: &mut Self) -> *mut c_void {
		v as *mut _ as *mut c_void
	}
	/// Safety: lifetime should be correctly specified
	pub unsafe fn from_raw<'r>(raw: *const c_void) -> &'r Self {
		mem::transmute(raw as *const _ as *const Self)
	}
	/// Safety: lifetime should be correctly specified
	pub unsafe fn from_raw_mut<'r>(raw: *mut c_void) -> &'r mut Self {
		mem::transmute(raw as *mut _ as *mut Self)
	}
}

pub trait HasVtable<V>: Sized + HasVtables {
	fn get(from: &WithVtables<Self>) -> &VtableRef<V>;
	// Vtable shouldn't be moved outside of owning struct, so it is wrapped in Pin
	fn get_mut(from: &mut WithVtables<Self>) -> Pin<&mut VtableRef<V>>;
}
