# Gravity Refactoring Plan

## Overview

The current Gravity codebase consists of a single 2600+ line `main.rs` file, making it difficult to test individual components and understand the code structure. This plan outlines a refactoring strategy to create a modular, testable architecture.

## Current Problems

1. **Monolithic Structure**: Everything in one file makes navigation difficult
2. **Limited Testing**: Can only do end-to-end UI tests, no unit tests
3. **Tight Coupling**: Code generation, type resolution, and formatting are intertwined
4. **Hard to Extend**: Adding new features requires understanding the entire file
5. **Code Duplication**: Similar patterns repeated without reusable abstractions

## Proposed Architecture

```
gravity/
├── cmd/
│   └── gravity/
│       ├── src/
│       │   ├── main.rs              # CLI entry point only
│       │   └── lib.rs               # Library root
│       ├── Cargo.toml
│       └── tests/
│           └── integration/          # End-to-end tests
└── crates/
    ├── gravity-codegen/              # Core code generation
    │   ├── src/
    │   │   ├── lib.rs
    │   │   ├── types.rs             # GoType, GoResult, etc.
    │   │   ├── instructions.rs      # Instruction handlers
    │   │   ├── bindings.rs          # Bindings generator
    │   │   └── context.rs           # Generation context
    │   └── tests/
    ├── gravity-go/                   # Go-specific formatting
    │   ├── src/
    │   │   ├── lib.rs
    │   │   ├── identifier.rs        # GoIdentifier
    │   │   ├── formatter.rs         # FormatInto implementations
    │   │   └── imports.rs           # Import management
    │   └── tests/
    └── gravity-wit/                  # WIT parsing helpers
        ├── src/
        │   ├── lib.rs
        │   └── resolver.rs           # Type resolution helpers
        └── tests/
```

## Refactoring Phases

### Phase 1: Extract Type System (Week 1)

**Goal**: Move type definitions and formatting to separate modules

**Files to create**:
- `crates/gravity-go/src/types.rs`:
  - `GoType` enum
  - `GoResult` enum  
  - `Operand` enum
- `crates/gravity-go/src/formatter.rs`:
  - `FormatInto` implementations
  - Comment generation helpers

**Benefits**:
- Can unit test type formatting independently
- Clear separation of Go-specific logic

**Example test**:
```rust
#[test]
fn test_optional_type_formatting() {
    let typ = GoType::ValueOrOk(Box::new(GoType::Uint32));
    let mut tokens = Tokens::new();
    typ.format_into(&mut tokens);
    assert_eq!(tokens.to_string(), "uint32, bool");
}
```

### Phase 2: Extract Instruction Handling (Week 1-2)

**Goal**: Separate instruction implementations from main logic

**Structure**:
```rust
// crates/gravity-codegen/src/instructions/mod.rs
pub trait InstructionHandler {
    fn handle(
        &mut self,
        instruction: &Instruction,
        context: &mut GenerationContext,
    ) -> Result<Vec<Operand>, Error>;
}

// crates/gravity-codegen/src/instructions/option.rs
pub struct OptionLiftHandler;
impl InstructionHandler for OptionLiftHandler { ... }

// crates/gravity-codegen/src/instructions/record.rs  
pub struct RecordLiftHandler;
impl InstructionHandler for RecordLiftHandler { ... }
```

**Benefits**:
- Each instruction can be tested in isolation
- Easy to add new instructions
- Clear documentation per instruction

### Phase 3: Extract Bindings Generator (Week 2)

**Goal**: Separate the high-level binding generation from instruction handling

**Structure**:
```rust
// crates/gravity-codegen/src/bindings.rs
pub struct BindingsGenerator {
    output: Tokens<Go>,
    types: Vec<TypeDef>,
}

impl BindingsGenerator {
    pub fn new() -> Self { ... }
    pub fn add_type(&mut self, type_def: &TypeDef) { ... }
    pub fn add_function(&mut self, func: &Function) { ... }
    pub fn generate(self) -> String { ... }
}
```

**Benefits**:
- Can test type and function generation separately
- Mock different WIT configurations easily
- Reusable for different output formats

### Phase 4: Create Builder Pattern for Complex Types (Week 2-3)

**Goal**: Make complex type construction more testable and readable

**Example**:
```rust
// crates/gravity-codegen/src/builders.rs
pub struct StructBuilder {
    name: String,
    fields: Vec<(String, GoType)>,
}

impl StructBuilder {
    pub fn new(name: impl Into<String>) -> Self { ... }
    pub fn field(mut self, name: impl Into<String>, typ: GoType) -> Self { ... }
    pub fn build(self) -> Tokens<Go> { ... }
}

// Usage in tests:
let struct_tokens = StructBuilder::new("Person")
    .field("name", GoType::String)
    .field("age", GoType::Uint32)
    .field("email", GoType::Pointer(Box::new(GoType::String)))
    .build();
```

### Phase 5: Integration Layer (Week 3)

**Goal**: Create clean integration between components

**Structure**:
```rust
// crates/gravity-codegen/src/lib.rs
pub struct CodeGenerator {
    instruction_registry: InstructionRegistry,
    bindings_generator: BindingsGenerator,
    context: GenerationContext,
}

impl CodeGenerator {
    pub fn from_component(component: &[u8]) -> Result<Self, Error> { ... }
    pub fn generate_go(self) -> Result<String, Error> { ... }
}
```

## Testing Strategy

### Unit Tests

Each module should have comprehensive unit tests:

```rust
// crates/gravity-go/src/types.rs
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pointer_type_in_struct_field() {
        // Test that ValueOrOk becomes Pointer in struct context
    }

    #[test]
    fn test_multivalue_in_function_return() {
        // Test that ValueOrOk becomes tuple in function context
    }
}
```

### Integration Tests

```rust
// crates/gravity-codegen/tests/integration.rs
#[test]
fn test_optional_field_conversion() {
    let wit = r#"
        record person {
            name: string,
            email: option<string>,
        }
    "#;
    let generator = CodeGenerator::from_wit(wit).unwrap();
    let go_code = generator.generate_go().unwrap();
    assert!(go_code.contains("Email *string"));
}
```

### Property-Based Testing

Use `proptest` for complex scenarios:

```rust
proptest! {
    #[test]
    fn test_variant_name_prefixing(
        variant_name in "[a-z-]+",
        case_names in prop::collection::vec("[a-z-]+", 1..10)
    ) {
        // Test that all case names are properly prefixed
        // and no collisions occur
    }
}
```

## Migration Strategy

### Step 1: Create Module Structure (Day 1)
- Create new crate structure
- Set up Cargo.toml files
- Keep main.rs unchanged

### Step 2: Extract Without Breaking (Days 2-5)
- Copy (don't move) types to new modules
- Add `pub use` statements in main.rs
- Ensure everything still compiles

### Step 3: Gradual Migration (Week 2)
- Replace main.rs code with calls to modules
- One instruction at a time
- Keep tests passing throughout

### Step 4: Remove Duplication (Week 3)
- Delete old code from main.rs
- Update imports
- Final cleanup

## Success Metrics

1. **Code Coverage**: Achieve >80% unit test coverage
2. **Compilation Time**: No significant increase in build time
3. **Performance**: No regression in code generation speed
4. **Modularity**: Each crate under 500 lines
5. **Documentation**: Every public API documented

## Benefits

### Immediate Benefits
- **Easier Testing**: Can test individual components
- **Better Organization**: Clear module boundaries
- **Faster Development**: Parallel work on different modules
- **Easier Onboarding**: New contributors can understand smaller pieces

### Long-term Benefits
- **Extensibility**: Easy to add new output formats (not just Go)
- **Reusability**: Other tools can use gravity as a library
- **Maintainability**: Bugs isolated to specific modules
- **Performance**: Can optimize hot paths independently

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| Breaking existing functionality | High | Incremental migration with tests at each step |
| Over-engineering | Medium | Start simple, refactor as needed |
| Lost context during refactor | Medium | Extensive comments and documentation |
| Merge conflicts | Low | Coordinate refactoring in focused sprints |

## Example: Refactored Option Handling

Before (in main.rs):
```rust
// 1000+ lines into the file...
Instruction::OptionLift { payload, .. } => {
    // 30 lines of inline code
}
```

After:
```rust
// crates/gravity-codegen/src/instructions/option.rs
pub struct OptionLift;

impl OptionLift {
    pub fn handle(context: &mut GenContext, payload: &Type) -> Result<()> {
        // Same logic, but testable
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_option_lift_generates_tuple() {
        let mut context = GenContext::mock();
        OptionLift::handle(&mut context, &Type::String).unwrap();
        assert_eq!(context.operands.last(), Some(&Operand::MultiValue(...)));
    }
}
```

## Timeline

- **Week 1**: Extract types and formatting
- **Week 2**: Extract instruction handlers
- **Week 3**: Create integration layer and migrate
- **Week 4**: Documentation and cleanup

## Next Steps

1. Review and approve this plan
2. Create feature branch for refactoring
3. Set up new crate structure
4. Begin Phase 1 extraction
5. Write initial unit tests as we go

## Appendix: Module Responsibilities

### gravity-go
- Go language specifics
- Identifier formatting (public/private/local)
- Import management
- Go type representations

### gravity-codegen
- Instruction handling
- Function generation
- Bindgen trait implementation
- Context management

### gravity-wit
- WIT parsing helpers
- Type resolution
- Component model understanding
- Size calculations

### cmd/gravity
- CLI argument parsing
- File I/O
- Error reporting to user
- Integration of all modules