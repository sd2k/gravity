package tuples

import (
	"testing"
)

func TestCustomTupleFunc(t *testing.T) {
	fac, err := NewTuplesFactory(t.Context(), struct{}{})
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	tuple := CustomTuple{F0: 0, F1: 1, F2: "2"}
	actual := ins.CustomTupleFunc(t.Context(), tuple)
	if actual != tuple {
		t.Errorf("expected: %v, but got: %v", tuple, actual)
	}
}

func TestAnonymousTupleFunc(t *testing.T) {
	fac, err := NewTuplesFactory(t.Context(), struct{}{})
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(t.Context())

	ins, err := fac.Instantiate(t.Context())
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(t.Context())

	tuple := struct {
		F0 uint32
		F1 float64
		F2 string
	}{F0: 0, F1: 1, F2: "2"}
	actual := ins.AnonymousTupleFunc(t.Context(), tuple)
	if actual != tuple {
		t.Errorf("expected: %v, but got: %v", tuple, actual)
	}
}
