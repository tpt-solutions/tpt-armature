// Heuristic crypto-rotation: flag exported symbols whose name suggests a
// cryptographic routine and prefix them with "crypto_".
//
// Run:
//   tpt-armature-cli script <binary> scripts/find_crypto.rhai

for ex in exports {
    if ex.name.contains("aes")
        || ex.name.contains("sha")
        || ex.name.contains("crypt")
        || ex.name.contains("md5")
        || ex.name.contains("des")
        || ex.name.contains("rsa") {
        print("crypto symbol: " + ex.name + " @0x" + ex.addr.to_string());
        rename(ex.addr, "crypto_" + ex.name);
    }
}
