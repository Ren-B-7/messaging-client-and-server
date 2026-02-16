#!/bin/bash
# Tower Implementation Verification Script

echo "🔍 Tower Implementation Verification"
echo "===================================="
echo ""

# Check if tower_middle exists
if [ -d "server/src/tower_middle" ]; then
    echo "✅ tower_middle directory exists"
else
    echo "❌ tower_middle directory NOT found"
    echo "   Run: cp -r tower_middle server/src/"
    exit 1
fi

# Check if mod.rs exists in tower_middle
if [ -f "server/src/tower_middle/mod.rs" ]; then
    echo "✅ tower_middle/mod.rs exists"
else
    echo "❌ tower_middle/mod.rs NOT found"
    exit 1
fi

# Check if security/mod.rs has pub mod
if grep -q "pub mod ip_filter" server/src/security/mod.rs 2>/dev/null; then
    echo "✅ security/mod.rs has public modules"
else
    echo "❌ security/mod.rs needs to be updated with 'pub mod'"
    echo "   Replace with: security_mod_fixed.rs"
    exit 1
fi

# Check if main.rs has tower_middle module
if grep -q "mod tower_middle" server/src/main.rs 2>/dev/null; then
    echo "✅ main.rs imports tower_middle module"
else
    echo "⚠️  main.rs doesn't import tower_middle yet"
    echo "   Add: mod tower_middle;"
fi

# Check Cargo.toml for tower dependencies
if grep -q "tower" server/Cargo.toml 2>/dev/null; then
    echo "✅ Tower dependencies in Cargo.toml"
else
    echo "❌ Tower dependencies NOT in Cargo.toml"
    echo "   Run: cargo add tower --features full"
    echo "   Run: cargo add tower-http --features trace,timeout"
    exit 1
fi

echo ""
echo "🎉 Verification Complete!"
echo ""
echo "Next steps:"
echo "1. Run: cd server && cargo check"
echo "2. If there are errors, check QUICK_FIX.md"
echo "3. Once compiling, update your connection loop to use middleware"
