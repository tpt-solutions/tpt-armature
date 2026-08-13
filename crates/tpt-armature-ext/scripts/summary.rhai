// Print a quick triage summary of the loaded binary.
//
// Run:
//   tpt-armature-cli script <binary> scripts/summary.rhai

print("format       : " + format);
print("arch         : " + arch);
print("entry        : 0x" + entry.to_string());
print("instructions : " + instruction_count.to_string());
print("functions    : " + function_count.to_string());
print("exports      : " + exports.len().to_string());
print("xref targets : " + symbol_xrefs.len().to_string());
