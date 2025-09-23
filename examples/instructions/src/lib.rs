wit_bindgen::generate!({
    world: "instructions",
});

struct InstructionsWorld;

export!(InstructionsWorld);

impl Guest for InstructionsWorld {
    fn i32_from_s8(val: i8) {
        assert!((i8::MIN..=i8::MAX).contains(&val));
    }
    fn s8_from_i32() -> i8 {
        Default::default()
    }
    fn i32_from_u8(val: u8) {
        assert!((u8::MIN..=u8::MAX).contains(&val));
    }
    fn u8_from_i32() -> u8 {
        Default::default()
    }
    fn i32_from_s16(val: i16) {
        assert!((i16::MIN..=i16::MAX).contains(&val));
    }
    fn s16_from_i32() -> i16 {
        Default::default()
    }
    fn i32_from_u16(val: u16) {
        assert!((u16::MIN..=u16::MAX).contains(&val));
    }
    fn u16_from_i32() -> u16 {
        Default::default()
    }
}
