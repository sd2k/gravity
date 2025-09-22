package basic

import (
	"testing"
)

func TestI32FromS8(t *testing.T) {
	fac, err := NewBasicFactory(t.Context())
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
