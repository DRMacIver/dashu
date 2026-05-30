// Stateful equivalence test: dashu_int::IBig vs num_bigint::BigInt.
//
// Drives both representations through the same sequence of arithmetic /
// bitwise / conversion operations, asserting their string forms stay equal
// after every step. Designed to catch correctness regressions introduced by
// performance work, particularly around the inline-vs-heap representation
// boundary in dashu's `Repr`.
//
// Gated behind the `hegel-tests` feature so the default `cargo test` doesn't
// pull in hegeltest (requires Rust edition 2024). Run with:
//   cargo test -p dashu-int --features hegel-tests --test stateful_ibig

use dashu_int::ops::{Abs, BitTest};
use dashu_int::IBig;
use hegel::generators as gs;
use hegel::stateful::{variables, Variables};
use hegel::TestCase;
use num_bigint::{BigInt, Sign};

#[derive(Clone)]
struct Pair {
    d: IBig,
    n: BigInt,
}

impl Pair {
    fn from_i128(v: i128) -> Self {
        Pair {
            d: IBig::from(v),
            n: BigInt::from(v),
        }
    }

    fn from_signed_bytes(sign_byte: u8, bytes: &[u8]) -> Self {
        // Build a value of arbitrary magnitude from a byte string. We treat
        // bit 0 of `sign_byte` as the sign and the remaining bytes as a
        // big-endian magnitude. We use the same construction on both sides
        // so we don't have to trust either implementation's byte parser.
        let mut mag = BigInt::from(0);
        for &b in bytes {
            mag = (mag << 8) + BigInt::from(b);
        }
        let mut dmag = IBig::from(0);
        for &b in bytes {
            dmag = (dmag << 8) + IBig::from(b);
        }
        if sign_byte & 1 == 1 {
            mag = -mag;
            dmag = -dmag;
        }
        Pair { d: dmag, n: mag }
    }

    fn check(&self) {
        assert_eq!(
            self.d.to_string(),
            self.n.to_string(),
            "dashu IBig and num-bigint diverged: dashu={:?} num-bigint={:?}",
            self.d,
            self.n,
        );
    }
}

// Pick an edge-case constant from a small table. Indexed by a drawn integer
// so hegel can shrink toward specific edges deterministically.
const EDGES: &[i128] = &[
    0,
    1,
    -1,
    2,
    -2,
    i32::MIN as i128,
    i32::MAX as i128,
    i64::MIN as i128,
    i64::MAX as i128,
    u64::MAX as i128,
    i128::MIN,
    i128::MAX,
    1 << 62,
    -(1 << 62),
    (1i128 << 64),
    -(1i128 << 64),
    (1i128 << 100),
    -(1i128 << 100),
];

struct IBigMachine {
    pool: Variables<Pair>,
}

#[hegel::state_machine]
impl IBigMachine {
    // ---- seeders: introduce new pairs into the pool ----

    #[rule]
    fn seed_i64(&mut self, tc: TestCase) {
        let v: i64 = tc.draw(gs::integers());
        let p = Pair::from_i128(v as i128);
        p.check();
        self.pool.add(p);
    }

    #[rule]
    fn seed_i128(&mut self, tc: TestCase) {
        let v: i128 = tc.draw(gs::integers());
        let p = Pair::from_i128(v);
        p.check();
        self.pool.add(p);
    }

    #[rule]
    fn seed_edge(&mut self, tc: TestCase) {
        let idx: usize = tc.draw(gs::integers::<usize>().max_value(EDGES.len() - 1));
        let p = Pair::from_i128(EDGES[idx]);
        p.check();
        self.pool.add(p);
    }

    #[rule]
    fn seed_bytes(&mut self, tc: TestCase) {
        let sign: u8 = tc.draw(gs::integers());
        // Cap the length so test cases stay tractable. ~20 bytes is enough
        // to drive multi-limb code on 64-bit platforms.
        let bytes: Vec<u8> = tc.draw(gs::vecs(gs::integers::<u8>()).max_size(20));
        let p = Pair::from_signed_bytes(sign, &bytes);
        p.check();
        self.pool.add(p);
    }

    // ---- producing rules: combine two pool entries ----

    #[rule]
    fn add(&mut self, _tc: TestCase) {
        let a = self.pool.draw().clone();
        let b = self.pool.draw().clone();
        let r = Pair {
            d: &a.d + &b.d,
            n: &a.n + &b.n,
        };
        r.check();
        self.pool.add(r);
    }

    #[rule]
    fn sub(&mut self, _tc: TestCase) {
        let a = self.pool.draw().clone();
        let b = self.pool.draw().clone();
        let r = Pair {
            d: &a.d - &b.d,
            n: &a.n - &b.n,
        };
        r.check();
        self.pool.add(r);
    }

    #[rule]
    fn mul(&mut self, _tc: TestCase) {
        let a = self.pool.draw().clone();
        let b = self.pool.draw().clone();
        let r = Pair {
            d: &a.d * &b.d,
            n: &a.n * &b.n,
        };
        r.check();
        self.pool.add(r);
    }

    #[rule]
    fn div_rem(&mut self, tc: TestCase) {
        let a = self.pool.draw().clone();
        let b = self.pool.draw().clone();
        tc.assume(b.n != BigInt::from(0));
        // Both libraries use truncated division for `/` and `%`. Check both
        // results in one step.
        let q = Pair {
            d: &a.d / &b.d,
            n: &a.n / &b.n,
        };
        q.check();
        let r = Pair {
            d: &a.d % &b.d,
            n: &a.n % &b.n,
        };
        r.check();
        self.pool.add(q);
        self.pool.add(r);
    }

    #[rule]
    fn neg(&mut self, _tc: TestCase) {
        let a = self.pool.draw().clone();
        let r = Pair {
            d: -&a.d,
            n: -&a.n,
        };
        r.check();
        self.pool.add(r);
    }

    #[rule]
    fn abs(&mut self, _tc: TestCase) {
        let a = self.pool.draw().clone();
        let abs_num = if a.n.sign() == Sign::Minus { -&a.n } else { a.n.clone() };
        let r = Pair {
            d: (&a.d).abs(),
            n: abs_num,
        };
        r.check();
        self.pool.add(r);
    }

    #[rule]
    fn bitand(&mut self, _tc: TestCase) {
        let a = self.pool.draw().clone();
        let b = self.pool.draw().clone();
        let r = Pair {
            d: &a.d & &b.d,
            n: &a.n & &b.n,
        };
        r.check();
        self.pool.add(r);
    }

    #[rule]
    fn bitor(&mut self, _tc: TestCase) {
        let a = self.pool.draw().clone();
        let b = self.pool.draw().clone();
        let r = Pair {
            d: &a.d | &b.d,
            n: &a.n | &b.n,
        };
        r.check();
        self.pool.add(r);
    }

    #[rule]
    fn bitxor(&mut self, _tc: TestCase) {
        let a = self.pool.draw().clone();
        let b = self.pool.draw().clone();
        let r = Pair {
            d: &a.d ^ &b.d,
            n: &a.n ^ &b.n,
        };
        r.check();
        self.pool.add(r);
    }

    #[rule]
    fn shl_small(&mut self, tc: TestCase) {
        let a = self.pool.draw().clone();
        let s: u32 = tc.draw(gs::integers::<u32>().max_value(200));
        let r = Pair {
            d: &a.d << (s as usize),
            n: &a.n << (s as usize),
        };
        r.check();
        self.pool.add(r);
    }

    #[rule]
    fn shr_small(&mut self, tc: TestCase) {
        let a = self.pool.draw().clone();
        let s: u32 = tc.draw(gs::integers::<u32>().max_value(200));
        let r = Pair {
            d: &a.d >> (s as usize),
            n: &a.n >> (s as usize),
        };
        r.check();
        self.pool.add(r);
    }

    #[rule]
    fn pow_small(&mut self, tc: TestCase) {
        let a = self.pool.draw().clone();
        let e: u32 = tc.draw(gs::integers::<u32>().max_value(8));
        let r = Pair {
            d: (&a.d).pow(e as usize),
            n: (&a.n).pow(e),
        };
        r.check();
        self.pool.add(r);
    }

    #[rule]
    fn add_primitive_i64(&mut self, tc: TestCase) {
        let a = self.pool.draw().clone();
        let v: i64 = tc.draw(gs::integers());
        let r = Pair {
            d: &a.d + IBig::from(v),
            n: &a.n + BigInt::from(v),
        };
        r.check();
        self.pool.add(r);
    }

    #[rule]
    fn mul_primitive_i64(&mut self, tc: TestCase) {
        let a = self.pool.draw().clone();
        let v: i64 = tc.draw(gs::integers());
        let r = Pair {
            d: &a.d * IBig::from(v),
            n: &a.n * BigInt::from(v),
        };
        r.check();
        self.pool.add(r);
    }

    // ---- non-producing rules: check derived quantities agree ----

    #[rule]
    fn check_cmp(&mut self, _tc: TestCase) {
        let a = self.pool.draw();
        let b = self.pool.draw();
        assert_eq!(
            a.d.cmp(&b.d),
            a.n.cmp(&b.n),
            "Ord disagreed: a={} b={}",
            a.n,
            b.n,
        );
        assert_eq!(
            a.d == b.d,
            a.n == b.n,
            "Eq disagreed: a={} b={}",
            a.n,
            b.n,
        );
    }

    #[rule]
    fn check_try_into_i128(&mut self, _tc: TestCase) {
        let a = self.pool.draw();
        let dashu_r: Result<i128, _> = (&a.d).try_into();
        // num-bigint exposes `i128() -> Option<i128>` via the ToPrimitive
        // trait, but we want to test the conversion path without pulling
        // in num-traits. So we do the range check ourselves on `n`.
        let bound = BigInt::from(i128::MIN)..=BigInt::from(i128::MAX);
        let in_range = bound.contains(&a.n);
        match (dashu_r, in_range) {
            (Ok(v), true) => {
                assert_eq!(BigInt::from(v), a.n, "i128 round-trip mismatch");
            }
            (Err(_), false) => {}
            (got, expected_in_range) => panic!(
                "i128 conversion disagreement on {}: dashu got {:?}, in_range_per_num={}",
                a.n, got, expected_in_range
            ),
        }
    }

    #[rule]
    fn check_bytes_roundtrip(&mut self, _tc: TestCase) {
        let a = self.pool.draw();
        let bytes = a.d.to_be_bytes();
        let parsed = IBig::from_be_bytes(&bytes);
        assert_eq!(parsed, a.d, "to_be_bytes / from_be_bytes round-trip failed");
    }

    #[rule]
    fn check_bit_queries(&mut self, _tc: TestCase) {
        let a = self.pool.draw();
        // Compare bit length on the magnitude (num-bigint reports the magnitude's bits too).
        let dashu_bits = a.d.bit_len();
        let num_bits = a.n.bits() as usize;
        assert_eq!(
            dashu_bits, num_bits,
            "bit_len disagreed on {}: dashu={} num-bigint={}",
            a.n, dashu_bits, num_bits,
        );
    }

    // ---- invariant: cheap sanity check on a freshly drawn pair ----

    #[invariant]
    fn pool_consistency(&mut self, _tc: TestCase) {
        if !self.pool.is_empty() {
            self.pool.draw().check();
        }
    }
}

#[hegel::test]
fn test_ibig_matches_num_bigint(tc: TestCase) {
    let pool = variables(&tc);
    let mut m = IBigMachine { pool };
    // Seed at least one starting value so producing rules can draw immediately.
    m.pool.add(Pair::from_i128(0));
    hegel::stateful::run(m, tc);
}
