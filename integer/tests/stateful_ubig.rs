// Stateful equivalence test: dashu_int::UBig vs num_bigint::BigUint.
//
// Mirror of stateful_ibig.rs for the unsigned path. Subtraction is guarded
// with `assume(a >= b)` so we never underflow either implementation.
//
// Gated behind the `hegel-tests` feature; run with:
//   cargo test -p dashu-int --features hegel-tests --test stateful_ubig

use dashu_int::ops::BitTest;
use dashu_int::UBig;
use hegel::generators as gs;
use hegel::stateful::{variables, Variables};
use hegel::TestCase;
use num_bigint::BigUint;

#[derive(Clone)]
struct Pair {
    d: UBig,
    n: BigUint,
}

impl Pair {
    fn from_u128(v: u128) -> Self {
        Pair {
            d: UBig::from(v),
            n: BigUint::from(v),
        }
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        let mut n = BigUint::from(0u32);
        let mut d = UBig::from(0u32);
        for &b in bytes {
            n = (n << 8) + BigUint::from(b);
            d = (d << 8) + UBig::from(b);
        }
        Pair { d, n }
    }

    fn check(&self) {
        assert_eq!(
            self.d.to_string(),
            self.n.to_string(),
            "dashu UBig and num-bigint diverged: dashu={:?} num-bigint={:?}",
            self.d,
            self.n,
        );
    }
}

const EDGES: &[u128] = &[
    0,
    1,
    2,
    u32::MAX as u128,
    u64::MAX as u128,
    1 << 63,
    1 << 64,
    1 << 100,
    u128::MAX,
];

struct UBigMachine {
    pool: Variables<Pair>,
}

#[hegel::state_machine]
impl UBigMachine {
    // ---- seeders ----

    #[rule]
    fn seed_u64(&mut self, tc: TestCase) {
        let v: u64 = tc.draw(gs::integers());
        let p = Pair::from_u128(v as u128);
        p.check();
        self.pool.add(p);
    }

    #[rule]
    fn seed_u128(&mut self, tc: TestCase) {
        let v: u128 = tc.draw(gs::integers());
        let p = Pair::from_u128(v);
        p.check();
        self.pool.add(p);
    }

    #[rule]
    fn seed_edge(&mut self, tc: TestCase) {
        let idx: usize = tc.draw(gs::integers::<usize>().max_value(EDGES.len() - 1));
        let p = Pair::from_u128(EDGES[idx]);
        p.check();
        self.pool.add(p);
    }

    #[rule]
    fn seed_bytes(&mut self, tc: TestCase) {
        let bytes: Vec<u8> = tc.draw(gs::vecs(gs::integers::<u8>()).max_size(20));
        let p = Pair::from_bytes(&bytes);
        p.check();
        self.pool.add(p);
    }

    // ---- producing rules ----

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
    fn sub(&mut self, tc: TestCase) {
        let a = self.pool.draw().clone();
        let b = self.pool.draw().clone();
        // Underflow on UBig panics, underflow on BigUint also panics, but we
        // avoid the panic path on both for now and only exercise valid sub.
        tc.assume(a.n >= b.n);
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
        tc.assume(b.n != BigUint::from(0u32));
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
    fn add_primitive_u64(&mut self, tc: TestCase) {
        let a = self.pool.draw().clone();
        let v: u64 = tc.draw(gs::integers());
        let r = Pair {
            d: &a.d + UBig::from(v),
            n: &a.n + BigUint::from(v),
        };
        r.check();
        self.pool.add(r);
    }

    #[rule]
    fn mul_primitive_u64(&mut self, tc: TestCase) {
        let a = self.pool.draw().clone();
        let v: u64 = tc.draw(gs::integers());
        let r = Pair {
            d: &a.d * UBig::from(v),
            n: &a.n * BigUint::from(v),
        };
        r.check();
        self.pool.add(r);
    }

    // ---- non-producing rules ----

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
    fn check_try_into_u128(&mut self, _tc: TestCase) {
        let a = self.pool.draw();
        let dashu_r: Result<u128, _> = (&a.d).try_into();
        let bound = BigUint::from(0u32)..=BigUint::from(u128::MAX);
        let in_range = bound.contains(&a.n);
        match (dashu_r, in_range) {
            (Ok(v), true) => {
                assert_eq!(BigUint::from(v), a.n, "u128 round-trip mismatch");
            }
            (Err(_), false) => {}
            (got, expected_in_range) => panic!(
                "u128 conversion disagreement on {}: dashu got {:?}, in_range_per_num={}",
                a.n, got, expected_in_range
            ),
        }
    }

    #[rule]
    fn check_bytes_roundtrip(&mut self, _tc: TestCase) {
        let a = self.pool.draw();
        let bytes = a.d.to_be_bytes();
        let parsed = UBig::from_be_bytes(&bytes);
        assert_eq!(parsed, a.d, "to_be_bytes / from_be_bytes round-trip failed");
    }

    #[rule]
    fn check_bit_queries(&mut self, _tc: TestCase) {
        let a = self.pool.draw();
        let dashu_bits = a.d.bit_len();
        let num_bits = a.n.bits() as usize;
        assert_eq!(
            dashu_bits, num_bits,
            "bit_len disagreed on {}: dashu={} num-bigint={}",
            a.n, dashu_bits, num_bits,
        );
    }

    // ---- invariant ----

    #[invariant]
    fn pool_consistency(&mut self, _tc: TestCase) {
        if !self.pool.is_empty() {
            self.pool.draw().check();
        }
    }
}

#[hegel::test]
fn test_ubig_matches_num_bigint(tc: TestCase) {
    let pool = variables(&tc);
    let mut m = UBigMachine { pool };
    m.pool.add(Pair::from_u128(0));
    hegel::stateful::run(m, tc);
}
