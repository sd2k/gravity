package resources

import (
	"context"
	"testing"
)

// foo is a resource type.
type foo struct {
	x uint32
	y string
}

// GetX implements the IFaceFooer interface.
func (f *foo) GetX(context.Context) uint32 {
	return f.x
}

// GetY implements the IFaceFooer interface.
func (f *foo) GetY(context.Context) string {
	return f.y
}

// SetX implements the IFaceFooer interface.
func (f *foo) SetX(_ context.Context, x uint32) {
	f.x = x
}

// SetY implements the IFaceFooer interface.
func (f *foo) SetY(_ context.Context, y string) {
	f.y = y
}

// iface is an implementation of the IFaceFooer interface.
type iface struct{}

// NewFoo implements the IResourcesIFace interface.
func (iface) NewFooer(ctx context.Context, x uint32, y string) foo {
	return foo{x: x, y: y}
}

func TestResources(t *testing.T) {
	ctx := context.Background()

	// Create factory with interface implementations.
	// Resource types are inferred from the interface implementation.
	// TODO: check if type inference works when using multiple interfaces and/or resources.
	fac, err := NewResourcesFactory(ctx, &iface{})
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(ctx)

	ins, err := fac.Instantiate(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(ctx)

	// Create a new instance of our `foo` resource, and store it
	// in the table.
	f1 := foo{x: 42, y: "Hello"}
	handle := fac.IfaceFooerResourceTable.Store(f1)
	defer fac.IfaceFooerResourceTable.Remove(handle)

	// Call the exported function `use-fooer` on the module, passing
	// the handle to the `foo` resource.
	ins.UseFooer(ctx, handle)

	// Get a copy of the resource from the table
	// and check that it has the expected values.
	// Since this is a borrowed resource, it should have been
	// modified.
	f1Ptr, ok := fac.IfaceFooerResourceTable.Get(handle)
	if !ok {
		t.Errorf("expected resource to be present in table")
	}
	if f1Ptr.x != 43 {
		t.Errorf("expected f1Ptr.x to be 43, got %d", f1Ptr.x)
	}
	if f1Ptr.y != "world" {
		t.Errorf("expected f1Ptr.y to be 'world', got '%s'", f1Ptr.y)
	}
	// Make sure the original wasn't modified.
	if f1.x != 42 {
		t.Errorf("expected f1.x to be unmodified, got %d", f1.x)
	}
	if f1.y != "Hello" {
		t.Errorf("expected f1.y to be unmodified, got '%s'", f1.y)
	}

}

func TestGuestCreatedResources(t *testing.T) {
	ctx := context.Background()

	// Create factory with interface implementation
	fac, err := NewResourcesFactory(ctx, &iface{})
	if err != nil {
		t.Fatal(err)
	}
	defer fac.Close(ctx)

	ins, err := fac.Instantiate(ctx)
	if err != nil {
		t.Fatal(err)
	}
	defer ins.Close(ctx)

	t.Run("UseFooerReturnNew_creates_host_resource_via_callback", func(t *testing.T) {
		// Create a host resource
		hostFoo := foo{x: 100, y: "host"}
		handle := fac.IfaceFooerResourceTable.Store(hostFoo)
		defer fac.IfaceFooerResourceTable.Remove(handle)

		// Call guest function that borrows host resource and returns NEW resource
		// The guest calls Fooer::new() which calls back to the HOST constructor
		newHandle := ins.UseFooerReturnNew(ctx, handle)

		// The returned handle should be different (it's a new resource)
		if newHandle == handle {
			t.Errorf("expected different handle for new resource, got same: %d", newHandle)
		}

		// The new resource IS in the host's table because the guest called the host constructor
		newResource, ok := fac.IfaceFooerResourceTable.Get(newHandle)
		if !ok {
			t.Errorf("new resource should be in host table (created via host constructor)")
		}

		// Verify it has the expected values (x+1, "world")
		if newResource.x != 101 {
			t.Errorf("expected new resource x=101, got %d", newResource.x)
		}
		if newResource.y != "world" {
			t.Errorf("expected new resource y='world', got %q", newResource.y)
		}

		// We can pass the new resource back to other guest functions
		ins.UseFooer(ctx, newHandle)

		// The original host resource should still be in the table (we only borrowed it)
		_, ok = fac.IfaceFooerResourceTable.Get(handle)
		if !ok {
			t.Errorf("original host resource should still be in table after borrow")
		}

		// Clean up the new resource
		fac.IfaceFooerResourceTable.Remove(newHandle)
	})

	t.Run("Multiple_new_resources_independent", func(t *testing.T) {
		// Create multiple host resources and get back multiple new resources
		host1 := foo{x: 1, y: "one"}
		host2 := foo{x: 2, y: "two"}
		handle1 := fac.IfaceFooerResourceTable.Store(host1)
		defer fac.IfaceFooerResourceTable.Remove(handle1)
		handle2 := fac.IfaceFooerResourceTable.Store(host2)
		defer fac.IfaceFooerResourceTable.Remove(handle2)

		// Get new resources from both (guest calls host constructor)
		new1 := ins.UseFooerReturnNew(ctx, handle1)
		defer fac.IfaceFooerResourceTable.Remove(new1)
		new2 := ins.UseFooerReturnNew(ctx, handle2)
		defer fac.IfaceFooerResourceTable.Remove(new2)

		// New handles should be different from each other
		if new1 == new2 {
			t.Errorf("expected different handles, got same: %d", new1)
		}

		// Both new resources should work independently
		ins.UseFooer(ctx, new1)
		ins.UseFooer(ctx, new2)

		// Can create another new resource from the same host resource
		new3 := ins.UseFooerReturnNew(ctx, handle1)
		defer fac.IfaceFooerResourceTable.Remove(new3)
		if new3 == new1 {
			t.Errorf("expected different handle for different resource, got same: %d", new3)
		}

		ins.UseFooer(ctx, new3)
	})
}
