use cppvtbl::{impl_vtables, vtable, HasVtable, WithVtables};

#[vtable]
trait Operator {
	fn perform(&self, a: u8) -> u8;
	fn reconfigure(&mut self, n: u8);
}

#[vtable]
trait Printable {
	fn print(&self);
	fn print_debug(&self);
}

#[impl_vtables(Operator, Printable)]
struct Add(u8);
#[impl_vtables(Operator)]
struct Sub(u8);

impl Operator for Add {
	fn perform(&self, a: u8) -> u8 {
		self.0 + a
	}

	fn reconfigure(&mut self, n: u8) {
		self.0 = n
	}
}
impl Printable for Add {
	fn print(&self) {
		print!("+ {}", self.0)
	}

	fn print_debug(&self) {
		print!("Add({:?})", self.0)
	}
}
impl Operator for Sub {
	fn perform(&self, a: u8) -> u8 {
		self.0 - a
	}

	fn reconfigure(&mut self, n: u8) {
		self.0 = n
	}
}

#[test]
fn test() {
	let mut add = WithVtables::new(Add(10));
	let operator = HasVtable::<OperatorVtable>::get(&add);
	let printable = HasVtable::<PrintableVtable>::get(&add);

	printable.print();
	assert_eq!(operator.perform(2), 12);

	let mut operator_mut = HasVtable::<OperatorVtable>::get_mut(&mut add);
	operator_mut.reconfigure(20);

	assert_eq!(operator_mut.perform(2), 22);
}
