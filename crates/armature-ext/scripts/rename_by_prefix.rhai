// Prefix every exported symbol with "tpt_" to mark it as triaged.
//
// Run:
//   armature-cli script <binary> scripts/rename_by_prefix.rhai

for ex in exports {
    rename(ex.addr, "tpt_" + ex.name);
}
