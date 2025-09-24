package instructions

import (
	"math"
	"testing"
)

func Test_S8Roundtrip(t *testing.T) {
	fac, err := NewInstructionsFactory(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	for expected := math.MinInt8; expected <= math.MaxInt8; expected++ {
		actual := ins.S8Roundtrip(t.Context(), int8(expected))
		if actual != expected {
			t.Errorf("expected: %d, but got: %d", expected, actual)
		}
	}
}

func Test_U8Roundtrip(t *testing.T) {
	fac, err := NewInstructionsFactory(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	for expected := 0; expected <= math.MaxUint8; expected++ {
		actual := ins.TestU8Roundtrip(t.Context(), uint8(expected))
		if actual != expected {
			t.Errorf("expected: %d, but got: %d", expected, actual)
		}
	}
}

func Test_S16Roundtrip(t *testing.T) {
	fac, err := NewInstructionsFactory(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	for expected := math.MinInt16; expected <= math.MaxInt16; expected++ {
		actual := ins.S16Roundtrip(t.Context(), int16(expected))
		if actual != expected {
			t.Errorf("expected: %d, but got: %d", expected, actual)
		}
	}
}

func Test_U16Roundtrip(t *testing.T) {
	fac, err := NewInstructionsFactory(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	for expected := 0; expected <= math.MaxUint16; expected++ {
		actual := ins.U16Roundtrip(t.Context(), uint16(expected))
		if actual != expected {
			t.Errorf("expected: %d, but got: %d", expected, actual)
		}
	}
}
