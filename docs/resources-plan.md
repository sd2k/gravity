# Resources Implementation Plan for Gravity

## Overview

Resources are stateful objects in the Component Model that represent entities that exist outside of the component and should be passed by reference rather than by value. This document outlines the plan to add resource support to Gravity.

## Motivation

Resources enable several key use cases:
- File handles and I/O streams
- Database connections
- Network sockets
- Cryptographic contexts
- Any stateful host-provided API

## Current Status

As of now, Gravity has:
- ✅ Basic types (string, u32, etc.)
- ✅ Option types (`option<T>`)
- ✅ Result types (`result<T, E>`)
- ✅ Records (structs)
- ✅ Variants (with prefixed names)
- ✅ Lists/arrays
- ❌ Resources (TODO #5)
- ❌ Handles (TODO #5)

## Implementation Phases

### Phase 1: Basic Resource Types (Week 1-2)

**Goal**: Generate Go types for resources without methods

**Tasks**:
1. Update `TypeDefKind::Resource` handling in `define_type()`
2. Generate basic Go struct with handle field
3. Add resource handle tracking to factory/instance

**Example WIT**:
```wit
resource blob {
    constructor(init: list<u8>);
}
```

**Generated Go**:
```go
type Blob struct {
    handle uint32
    instance *OutlierInstance
}

func (i *OutlierInstance) NewBlob(ctx context.Context, init []uint8) (*Blob, error) {
    // Call constructor
    handle, err := i.module.ExportedFunction("blob-new").Call(ctx, ...)
    return &Blob{handle: handle, instance: i}, err
}
```

### Phase 2: Resource Methods (Week 2-3)

**Goal**: Support method calls on resources

**Tasks**:
1. Parse resource methods from WIT
2. Generate Go methods that pass handle as first parameter
3. Handle `self: borrow<resource>` parameters

**Example**:
```go
func (b *Blob) Write(ctx context.Context, bytes []uint8) error {
    _, err := b.instance.module.ExportedFunction("blob-write").Call(
        ctx, 
        uint64(b.handle),
        // ... other parameters
    )
    return err
}
```

### Phase 3: Resource Lifecycle (Week 3-4)

**Goal**: Proper cleanup and ownership tracking

**Tasks**:
1. Add destructor support (`Close()` methods)
2. Track resource ownership (owned vs borrowed)
3. Prevent use-after-free

**Implementation**:
```go
type ResourceManager struct {
    sync.RWMutex
    handles  map[uint32]interface{}
    nextID   uint32
    closed   map[uint32]bool
}

func (b *Blob) Close(ctx context.Context) error {
    if b.instance.resources.IsClosed(b.handle) {
        return errors.New("resource already closed")
    }
    _, err := b.instance.module.ExportedFunction("blob-drop").Call(ctx, uint64(b.handle))
    b.instance.resources.MarkClosed(b.handle)
    return err
}
```

### Phase 4: Static Functions (Week 4)

**Goal**: Support static resource functions

**Example**:
```wit
resource blob {
    merge: static func(lhs: blob, rhs: blob) -> blob;
}
```

**Generated Go**:
```go
func (i *OutlierInstance) BlobMerge(ctx context.Context, lhs *Blob, rhs *Blob) (*Blob, error) {
    // Static function - no implicit self parameter
    handle, err := i.module.ExportedFunction("blob-merge").Call(
        ctx,
        uint64(lhs.handle),
        uint64(rhs.handle),
    )
    // ...
}
```

### Phase 5: Advanced Features (Week 5+)

**Goal**: Handle complex resource patterns

**Tasks**:
1. Resources as parameters/return values in regular functions
2. Resources in records and variants
3. Optional resources (`option<resource>`)
4. Lists of resources (`list<resource>`)

## Technical Details

### Instructions to Implement

New instructions needed:
- `ResourceNew` - constructor calls
- `ResourceRep` - get handle representation
- `ResourceDrop` - destructor calls
- `HandleOwned` - owned handle operations
- `HandleBorrowed` - borrowed handle operations

### Code Changes Required

1. **main.rs**:
   - Add `GoType::Resource(String)` variant
   - Implement `ResourceNew`, `ResourceRep`, `ResourceDrop` instructions
   - Update `define_type()` for `TypeDefKind::Resource`

2. **Generated Go Code**:
   - Resource struct with handle
   - Constructor functions
   - Method receivers
   - Close/cleanup methods
   - Resource manager integration

### Testing Strategy

1. **Unit Tests**:
   - Simple resource creation/destruction
   - Method calls
   - Static functions

2. **Integration Tests**:
   - Resource lifecycle across multiple calls
   - Error handling (invalid handles)
   - Concurrent access

3. **Example Components**:
   - File I/O resource
   - Database connection pool
   - HTTP client resource

## Open Questions

1. **Finalizers**: Should we use Go finalizers for automatic cleanup?
   - Pro: Prevents leaks
   - Con: Non-deterministic, performance impact

2. **Thread Safety**: How to handle concurrent access?
   - Option A: All methods take locks
   - Option B: Leave to user
   - Option C: Document as not thread-safe

3. **Handle Format**: uint32 vs opaque type?
   - uint32 is simple but allows mistakes
   - Opaque type is safer but more complex

4. **Error Recovery**: What if Wasm panics while holding resources?
   - Need cleanup in defer blocks
   - Track all active resources per instance

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Memory leaks from unclosed resources | High | Finalizers + leak detection in tests |
| Use-after-free bugs | High | Track closed handles, return errors |
| Complex ownership semantics | Medium | Clear documentation, examples |
| Performance overhead of handle tracking | Low | Use efficient maps, lazy initialization |

## Success Criteria

- [ ] Basic resource types compile without errors
- [ ] Constructor and destructor calls work
- [ ] Methods can be called on resources
- [ ] Static functions work
- [ ] Resources can be passed as parameters
- [ ] No memory leaks in typical usage
- [ ] Clear documentation and examples

## Timeline Estimate

- **Phase 1-2**: 2-3 weeks (basic functionality)
- **Phase 3-4**: 1-2 weeks (lifecycle and static functions)
- **Phase 5**: 2+ weeks (advanced features)
- **Total**: 5-7 weeks for full implementation

## Next Steps

1. Review and approve this plan
2. Create tracking issue for resources
3. Start with Phase 1 implementation
4. Create test WIT files with resource definitions
5. Document Go usage patterns as we implement

## References

- [Component Model Resources Spec](https://component-model.bytecodealliance.org/design/wit.html#resources)
- [Canonical ABI for Resources](https://github.com/WebAssembly/component-model/blob/main/design/mvp/CanonicalABI.md#resources)
- [wit-bindgen Resource Handling](https://github.com/bytecodealliance/wit-bindgen)