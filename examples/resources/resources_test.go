package resources

import (
	"context"
	"testing"
)

// ============================================================================
// Host-side resource implementations for imported interfaces
// ============================================================================

// TypesAFoo implementation (host-provided)
type typesAFooResource struct {
	x uint32
}

func (f *typesAFooResource) GetX(ctx context.Context) uint32 {
	return f.x
}

func (f *typesAFooResource) SetX(ctx context.Context, n uint32) {
	f.x = n
}

// TypesABar implementation (host-provided)
type typesABarResource struct {
	value string
}

func (f *typesABarResource) GetValue(ctx context.Context) string {
	return f.value
}

func (f *typesABarResource) Append(ctx context.Context, s string) {
	f.value += s
}

// TypesBFoo implementation (host-provided, different from types-a foo!)
type typesBFooResource struct {
	y string
}

func (f *typesBFooResource) GetY(ctx context.Context) string {
	return f.y
}

func (f *typesBFooResource) SetY(ctx context.Context, s string) {
	f.y = s
}

// TypesBBaz implementation (host-provided)
type typesBBazResource struct {
	count uint32
}

func (f *typesBBazResource) Increment(ctx context.Context) {
	f.count++
}

func (f *typesBBazResource) GetCount(ctx context.Context) uint32 {
	return f.count
}

// IResourcesTypesA implementation
type typesAImpl struct{}

func (typesAImpl) NewFoo(ctx context.Context, x uint32) typesAFooResource {
	return typesAFooResource{x: x}
}

func (typesAImpl) NewBar(ctx context.Context, value string) typesABarResource {
	return typesABarResource{value: value}
}

func (typesAImpl) DoubleFooX(ctx context.Context, f *typesAFooResource) uint32 {
	// This is called by guest WASM when it wants to use a host-provided resource
	// The guest calls the freestanding function DoubleFooX with a borrowed foo handle
	// which gets looked up from our resource table and passed here
	return f.GetX(ctx) * 2
}

func (typesAImpl) MakeBar(ctx context.Context, value string) typesABarResource {
	// This is called by guest WASM when it wants the host to create a bar resource
	return typesABarResource{value: value}
}

// IResourcesTypesB implementation
type typesBImpl struct{}

func (typesBImpl) NewFoo(ctx context.Context, y string) typesBFooResource {
	return typesBFooResource{y: y}
}

func (typesBImpl) NewBaz(ctx context.Context, count uint32) typesBBazResource {
	return typesBBazResource{count: count}
}

func (typesBImpl) TripleBazCount(ctx context.Context, b *typesBBazResource) uint32 {
	// This is called by guest WASM when it wants to use a host-provided resource
	return b.GetCount(ctx) * 3
}

func (typesBImpl) MakeFoo(ctx context.Context, y string) typesBFooResource {
	// This is called by guest WASM when it wants the host to create a foo resource
	return typesBFooResource{y: y}
}

// ============================================================================
// Tests
// ============================================================================

func TestFreestandingFunctions(t *testing.T) {
	ctx := context.Background()

	fac, err := NewResourcesFactory(ctx, &typesAImpl{}, &typesBImpl{})
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(ctx)

	ins, err := fac.Instantiate(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(ctx)

	// Note: We can't easily test DoubleFooX because it requires a guest-created foo resource,
	// and we don't have a freestanding function in types-a that creates a foo.
	// The constructor is guest-internal and shouldn't be called from the host.

	t.Run("MakeBar_creates_guest_resource", func(t *testing.T) {
		// Call the guest's MakeBar function which creates a bar resource in the guest
		// and returns an opaque handle
		barHandle := ins.MakeBar(ctx, "hello")

		// Verify we got a non-zero handle (guest resources start from 1)
		if barHandle == 0 {
			t.Error("MakeBar returned zero handle, expected non-zero")
		}
	})

	t.Run("MakeFoo_creates_guest_resource", func(t *testing.T) {
		// Call the guest's MakeFoo function which creates a foo resource in the guest
		// and returns an opaque handle
		fooHandle := ins.MakeFoo(ctx, "test")

		// Verify we got a non-zero handle
		if fooHandle == 0 {
			t.Error("MakeFoo returned zero handle, expected non-zero")
		}
	})

	t.Run("Multiple_guest_resources_independent", func(t *testing.T) {
		// Create multiple guest bar resources
		bar1 := ins.MakeBar(ctx, "first")
		bar2 := ins.MakeBar(ctx, "second")
		bar3 := ins.MakeBar(ctx, "third")

		// Verify different handles
		if bar1 == bar2 || bar1 == bar3 || bar2 == bar3 {
			t.Error("Expected different handles for different guest resources")
		}

		// All handles should be non-zero
		if bar1 == 0 || bar2 == 0 || bar3 == 0 {
			t.Error("Expected non-zero handles from guest")
		}
	})
}

func TestHostResourceModification(t *testing.T) {
	ctx := context.Background()

	fac, err := NewResourcesFactory(ctx, &typesAImpl{}, &typesBImpl{})
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(ctx)

	ins, err := fac.Instantiate(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(ctx)

	t.Run("Host_resource_state_preserved", func(t *testing.T) {
		// Create a host resource
		hostFoo := typesAFooResource{x: 100}
		handle := fac.TypesAFooResourceTable.Store(hostFoo)
		defer fac.TypesAFooResourceTable.Remove(handle)

		// Get it from the table and verify
		retrieved, ok := fac.TypesAFooResourceTable.Get(handle)
		if !ok {
			t.Fatal("Failed to retrieve resource from table")
		}
		if retrieved.x != 100 {
			t.Errorf("Retrieved resource has x=%d, want 100", retrieved.x)
		}

		// Modify it
		retrievedPtr, ok := fac.TypesAFooResourceTable.get(handle)
		if !ok {
			t.Fatal("Failed to get pointer to resource")
		}
		retrievedPtr.SetX(ctx, 200)

		// Verify modification
		retrieved2, ok := fac.TypesAFooResourceTable.Get(handle)
		if !ok {
			t.Fatal("Failed to retrieve resource after modification")
		}
		if retrieved2.x != 200 {
			t.Errorf("After SetX(200), x=%d, want 200", retrieved2.x)
		}
	})
}

func TestMultipleResources(t *testing.T) {
	ctx := context.Background()

	fac, err := NewResourcesFactory(ctx, &typesAImpl{}, &typesBImpl{})
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(ctx)

	ins, err := fac.Instantiate(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(ctx)

	t.Run("Multiple_host_resources_independent", func(t *testing.T) {
		// Create multiple host resources
		foo1 := typesAFooResource{x: 10}
		foo2 := typesAFooResource{x: 20}
		foo3 := typesAFooResource{x: 30}

		handle1 := fac.TypesAFooResourceTable.Store(foo1)
		defer fac.TypesAFooResourceTable.Remove(handle1)
		handle2 := fac.TypesAFooResourceTable.Store(foo2)
		defer fac.TypesAFooResourceTable.Remove(handle2)
		handle3 := fac.TypesAFooResourceTable.Store(foo3)
		defer fac.TypesAFooResourceTable.Remove(handle3)

		// Verify different handles
		if handle1 == handle2 || handle1 == handle3 || handle2 == handle3 {
			t.Error("Expected different handles for different resources")
		}

		// Verify all were stored successfully
		r1, ok1 := fac.TypesAFooResourceTable.Get(handle1)
		r2, ok2 := fac.TypesAFooResourceTable.Get(handle2)
		r3, ok3 := fac.TypesAFooResourceTable.Get(handle3)

		if !ok1 || !ok2 || !ok3 {
			t.Fatal("Failed to retrieve one or more resources")
		}

		// Verify independent state
		if r1.x != 10 {
			t.Errorf("resource1.x = %d, want 10", r1.x)
		}
		if r2.x != 20 {
			t.Errorf("resource2.x = %d, want 20", r2.x)
		}
		if r3.x != 30 {
			t.Errorf("resource3.x = %d, want 30", r3.x)
		}
	})
}

func TestResourceTableOperations(t *testing.T) {
	ctx := context.Background()

	fac, err := NewResourcesFactory(ctx, &typesAImpl{}, &typesBImpl{})
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(ctx)

	t.Run("Store_and_Get", func(t *testing.T) {
		foo := typesAFooResource{x: 42}
		handle := fac.TypesAFooResourceTable.Store(foo)

		retrieved, ok := fac.TypesAFooResourceTable.Get(handle)
		if !ok {
			t.Fatal("Failed to retrieve stored resource")
		}
		if retrieved.x != 42 {
			t.Errorf("Retrieved resource has x=%d, want 42", retrieved.x)
		}

		fac.TypesAFooResourceTable.Remove(handle)
	})

	t.Run("Remove_makes_unavailable", func(t *testing.T) {
		foo := typesAFooResource{x: 99}
		handle := fac.TypesAFooResourceTable.Store(foo)

		fac.TypesAFooResourceTable.Remove(handle)

		_, ok := fac.TypesAFooResourceTable.Get(handle)
		if ok {
			t.Error("Expected resource to be unavailable after Remove")
		}
	})

	t.Run("Multiple_tables_independent", func(t *testing.T) {
		// Store in types-a foo table
		fooA := typesAFooResource{x: 10}
		handleA := fac.TypesAFooResourceTable.Store(fooA)
		defer fac.TypesAFooResourceTable.Remove(handleA)

		// Store in types-a bar table
		bar := typesABarResource{value: "test"}
		handleBar := fac.TypesABarResourceTable.Store(bar)
		defer fac.TypesABarResourceTable.Remove(handleBar)

		// Store in types-b foo table (different resource type!)
		fooB := typesBFooResource{y: "hello"}
		handleB := fac.TypesBFooResourceTable.Store(fooB)
		defer fac.TypesBFooResourceTable.Remove(handleB)

		// Store in types-b baz table
		baz := typesBBazResource{count: 5}
		handleBaz := fac.TypesBBazResourceTable.Store(baz)
		defer fac.TypesBBazResourceTable.Remove(handleBaz)

		// Verify all can be retrieved independently
		retrievedA, okA := fac.TypesAFooResourceTable.Get(handleA)
		retrievedBar, okBar := fac.TypesABarResourceTable.Get(handleBar)
		retrievedB, okB := fac.TypesBFooResourceTable.Get(handleB)
		retrievedBaz, okBaz := fac.TypesBBazResourceTable.Get(handleBaz)

		if !okA || !okBar || !okB || !okBaz {
			t.Fatal("Failed to retrieve resources from different tables")
		}

		if retrievedA.x != 10 {
			t.Errorf("types-a foo: got x=%d, want 10", retrievedA.x)
		}
		if retrievedBar.value != "test" {
			t.Errorf("types-a bar: got value=%q, want %q", retrievedBar.value, "test")
		}
		if retrievedB.y != "hello" {
			t.Errorf("types-b foo: got y=%q, want %q", retrievedB.y, "hello")
		}
		if retrievedBaz.count != 5 {
			t.Errorf("types-b baz: got count=%d, want 5", retrievedBaz.count)
		}
	})
}
