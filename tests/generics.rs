use cppvtbl::{impl_vtables, vtable};

#[vtable]
trait Test {
	fn id(&self) -> u32;
}

#[impl_vtables(Test)]
struct WrapperRef<'p>(&'p u32);
impl<'t> Test for WrapperRef<'t> {
	fn id(&self) -> u32 {
		0
	}
}

// Const (used for storing vtables) can't be generic over type,
// so we need to allocate vtables to make it work, and this is not
// yet implemented
//
// #[impl_vtables(Test)]
// struct WrapperGeneric<T>(T);
// impl<T> Test for WrapperGeneric<T> {
// 	fn id(&self) -> u32 {
// 		0
// 	}
// }
