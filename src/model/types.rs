use async_lsp::lsp_types::Range;

/// Represents a single, isolated block of raw comment text extracted from the
/// schema.
///
/// This structure holds both the cleaned textual content and its original
/// physical boundaries on disk, allowing the language server to accurately map
/// comments to their respective elements.
#[derive(Debug, Clone)]
pub struct CommentBlock {
    /// The processed inner text of the comment, stripped of syntax tokens
    /// (`//`, `/*`, `*/`)  and normalized for markdown presentation.
    pub text: String,

    /// The exact text range spanning the full physical presence of the comment
    /// token in the source file.
    pub range: Range,
}

/// Houses the structural metadata common to every unique element extracted into
/// the metamodel.
///
/// This container decouples the generic syntax positioning and documentation
/// properties from the specific semantic behavior defined inside
/// [`ElementKind`].
#[derive(Debug, Clone, Default)]
pub struct ElementMeta {
    /// The local relative name identifier of the element (e.g., `"BookItem"`,
    /// `"isbn"`, `"CatalogService"`).
    pub name: String,

    /// The comprehensive semantic range encompassing the entire block wrapper
    /// of the element from start to finish.
    pub range: Range,

    /// The specific precise text range covering strictly the local name token
    /// identifier of this element.
    pub selection_range: Range,

    /// A collection of chronological adjacent comment blocks serving as the
    /// active documentation for this element.
    pub documentation: Vec<CommentBlock>,
}

/// Specifies the explicit iteration or optional presence strategy applied to a
/// protobuf field descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CardinalityKind {
    /// Indicates a field that can be repeated zero or more times (an array/list
    /// structure).
    ///
    /// # Examples
    ///
    /// ```protobuf
    /// repeated string tag_list = 3;
    /// ```
    Repeated,

    /// Indicates a field whose presence is tracked explicitly via presence
    /// bitmasks (standard optional field).
    ///
    /// # Examples
    ///
    /// ```protobuf
    /// optional string title = 2;
    /// ```
    Optional,

    /// Indicates a legacy strict presence constraint enforced by the
    /// serialization pipeline (proto2 validation only).
    ///
    /// # Examples
    ///
    /// ```protobuf
    /// required string isbn = 1;
    /// ```
    Required,
}

/// An error indicating that the provided string sequence could not be
/// successfully mapped into a valid [`CardinalityKind`] variant during string
/// parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseCardinalityError;

/// Represents the explicit cardinality label of a protobuf field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldCardinality {
    /// The parsed flavor of field presence or repetition strategy.
    pub kind: CardinalityKind,

    /// The precise syntactic range spanning strictly the cardinality keyword
    /// token (e.g., the bounds of the word `repeated` or `required`).
    pub range: Range,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamModifier {
    pub range: Range,
}

/// Represents a syntactic reference to a type name within a protobuf schema
/// (e.g., a field type, a `map` key/value type, or an RPC request/response
/// type).
///
/// This structure stores exclusively the clean semantic identifier of the
/// referenced type and its exact byte boundaries, isolating it from any
/// formatting labels or syntax modifiers like `repeated`, `optional`, or
/// `stream`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeReference {
    /// The clean semantic name of the referenced type (e.g., `"int64"`,
    /// `"BookRequest"`).
    pub name: String,

    /// The exact text range encompassing strictly the type identifier itself,
    /// excluding any adjacent cardinality tokens or streaming modifiers.
    pub range: Range,
}

/// Represents the specific semantic variant of a protobuf schema element,
/// housing its clean data structures, cross-references, and metadata flags.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ElementKind {
    /// An external file dependency declared via the `import` statement.
    ///
    /// # Examples
    ///
    /// ```protobuf
    /// import "google/protobuf/any.proto";
    /// ```
    Import {
        /// The raw string path of the imported protobuf file as specified in
        /// the source.
        ///
        /// # Example
        ///
        /// ```text
        /// google/protobuf/any.proto
        /// ```
        path: String,
    },

    /// A core container structured data type declaration (`message`).
    ///
    /// # Examples
    ///
    /// ```protobuf
    /// message BookItem {
    ///   string id = 1;
    /// }
    /// ```
    Message {
        /// The Fully Qualified Name of the message.
        ///
        /// # Example
        ///
        /// ```text
        /// com.book.BookItem
        /// ```
        fqn: String,

        /// Indicates whether the message is explicitly marked with the
        /// `deprecated = true` option.
        is_deprecated: bool,
    },

    /// A standard single-value terminal data field inside a `message` or a
    /// `oneof` container.
    ///
    /// # Examples
    ///
    /// ```protobuf
    /// repeated string tag_list = 3 [deprecated = true];
    /// ```
    Field {
        /// The Fully Qualified Name of this specific field.
        ///
        /// # Example
        ///
        /// ```text
        /// com.book.Book.tag_list
        /// ```
        fqn: String,

        /// The syntactic name and exact geometric range of the field's data type.
        type_ref: TypeReference,

        /// The explicit cardinality prefix modifier (`required`, `optional`,
        /// `repeated`), if present.
        cardinality: Option<FieldCardinality>,
        /// The unique numeric identifier assigned to the field within its
        /// parent message scope.
        ///
        /// # Example
        ///
        /// ```text
        /// 3
        /// ```
        tag: u32,

        /// Indicates whether the field is explicitly marked with the
        /// `deprecated = true` option.
        is_deprecated: bool,
    },

    /// An associative key-value pair map field container entity.
    ///
    /// # Examples
    ///
    /// ```protobuf
    /// map<string, string> attributes = 3;
    /// ```
    MapField {
        /// The Fully Qualified Name of this map field.
        ///
        /// # Example
        ///
        /// ```text
        /// com.book.Book.attributes
        /// ```
        fqn: String,

        /// The reference to the map's primitive key type (always an un-prefixed
        /// scalar token).
        key_type_ref: TypeReference,

        /// The reference to the map's values data type.
        value_type_ref: TypeReference,

        /// The unique numeric identifier assigned to the map field within its
        /// parent scope.
        ///
        /// # Example
        ///
        /// ```text
        /// 3
        /// ```
        tag: u32,

        /// Indicates whether the map field is explicitly marked with the
        /// `deprecated = true` option.
        is_deprecated: bool,
    },

    /// A specialized algebraic mutually exclusive option container block
    /// (`oneof`).
    ///
    /// # Examples
    ///
    /// ```protobuf
    /// oneof delivery_method {
    ///   string download_url = 4;
    /// }
    /// ```
    Oneof {
        /// The Fully Qualified Name of the oneof block wrapper itself.
        ///
        /// # Example
        ///
        /// ```text
        /// com.book.Book.delivery_method
        /// ```
        fqn: String,
    },

    /// A terminal field constrained and grouped strictly inside an parent
    /// `oneof` context block.
    ///
    /// # Examples
    ///
    /// ```protobuf
    /// string download_url = 4; // structural element inside a oneof scope
    /// ```
    OneofField {
        /// The Fully Qualified Name of this specific nested `oneof` field.
        ///
        /// # Example
        ///
        /// ```text
        /// com.book.Book.download_url
        /// ```
        fqn: String,

        /// The syntactic name and exact geometric range of the field's data
        /// type.
        type_ref: TypeReference,

        /// The unique numeric identifier assigned to the field within its
        /// parent scope.
        ///
        /// # Example
        ///
        /// ```text
        /// 4
        /// ```
        tag: u32,

        /// Indicates whether the field is explicitly marked with the
        /// `deprecated = true` option.
        is_deprecated: bool,
    },

    /// An enumerated scalar discrete value category definition wrapper
    /// (`enum`).
    ///
    /// # Examples
    ///
    /// ```protobuf
    /// enum State {
    ///   UNKNOWN = 0;
    /// }
    /// ```
    Enum {
        /// The Fully Qualified Name of the `enum` type block wrapper.
        ///
        /// # Example
        ///
        /// ```text
        /// com.book.State
        /// ```
        fqn: String,

        /// Indicates whether the `enum` wrapper is explicitly marked with the
        /// `deprecated = true` option.
        is_deprecated: bool,
    },

    /// An individual constant named integer value identifier mapped strictly
    /// inside a parent `enum` body scope.
    ///
    /// # Examples
    ///
    /// ```protobuf
    /// UNKNOWN = 0;
    /// ```
    EnumValue {
        /// The Fully Qualified Name of this constant value identifier.
        ///
        /// # Example
        ///
        /// ```text
        /// com.book.State.UNKNOWN
        /// ```
        fqn: String,

        /// The concrete numerical mapping code assigned to this literal
        /// identifier option.
        ///
        /// # Example
        ///
        /// ```text
        /// 0
        /// ```
        number: i32,

        /// Indicates whether the constant is explicitly marked with the
        /// `deprecated = true` option.
        is_deprecated: bool,
    },

    /// A logical group interface contract declaration containing network
    /// request methods (`service`).
    ///
    /// # Examples
    ///
    /// ```protobuf
    /// service CatalogService {
    ///   rpc GetBookItem(BookRequest) returns (BookItem);
    /// }
    /// ```
    Service {
        /// The Fully Qualified Name of the `service` wrapper block.
        ///
        /// # Example
        ///
        /// ```text
        /// com.book.CatalogService
        /// ```
        fqn: String,

        /// Indicates whether the `service` wrapper is explicitly marked with
        /// the `deprecated = true` option.
        is_deprecated: bool,
    },

    /// A remote procedural call interface endpoint method declaration mapped
    /// inside a `service` container.
    ///
    /// # Examples
    ///
    /// ```protobuf
    /// rpc GetBookStream(BookRequest) returns (stream BookItem);
    /// ```
    Rpc {
        /// The Fully Qualified Name of this explicit RPC invocation endpoint.
        ///
        /// # Example
        ///
        /// ```text
        /// com.book.CatalogService.GetBookStream
        /// ```
        fqn: String,

        /// The reference to the protobuf input type accepted by this remote
        /// invocation call.
        request_type_ref: TypeReference,

        /// The asynchronous streaming modifier flag and boundaries for the
        /// incoming request pipeline.
        request_stream: Option<StreamModifier>,

        /// The reference to the protobuf output type generated upon successful
        /// response pipeline completion.
        response_type_ref: TypeReference,

        /// The asynchronous streaming modifier flag and boundaries for the
        /// outgoing response pipeline.
        response_stream: Option<StreamModifier>,

        /// Indicates whether the RPC method is explicitly marked with the
        /// `deprecated = true` option.
        is_deprecated: bool,
    },
}

/// A normalized, index-backed semantic graph node representing a single
/// declared entity within a protobuf abstract syntax tree.
///
/// Instead of relying on heavy runtime pointer networks or heap-allocated
/// reference counting pointers (`Rc`/`Arc`), this model utilizes flat, safe
/// vector indexing via numerical IDs.
///
/// It acts as the ultimate structural building block for the language server's
/// pure-memory caching layers.
#[derive(Debug, Clone)]
pub struct ModelElement {
    /// The unique sequential identifier and position index of this specific
    /// element within the master flat vector registry of the parsed tree.
    pub id: usize,

    /// The unique numerical identifier of the parent container enclosing this
    /// element scope, or `None` if the element resides at the root level of the
    /// protobuf package namespace.
    pub parent_id: Option<usize>,

    /// The common structural metadata boundaries, selection ranges, and
    /// docstring buffers of this element.
    pub meta: ElementMeta,

    /// The specialized type descriptor enum housing flavor-specific data
    /// structures (e.g., fields, messages, RPC methods).
    pub kind: ElementKind,

    /// A chronological registry containing the numerical unique IDs of all
    /// elements syntactically declared and nested inside this container wrapper
    /// scope.
    pub children: Vec<usize>,
}

/// An immutable, ultra-lightweight entry representing a localized spatial
/// intersection point mapped directly inside the lookup index.
#[derive(Debug, Clone, Copy)]
pub struct SpatialEntry {
    /// The physical coordinates and line boundaries encompassing this token in
    /// the source document.
    pub range: Range,

    /// The absolute numeric unique ID referencing the target [`ModelElement`]
    /// mapped to these exact bounds.
    pub element_id: usize,
}
