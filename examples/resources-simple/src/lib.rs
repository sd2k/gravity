wit_bindgen::generate!();

struct ResourcesWorld;
export!(ResourcesWorld);

impl Guest for ResourcesWorld {
    fn use_fooer(foo: &Fooer) {
        let x = foo.get_x();
        foo.set_x(x + 1);
        foo.set_y("world");
    }

    fn use_fooer_return_new(foo: &Fooer) -> Fooer {
        let x = foo.get_x();
        Fooer::new(x + 1, "world")
    }
}
