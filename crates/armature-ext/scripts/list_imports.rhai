// List every imported symbol, grouped by the library that provides it.
//
// Run:
//   armature-cli script <binary> scripts/list_imports.rhai

for imp in imports {
    print("import: " + imp.dll + "!" + imp.name);
}
