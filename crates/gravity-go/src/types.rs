/// Represents a Go type in the code generation system.
///
/// This enum covers all the basic Go types as well as special types
/// used for WebAssembly Component Model interop.
#[derive(Debug, Clone, PartialEq)]
pub enum GoType {
    /// Boolean type
    Bool,
    /// Unsigned 8-bit integer
    Uint8,
    /// Unsigned 16-bit integer
    Uint16,
    /// Unsigned 32-bit integer
    Uint32,
    /// Unsigned 64-bit integer
    Uint64,
    /// Signed 8-bit integer
    Int8,
    /// Signed 16-bit integer
    Int16,
    /// Signed 32-bit integer
    Int32,
    /// Signed 64-bit integer
    Int64,
    /// 32-bit floating point
    Float32,
    /// 64-bit floating point
    Float64,
    /// String type
    String,
    /// Error type (represents Result<None, String>)
    Error,
    /// Interface type (for variants/discriminated unions)
    Interface,
    /// Pointer to another type
    Pointer(Box<GoType>),
    /// Result type with Ok value
    ValueOrOk(Box<GoType>),
    /// Result type with Error value
    ValueOrError(Box<GoType>),
    /// Slice/array of another type
    Slice(Box<GoType>),
    /// User-defined type (records, enums, type aliases)
    UserDefined(String),
    /// Represents no value/void
    Nothing,
}

impl GoType {
    /// Returns true if this type needs post-return cleanup (cabi_post_* function)
    ///
    /// According to the Component Model Canonical ABI specification, cleanup is needed
    /// for types that allocate memory in the guest's linear memory when being returned.
    ///
    /// Types that need cleanup:
    /// - Strings: allocate memory for the string data
    /// - Lists/Slices: allocate memory for the array data
    /// - Types containing the above (recursively)
    ///
    /// Types that DON'T need cleanup:
    /// - Primitives (bool, integers, floats): passed by value
    /// - Enums: represented as integers
    ///
    /// Limitations:
    /// - For UserDefined types (records, type aliases), we can't determine here if they
    ///   contain strings/lists without the full type definition, so we're conservative
    /// - A perfect implementation would recursively check record fields, but that would
    ///   require passing the Resolve context here
    pub fn needs_cleanup(&self) -> bool {
        match self {
            // Primitive types don't need cleanup
            GoType::Bool
            | GoType::Uint8
            | GoType::Uint16
            | GoType::Uint32
            | GoType::Uint64
            | GoType::Int8
            | GoType::Int16
            | GoType::Int32
            | GoType::Int64
            | GoType::Float32
            | GoType::Float64 => false,

            // String and slices allocate memory and need cleanup
            GoType::String | GoType::Slice(_) => true,

            // Complex types need cleanup if their inner types do
            GoType::ValueOrOk(inner) => inner.needs_cleanup(),

            // The inner type of `Err` is always a String so it requires cleanup
            // TODO(#91): Store the error type to check both inner types.
            GoType::ValueOrError(_) => true,

            // Interfaces (variants) might need cleanup (conservative approach)
            GoType::Interface => true,

            // User-defined types (records, enums, type aliases) need cleanup if they
            // contain strings or other allocated types. Since we don't have access to
            // the type definition here, we must be conservative and assume they might.
            //
            // This means we might generate unnecessary cleanup calls for:
            // - Enums (which are just integers)
            // - Records containing only primitives
            // - Type aliases to primitives
            //
            // TODO(#92): Improve this by either:
            // 1. Passing the Resolve context to check actual type definitions
            // 2. Tracking cleanup requirements during type resolution
            // 3. Using a different representation that carries this information
            GoType::UserDefined(_) => true,

            // Error is actually Result<None, String> - strings need cleanup!
            GoType::Error => true,

            // Nothing represents no value, so no cleanup needed
            GoType::Nothing => false,

            // A pointer probably needs cleanup, not sure?
            GoType::Pointer(_) => true,
        }
    }
}

/// Represents a Go function result type.
///
/// Can be either empty (no return value) or an anonymous type.
/// Used for modeling function returns in the generated Go code.
#[derive(Debug, Clone, PartialEq)]
pub enum GoResult {
    /// No return value
    Empty,
    /// Anonymous return type
    Anon(GoType),
}

impl GoResult {
    /// Returns true if this result type needs post-return cleanup.
    ///
    /// Delegates to the underlying type's cleanup requirements.
    /// Empty results don't need cleanup as they represent no value.
    ///
    /// # Returns
    /// `true` if cleanup is needed, `false` otherwise.
    pub fn needs_cleanup(&self) -> bool {
        match self {
            GoResult::Empty => false,
            GoResult::Anon(typ) => typ.needs_cleanup(),
        }
    }
}

/// Represents an operand in Go code generation.
///
/// Operands can be literals, single values (variables), or multi-value tuples
/// (used for functions returning multiple values).
#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    /// A literal value (e.g., "0", "true", "\"hello\"")
    Literal(String),
    /// A single variable or expression
    SingleValue(String),
    /// A tuple of two values (for multi-value returns)
    MultiValue((String, String)),
}

impl Operand {
    /// Returns the primary value of the operand.
    ///
    /// For single values and literals, returns the value itself.
    /// For multi-value tuples, returns the first value.
    ///
    /// # Returns
    /// A string representation of the primary value.
    pub fn as_string(&self) -> String {
        match self {
            Operand::Literal(s) => s.clone(),
            Operand::SingleValue(s) => s.clone(),
            Operand::MultiValue((s1, _)) => s1.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_needs_cleanup_primitives() {
        // Primitives don't need cleanup
        assert!(!GoType::Bool.needs_cleanup());
        assert!(!GoType::Uint8.needs_cleanup());
        assert!(!GoType::Uint16.needs_cleanup());
        assert!(!GoType::Uint32.needs_cleanup());
        assert!(!GoType::Uint64.needs_cleanup());
        assert!(!GoType::Int8.needs_cleanup());
        assert!(!GoType::Int16.needs_cleanup());
        assert!(!GoType::Int32.needs_cleanup());
        assert!(!GoType::Int64.needs_cleanup());
        assert!(!GoType::Float32.needs_cleanup());
        assert!(!GoType::Float64.needs_cleanup());
    }

    #[test]
    fn test_needs_cleanup_allocated_types() {
        // These types allocate memory and need cleanup
        assert!(GoType::String.needs_cleanup());
        assert!(GoType::Slice(Box::new(GoType::Uint8)).needs_cleanup());
        assert!(GoType::Slice(Box::new(GoType::String)).needs_cleanup());
        assert!(GoType::Error.needs_cleanup());
    }

    #[test]
    fn test_needs_cleanup_complex_types() {
        // ValueOrOk depends on inner type
        assert!(!GoType::ValueOrOk(Box::new(GoType::Uint32)).needs_cleanup());
        assert!(GoType::ValueOrOk(Box::new(GoType::String)).needs_cleanup());

        // ValueOrError always needs cleanup (contains error which is a string)
        assert!(GoType::ValueOrError(Box::new(GoType::Uint32)).needs_cleanup());
        assert!(GoType::ValueOrError(Box::new(GoType::String)).needs_cleanup());

        // Pointers always need cleanup (conservative approach)
        assert!(GoType::Pointer(Box::new(GoType::Uint32)).needs_cleanup());
        assert!(GoType::Pointer(Box::new(GoType::String)).needs_cleanup());
    }

    #[test]
    fn test_needs_cleanup_special_types() {
        assert!(GoType::Interface.needs_cleanup()); // Conservative
        assert!(GoType::UserDefined("MyType".to_string()).needs_cleanup()); // Conservative
        assert!(!GoType::Nothing.needs_cleanup()); // No value, no cleanup
    }

    #[test]
    fn test_go_result_needs_cleanup() {
        assert!(!GoResult::Empty.needs_cleanup());
        assert!(!GoResult::Anon(GoType::Uint32).needs_cleanup());
        assert!(GoResult::Anon(GoType::String).needs_cleanup());
        assert!(GoResult::Anon(GoType::Slice(Box::new(GoType::Uint8))).needs_cleanup());
    }
}
