wit_bindgen::generate!({
    world: "tuples",
});

struct TuplesWorld;

export!(TuplesWorld);

impl Guest for TuplesWorld {
    fn custom_tuple_func(t: (u32, f64, String)) -> (u32, f64, String) {
        t
    }
    fn anonymous_tuple_func(t: (u32, f64, String)) -> (u32, f64, String) {
        t
    }
}
