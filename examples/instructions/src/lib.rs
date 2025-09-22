wit_bindgen::generate!({
    world: "instructions",
});

struct InstructionsWorld;

export!(InstructionsWorld);

impl Guest for InstructionsWorld {
    fn i32_from_s8(val: i8) {
        assert!((i8::MIN..=i8::MAX).contains(&val));
    }
}
