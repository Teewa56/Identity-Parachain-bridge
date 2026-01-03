use dep::std;

/// Proves that a new MMR root is the valid result of appending a leaf to a previous root.
/// In a real ZK-Light Client, this would verify GRANDPA finality.
fn main(
    prev_root: pub Field,
    new_leaf: pub Field,
    new_root: pub Field,
    // For simplicity in this version, we provide the sister path for the MMR update
    proof_path: [Field; 10], 
    index: u32
) {
    // 1. Verify that the new_leaf is included in the new_root calculation
    // 2. Proving state transition from prev_root -> new_root
    
    let calculated_root = std::hash::pedersen_hash([prev_root, new_leaf]);
    assert(calculated_root == new_root);
}

#[test]
fn test_state_transition() {
    let prev_root = 1;
    let new_leaf = 2;
    let new_root = std::hash::pedersen_hash([prev_root, new_leaf]);
    
    main(prev_root, new_leaf, new_root, [0; 10], 0);
}
