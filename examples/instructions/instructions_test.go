package instructions

import (
	"math"
	"testing"
)

func TestI32FromS8(t *testing.T) {
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

	for x := math.MinInt8; x <= math.MaxInt8; x++ {
		ins.I32FromS8(t.Context(), int8(x))
	}
}

func TestS8FromI32(t *testing.T) {
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

	actual := ins.S8FromI32(t.Context())

	const expected = 0
	if actual != expected {
		t.Errorf("expected: %d, but got: %d", expected, actual)
	}
}
