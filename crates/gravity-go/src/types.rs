#[derive(Debug, Clone, PartialEq)]
pub enum GoType {
    Bool,
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Int8,
    Int16,
    Int32,
    Int64,
    Float32,
    Float64,
    String,
    Error,
    Interface,
    Pointer(Box<GoType>),
    ValueOrOk(Box<GoType>),
    ValueOrError(Box<GoType>),
    Slice(Box<GoType>),
    UserDefined(String),
    Nothing,
}

impl GoType {
    /// Returns true if this type needs post-return cleanup (cabi_post_* function)
    ///
    /// According to the Component Model Canonical ABI specification, cleanup is needed
    /// for types that allocate memory in the guest's linear memory when being returned.
    ///
    /// Types that need cleanup:
    /// - Strings (allocate memory for the string data)
    /// - Lists/Slices (allocate memory for the list elements)
    /// - Records with fields that need cleanup
    /// - Variants with cases that need cleanup
    /// - Results with values that need cleanup
    ///
    /// Types that don't need cleanup:
    /// - Primitive numeric types (passed by value)
    /// - Booleans (passed by value)
    /// - Empty results
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
            | GoType::Float64
            | GoType::Interface
            | GoType::Nothing => false,

            // String always needs cleanup
            GoType::String => true,

            // Lists always need cleanup
            GoType::Slice(_) => true,

            // Pointers need cleanup if the inner type does
            GoType::Pointer(inner) => inner.needs_cleanup(),

            // ValueOrOk and ValueOrError need cleanup if the inner type does
            GoType::ValueOrOk(inner) | GoType::ValueOrError(inner) => inner.needs_cleanup(),

            // Error types typically need cleanup (they contain strings)
            GoType::Error => true,

            // User-defined types might need cleanup (conservatively return true)
            // In practice, we'd need to look up the actual type definition
            GoType::UserDefined(_) => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GoResult {
    Empty,
    Anon(GoType),
}

impl GoResult {
    /// Returns true if this result type needs post-return cleanup
    pub fn needs_cleanup(&self) -> bool {
        match self {
            GoResult::Empty => false,
            GoResult::Anon(typ) => typ.needs_cleanup(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Operand {
    Literal(String),
    SingleValue(String),
    MultiValue((String, String)),
}

impl Operand {
    /// Returns the primary value of the operand (for single values and literals)
    /// or the first value of a multi-value tuple
    pub fn as_string(&self) -> String {
        match self {
            Operand::Literal(s) => s.clone(),
            Operand::SingleValue(s) => s.clone(),
            Operand::MultiValue((s1, _)) => s1.clone(),
        }
    }
}
