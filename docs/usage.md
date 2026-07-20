# Usage

Protols offers a rich set of features to enhance your `.proto` file editing experience.

## Code Completion

**Protols** offers intelligent autocompletion for messages, enums, and proto3 keywords within the current package. Simply start typing, and Protols will suggest valid completions.

## Diagnostics

Syntax errors are caught by the tree-sitter parser, which highlights issues directly in your editor. More advanced error reporting, is done by `protoc` which runs after a file saved. You must have `protoc` installed and added to your path or you can specify its path in the configuration above

## Code Formatting

Format your `.proto` files using `clang-format`. To customize the formatting style, add a `.clang-format` file to the root of your project. Both document and range formatting are supported.

## Workspace Symbols

Protols implements workspace symbol capabilities allowing you to search for symbols across workspace or list them, including nested symbols such as messages and enums. This allows for easy navigation and reference across workspace.

## Document Symbols

Protols provides a list of symbols in the current document, including nested symbols such as messages and enums. This allows for easy navigation and reference.

## Go to Definition

Jump directly to the definition of any custom symbol or imports, including those in other files or packages. This feature works across package boundaries.

## Hover Information

Hover over any symbol or imports to get detailed documentation and comments associated with it. This works seamlessly across different packages and namespaces.

## Rename Symbols

Rename symbols like messages, enums, services and RPC methods, and propagate the changes throughout the codebase. Rename also works when invoked on a type reference (e.g. the request or response type of an `rpc`) — the LSP pivots to the declaration and applies the rename from there. Field names, oneof names, and enum values can also be renamed at their declaration site (single-site rename, since they aren't referenced as types from other `.proto` files).

When an `rpc` follows the `rpc <Name>(<Name>Request) returns (<Name>Response)` convention from the [Google API design guide](https://google.aip.dev/) (AIPs 131–136), renaming any one of the three triggers a chained rename of the other two — but only when (a) the matching message name follows the convention exactly, (b) the request/response is used by exactly one rpc in the workspace, and (c) the user's new name preserves the convention. If any check fails, only the symbol the user invoked rename on is renamed.

## Find References

Find all references to user-defined types like messages or enums. Nested fields are fully supported, making it easier to track symbol usage across your project.
