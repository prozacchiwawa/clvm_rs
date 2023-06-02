use crate::allocator::{Allocator, NodePtr};
use crate::cost::{check_cost, Cost};
use crate::node::Node;
use crate::op_utils::{
    arg_count, atom, check_arg_count, int_atom, mod_group_order, new_atom_and_cost,
    number_to_scalar, MALLOC_COST_PER_BYTE,
};
use crate::reduction::{Reduction, Response};
use bls12_381::hash_to_curve::{ExpandMsgXmd, HashToCurve};
use bls12_381::{multi_miller_loop, G1Affine, G1Projective, G2Affine, G2Prepared, G2Projective};
use group::Group;
use std::ops::Neg;

// the same cost as point_add (aka g1_add)
const BLS_G1_SUBTRACT_BASE_COST: Cost = 101094;
const BLS_G1_SUBTRACT_COST_PER_ARG: Cost = 1343980;

const BLS_G1_MULTIPLY_BASE_COST: Cost = 705500;
const BLS_G1_MULTIPLY_COST_PER_BYTE: Cost = 10;

// this is the same cost as XORing the top bit (minus the heap allocation of the
// return value, which the operator is adding back)
const BLS_G1_NEGATE_BASE_COST: Cost = 1396 - 480;

// g2_add and g2_subtract have the same cost
const BLS_G2_ADD_BASE_COST: Cost = 80000;
const BLS_G2_ADD_COST_PER_ARG: Cost = 1950000;
const BLS_G2_SUBTRACT_BASE_COST: Cost = 80000;
const BLS_G2_SUBTRACT_COST_PER_ARG: Cost = 1950000;

const BLS_G2_MULTIPLY_BASE_COST: Cost = 2100000;
const BLS_G2_MULTIPLY_COST_PER_BYTE: Cost = 5;

// this is the same cost as XORing the top bit (minus the heap allocation of the
// return value, which the operator is adding back)
const BLS_G2_NEGATE_BASE_COST: Cost = 2164 - 960;

const BLS_MAP_TO_G1_BASE_COST: Cost = 195000;
const BLS_MAP_TO_G1_COST_PER_BYTE: Cost = 4;
const BLS_MAP_TO_G1_COST_PER_DST_BYTE: Cost = 4;

const BLS_MAP_TO_G2_BASE_COST: Cost = 815000;
const BLS_MAP_TO_G2_COST_PER_BYTE: Cost = 4;
const BLS_MAP_TO_G2_COST_PER_DST_BYTE: Cost = 4;

const BLS_PAIRING_BASE_COST: Cost = 3000000;
const BLS_PAIRING_COST_PER_ARG: Cost = 1200000;

const DST_G2: &[u8; 43] = b"BLS_SIG_BLS12381G2_XMD:SHA-256_SSWU_RO_AUG_";

pub fn op_bls_g1_subtract(a: &mut Allocator, input: NodePtr, max_cost: Cost) -> Response {
    let args = Node::new(a, input);
    let mut cost = BLS_G1_SUBTRACT_BASE_COST;
    check_cost(a, cost, max_cost)?;
    let mut total: G1Projective = G1Projective::identity();
    let mut is_first = true;
    for arg in &args {
        let point = a.g1(arg.node)?;
        cost += BLS_G1_SUBTRACT_COST_PER_ARG;
        check_cost(a, cost, max_cost)?;
        if is_first {
            total = point;
        } else {
            total -= point;
        };
        is_first = false;
    }
    Ok(Reduction(
        cost + 48 * MALLOC_COST_PER_BYTE,
        a.new_g1(total)?,
    ))
}

pub fn op_bls_g1_multiply(a: &mut Allocator, input: NodePtr, max_cost: Cost) -> Response {
    let args = Node::new(a, input);
    check_arg_count(&args, 2, "g1_multiply")?;

    let mut cost = BLS_G1_MULTIPLY_BASE_COST;
    check_cost(a, cost, max_cost)?;

    let mut total = a.g1(args.first()?.node)?;
    let args = args.rest()?;
    let (scalar, scalar_len) = int_atom(args.first()?, "g1_multiply")?;
    cost += scalar_len as Cost * BLS_G1_MULTIPLY_COST_PER_BYTE;
    check_cost(a, cost, max_cost)?;

    total *= number_to_scalar(mod_group_order(scalar));

    Ok(Reduction(
        cost + 48 * MALLOC_COST_PER_BYTE,
        a.new_g1(total)?,
    ))
}

pub fn op_bls_g1_negate(a: &mut Allocator, input: NodePtr, _max_cost: Cost) -> Response {
    let args = Node::new(a, input);
    check_arg_count(&args, 1, "g1_negate")?;

    // we don't validate the point. We may want to soft fork-in validating the
    // point once the allocator preserves native representation of points
    let blob = atom(args.first()?, "G1 atom")?;
    if blob.len() != 48 {
        return args.first()?.err("atom is not G1 size, 48 bytes");
    }
    if (blob[0] & 0xe0) == 0xc0 {
        // This is compressed infinity. negating it is a no-op
        // we can just pass through the same atom as we received. We'll charge
        // the allocation cost anyway, for consistency
        Ok(Reduction(
            BLS_G1_NEGATE_BASE_COST + 48 * MALLOC_COST_PER_BYTE,
            args.first()?.node,
        ))
    } else {
        let mut blob: [u8; 48] = blob.try_into().unwrap();
        blob[0] ^= 0x20;
        new_atom_and_cost(a, BLS_G1_NEGATE_BASE_COST, &blob)
    }
}

pub fn op_bls_g2_add(a: &mut Allocator, input: NodePtr, max_cost: Cost) -> Response {
    let args = Node::new(a, input);
    let mut cost = BLS_G2_ADD_BASE_COST;
    check_cost(a, cost, max_cost)?;
    let mut total: G2Projective = G2Projective::identity();
    for arg in &args {
        let point = a.g2(arg.node)?;
        cost += BLS_G2_ADD_COST_PER_ARG;
        check_cost(a, cost, max_cost)?;
        total += &point;
    }
    Ok(Reduction(
        cost + 96 * MALLOC_COST_PER_BYTE,
        a.new_g2(total)?,
    ))
}

pub fn op_bls_g2_subtract(a: &mut Allocator, input: NodePtr, max_cost: Cost) -> Response {
    let args = Node::new(a, input);
    let mut cost = BLS_G2_SUBTRACT_BASE_COST;
    check_cost(a, cost, max_cost)?;
    let mut total: G2Projective = G2Projective::identity();
    let mut is_first = true;
    for arg in &args {
        let point = a.g2(arg.node)?;
        cost += BLS_G2_SUBTRACT_COST_PER_ARG;
        check_cost(a, cost, max_cost)?;
        if is_first {
            total = point;
        } else {
            total -= point;
        };
        is_first = false;
    }
    Ok(Reduction(
        cost + 96 * MALLOC_COST_PER_BYTE,
        a.new_g2(total)?,
    ))
}

pub fn op_bls_g2_multiply(a: &mut Allocator, input: NodePtr, max_cost: Cost) -> Response {
    let args = Node::new(a, input);
    check_arg_count(&args, 2, "g2_multiply")?;

    let mut cost = BLS_G2_MULTIPLY_BASE_COST;
    check_cost(a, cost, max_cost)?;

    let mut total = a.g2(args.first()?.node)?;
    let args = args.rest()?;
    let (scalar, scalar_len) = int_atom(args.first()?, "g2_multiply")?;
    cost += scalar_len as Cost * BLS_G2_MULTIPLY_COST_PER_BYTE;
    check_cost(a, cost, max_cost)?;

    total *= number_to_scalar(mod_group_order(scalar));

    Ok(Reduction(
        cost + 96 * MALLOC_COST_PER_BYTE,
        a.new_g2(total)?,
    ))
}

pub fn op_bls_g2_negate(a: &mut Allocator, input: NodePtr, _max_cost: Cost) -> Response {
    let args = Node::new(a, input);
    check_arg_count(&args, 1, "g2_negate")?;

    // we don't validate the point. We may want to soft fork-in validating the
    // point once the allocator preserves native representation of points
    let blob = atom(args.first()?, "G2 atom")?;
    if blob.len() != 96 {
        return args.first()?.err("atom is not G2 size, 96 bytes");
    }
    if (blob[0] & 0xe0) == 0xc0 {
        // This is compressed infinity. negating it is a no-op
        // we can just pass through the same atom as we received. We'll charge
        // the allocation cost anyway, for consistency
        Ok(Reduction(
            BLS_G2_NEGATE_BASE_COST + 96 * MALLOC_COST_PER_BYTE,
            args.first()?.node,
        ))
    } else {
        let mut blob: [u8; 96] = blob.try_into().unwrap();
        blob[0] ^= 0x20;
        new_atom_and_cost(a, BLS_G2_NEGATE_BASE_COST, &blob)
    }
}

pub fn op_bls_map_to_g1(a: &mut Allocator, input: NodePtr, max_cost: Cost) -> Response {
    let args = Node::new(a, input);
    let ac = arg_count(&args, 2);
    if !(1..=2).contains(&ac) {
        return args.err("g_1_map takes exactly 1 or 2 arguments");
    }
    let mut cost: Cost = BLS_MAP_TO_G1_BASE_COST;
    check_cost(a, cost, max_cost)?;

    let msg = atom(args.first()?, "g1_map")?;
    let args = args.rest()?;
    cost += msg.len() as Cost * BLS_MAP_TO_G1_COST_PER_BYTE;
    check_cost(a, cost, max_cost)?;

    let dst: &[u8] = if ac == 2 {
        atom(args.first()?, "g1_map")?
    } else {
        b"BLS_SIG_BLS12381G1_XMD:SHA-256_SSWU_RO_AUG_"
    };

    cost += dst.len() as Cost * BLS_MAP_TO_G1_COST_PER_DST_BYTE;
    check_cost(a, cost, max_cost)?;

    let point = <G1Projective as HashToCurve<ExpandMsgXmd<sha2::Sha256>>>::hash_to_curve(msg, dst);
    Ok(Reduction(
        cost + 48 * MALLOC_COST_PER_BYTE,
        a.new_g1(point)?,
    ))
}

pub fn op_bls_map_to_g2(a: &mut Allocator, input: NodePtr, max_cost: Cost) -> Response {
    let args = Node::new(a, input);
    let ac = arg_count(&args, 2);
    if !(1..=2).contains(&ac) {
        return args.err("g2_map takes exactly 1 or 2 arguments");
    }
    let mut cost: Cost = BLS_MAP_TO_G2_BASE_COST;
    check_cost(a, cost, max_cost)?;

    let msg = atom(args.first()?, "g2_map")?;
    let args = args.rest()?;
    cost += msg.len() as Cost * BLS_MAP_TO_G2_COST_PER_BYTE;

    let dst: &[u8] = if ac == 2 {
        atom(args.first()?, "g2_map")?
    } else {
        DST_G2
    };

    cost += dst.len() as Cost * BLS_MAP_TO_G2_COST_PER_DST_BYTE;
    check_cost(a, cost, max_cost)?;

    let point = <G2Projective as HashToCurve<ExpandMsgXmd<sha2::Sha256>>>::hash_to_curve(msg, dst);
    Ok(Reduction(
        cost + 96 * MALLOC_COST_PER_BYTE,
        a.new_g2(point)?,
    ))
}

// This operator takes a variable number of G1 and G2 points. The points must
// come in pairs (as a "flat" argument list).
// It performs a low-level pairing operation of the (G1, G2)-pairs
// and returns a boolean indicating whether the resulting Gt point is the
// identity or not. True means identity False otherwise. This is a building
// block for signature verification.
pub fn op_bls_pairing_identity(a: &mut Allocator, input: NodePtr, max_cost: Cost) -> Response {
    let mut cost = BLS_PAIRING_BASE_COST;
    check_cost(a, cost, max_cost)?;
    let mut items = Vec::<(G1Affine, G2Prepared)>::new();

    let mut args = Node::new(a, input);
    while !args.nullp() {
        cost += BLS_PAIRING_COST_PER_ARG;
        check_cost(a, cost, max_cost)?;
        let g1 = a.g1(args.first()?.node)?;
        args = args.rest()?;
        let g2 = a.g2(args.first()?.node)?;
        args = args.rest()?;
        items.push((g1.into(), G2Prepared::from(G2Affine::from(g2))));
    }

    let mut item_refs = Vec::<(&G1Affine, &G2Prepared)>::new();
    for (p, q) in &items {
        item_refs.push((p, q));
    }
    let identity: bool = multi_miller_loop(&item_refs)
        .final_exponentiation()
        .is_identity()
        .into();
    if !identity {
        Node::new(a, input).err("bls_pairing_identity failed")
    } else {
        Ok(Reduction(cost, a.null()))
    }
}

// expects: G2 G1 msg G1 msg ...
// G2 is the signature
// G1 is a public key
// the G1 and its corresponding message must be passed in pairs.
pub fn op_bls_verify(a: &mut Allocator, input: NodePtr, max_cost: Cost) -> Response {
    let mut cost = BLS_PAIRING_BASE_COST;
    check_cost(a, cost, max_cost)?;

    let args = Node::new(a, input);

    // the first argument is the signature
    let signature = a.g2(args.first()?.node)?;

    // followed by a variable number of (G1, msg)-pairs (as a flat list)
    let mut args = args.rest()?;

    let mut items = Vec::<(G1Affine, G2Prepared)>::new();
    while !args.nullp() {
        let pk = a.g1(args.first()?.node)?;
        args = args.rest()?;
        let msg = atom(args.first()?, "bls_verify message")?;
        args = args.rest()?;

        cost += BLS_PAIRING_COST_PER_ARG;
        cost += msg.len() as Cost * BLS_MAP_TO_G2_COST_PER_BYTE;
        cost += DST_G2.len() as Cost * BLS_MAP_TO_G2_COST_PER_DST_BYTE;
        check_cost(a, cost, max_cost)?;

        // The AUG scheme requires prepending the public key to the signed
        // message
        let mut prepended_msg = G1Affine::from(pk).to_compressed().to_vec();
        prepended_msg.extend_from_slice(msg);

        let point = <G2Projective as HashToCurve<ExpandMsgXmd<sha2::Sha256>>>::hash_to_curve(
            prepended_msg,
            DST_G2,
        );
        items.push((pk.into(), G2Prepared::from(G2Affine::from(point))));
    }

    items.push((
        G1Affine::generator().neg(),
        G2Prepared::from(G2Affine::from(signature)),
    ));

    let mut item_refs = Vec::<(&G1Affine, &G2Prepared)>::new();
    for (p, q) in &items {
        item_refs.push((p, q));
    }
    let identity: bool = multi_miller_loop(&item_refs)
        .final_exponentiation()
        .is_identity()
        .into();
    if !identity {
        Node::new(a, input).err("bls_verify failed")
    } else {
        Ok(Reduction(cost, a.null()))
    }
}