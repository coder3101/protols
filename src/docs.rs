use std::{collections::HashMap, sync::LazyLock};

macro_rules! docmap_builtin {
    ($name:literal) => {
        ($name, include_str!(concat!("docs/builtin/", $name, ".md")))
    };
}

macro_rules! docmap_wellknown {
    ($name:literal) => {
        (
            concat!("google.protobuf.", $name),
            include_str!(concat!("docs/wellknown/", $name, ".md")),
        )
    };
}

pub static BUITIN: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        docmap_builtin!("int32"),
        docmap_builtin!("int64"),
        docmap_builtin!("uint32"),
        docmap_builtin!("uint64"),
        docmap_builtin!("sint32"),
        docmap_builtin!("sint64"),
        docmap_builtin!("fixed32"),
        docmap_builtin!("fixed64"),
        docmap_builtin!("sfixed32"),
        docmap_builtin!("sfixed64"),
        docmap_builtin!("float"),
        docmap_builtin!("double"),
        docmap_builtin!("string"),
        docmap_builtin!("bytes"),
        docmap_builtin!("bool"),
        docmap_builtin!("default"),
    ])
});

pub static WELLKNOWN: LazyLock<HashMap<&'static str, &'static str>> = LazyLock::new(|| {
    HashMap::from([
        docmap_wellknown!("Any"),
        docmap_wellknown!("Api"),
        docmap_wellknown!("BoolValue"),
        docmap_wellknown!("BytesValue"),
        docmap_wellknown!("DoubleValue"),
        docmap_wellknown!("Duration"),
        docmap_wellknown!("Empty"),
        docmap_wellknown!("Enum"),
        docmap_wellknown!("EnumValue"),
        docmap_wellknown!("Field"),
        docmap_wellknown!("Field.Cardinality"),
        docmap_wellknown!("Field.Kind"),
        docmap_wellknown!("FieldMask"),
        docmap_wellknown!("FloatValue"),
        docmap_wellknown!("Int32Value"),
        docmap_wellknown!("Int64Value"),
        docmap_wellknown!("ListValue"),
        docmap_wellknown!("Method"),
        docmap_wellknown!("Mixin"),
        docmap_wellknown!("NullValue"),
        docmap_wellknown!("Option"),
        docmap_wellknown!("SourceContext"),
        docmap_wellknown!("StringValue"),
        docmap_wellknown!("Struct"),
        docmap_wellknown!("Syntax"),
        docmap_wellknown!("Timestamp"),
        docmap_wellknown!("Type"),
        docmap_wellknown!("UInt32Value"),
        docmap_wellknown!("UInt64Value"),
        docmap_wellknown!("Value"),
    ])
});
