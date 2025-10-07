use std::cell::{Cell, RefCell};

use crate::exports::arcjet::resources::{
    types_a::{GuestBar as _, GuestFoo as _},
    types_b::{GuestBaz, GuestFoo as _},
};

wit_bindgen::generate!({
    world: "resources",
});

struct ResourcesWorld;

export!(ResourcesWorld);

// Implementation for types-a::foo
struct FooA {
    x: Cell<u32>,
}

impl exports::arcjet::resources::types_a::GuestFoo for FooA {
    fn new(x: u32) -> Self {
        Self { x: Cell::new(x) }
    }

    fn get_x(&self) -> u32 {
        self.x.get()
    }

    fn set_x(&self, n: u32) {
        self.x.set(n);
    }
}

// Implementation for types-a::bar
struct Bar {
    value: RefCell<String>,
}

impl exports::arcjet::resources::types_a::GuestBar for Bar {
    fn new(value: String) -> Self {
        Self {
            value: RefCell::new(value),
        }
    }

    fn get_value(&self) -> String {
        self.value.borrow().clone()
    }

    fn append(&self, s: String) {
        self.value.borrow_mut().push_str(&s);
    }
}

// Implementation for types-b::foo (different from types-a::foo!)
struct FooB {
    y: RefCell<String>,
}

impl exports::arcjet::resources::types_b::GuestFoo for FooB {
    fn new(y: String) -> Self {
        Self { y: RefCell::new(y) }
    }

    fn get_y(&self) -> String {
        self.y.borrow().clone()
    }

    fn set_y(&self, s: String) {
        self.y.replace(s);
    }
}

// Implementation for types-b::baz
struct Baz {
    count: Cell<u32>,
}

impl exports::arcjet::resources::types_b::GuestBaz for Baz {
    fn new(count: u32) -> Self {
        Self {
            count: Cell::new(count),
        }
    }

    fn increment(&self) {
        self.count.update(|c| c + 1);
    }

    fn get_count(&self) -> u32 {
        self.count.get()
    }
}

// Guest implementations for both interfaces
impl exports::arcjet::resources::types_a::Guest for ResourcesWorld {
    type Foo = FooA;
    type Bar = Bar;

    // Function that takes a host-provided resource (import side)
    fn double_foo_x(f: exports::arcjet::resources::types_a::FooBorrow<'_>) -> u32 {
        // Call the host-provided foo's get_x method and double it
        f.get::<FooA>().get_x() * 2
    }

    // Function that creates and returns a guest resource (export side)
    fn make_bar(value: String) -> exports::arcjet::resources::types_a::Bar {
        exports::arcjet::resources::types_a::Bar::new(Bar::new(value))
    }
}

impl exports::arcjet::resources::types_b::Guest for ResourcesWorld {
    type Foo = FooB;
    type Baz = Baz;

    // Function that takes a host-provided resource (import side)
    fn triple_baz_count(b: exports::arcjet::resources::types_b::BazBorrow<'_>) -> u32 {
        // Call the host-provided baz's get_count method and triple it
        b.get::<Baz>().get_count() * 3
    }

    // Function that creates and returns a guest resource (export side)
    fn make_foo(y: String) -> exports::arcjet::resources::types_b::Foo {
        exports::arcjet::resources::types_b::Foo::new(FooB::new(y))
    }
}
