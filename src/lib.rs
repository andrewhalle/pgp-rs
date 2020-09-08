use num::bigint::{BigInt, BigUint, RandBigInt, ToBigInt, ToBigUint};
use num::integer::ExtendedGcd;
use num::integer::Integer;
use num::traits::identities::Zero;
use rand::thread_rng;
use rayon::iter::repeat;
use rayon::prelude::*;
use std::fs;
use std::thread;
use std::time::Duration;
use termprogress::prelude::*;

const TRIALS: u32 = 10;
const BIT_SIZE: u64 = 1024;

fn factor_as_multiplication(n: &BigUint) -> (BigUint, BigUint) {
    let mut d = n.clone();
    let mut s = BigUint::new(vec![0]);
    while d.is_even() {
        s += 1_u32;
        d /= 2_u32;
    }

    (d, s)
}

fn miller_rabin(n: &BigUint) -> bool {
    let mut rng = thread_rng();

    if n.is_even() {
        return false;
    }

    let n_minus_one = n.clone() - 1_u32;
    let (d, s) = factor_as_multiplication(&n_minus_one);
    let s_minus_one = s - 1_u32;

    'witness: for _i in 0..TRIALS {
        let a = rng.gen_biguint_below(&n);
        let mut x = a.modpow(&d, &n);

        let one = BigUint::new(vec![1]);
        if x == one || x == n_minus_one {
            continue 'witness;
        }

        let mut j = BigUint::new(vec![0]);
        while j < s_minus_one {
            let two = BigUint::new(vec![2]);
            x = x.modpow(&two, &n);
            if x == n_minus_one {
                continue 'witness;
            }

            j += 1_u32;
        }

        return false;
    }

    true
}

fn fermat(n: &BigUint) -> bool {
    let mut rng = thread_rng();

    let one = BigUint::new(vec![1]);
    let n_minus_1 = n.clone() - 1_u32;

    let a = rng.gen_biguint_below(&n_minus_1);

    a.modpow(&n_minus_1, &n) == one
}

fn first_twenty_primes(n: &BigUint) -> bool {
    let primes = &[
        2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53, 59, 61, 67, 71_u32,
    ];

    let zero = BigUint::new(vec![0]);
    for p in primes.iter() {
        if n % p == zero {
            return false;
        }
    }

    true
}

fn is_probable_prime(n: &BigUint) -> bool {
    first_twenty_primes(n) && fermat(n) && miller_rabin(n)
}

fn gen_prime_candidate() -> (bool, BigUint) {
    let mut rng = thread_rng();
    let num = rng.gen_biguint(BIT_SIZE);

    (is_probable_prime(&num), num)
}

pub fn gen_large_prime() -> BigUint {
    let (_, large_prime) = repeat(())
        .map(|_| gen_prime_candidate())
        .find_any(|(is_prime, _)| *is_prime)
        .unwrap();

    large_prime
}

fn carmichaels_totient_function(p: &BigUint, q: &BigUint) -> BigUint {
    let p_minus_1 = p.clone() - 1_u32;
    let q_minus_1 = q.clone() - 1_u32;

    let mag = p_minus_1.clone() * q_minus_1.clone();

    mag / p_minus_1.gcd(&q_minus_1)
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PublicKey {
    n: BigUint,
    e: BigUint,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct PrivateKey {
    n: BigUint,
    e: BigUint,
    d: BigUint,
}

pub fn gen_key(private_key_path: &str, public_key_path: &str) {
    let estimated = 1.0;

    let mut p_elapsed = 0.0;
    let mut progress = Bar::default();
    progress.set_title("Generating p...");
    let t = thread::spawn(move || {
        while p_elapsed < estimated {
            p_elapsed += 0.2;
            progress.set_progress(f64::min(1.0, p_elapsed / estimated));
            thread::sleep(Duration::from_millis(200));
        }
    });
    let p = gen_large_prime();
    t.join().unwrap();

    let mut q_elapsed = 0.0;
    let mut progress = Bar::default();
    progress.set_title("Generating q...");
    let t = thread::spawn(move || {
        while q_elapsed < estimated {
            q_elapsed += 0.2;
            progress.set_progress(f64::min(1.0, q_elapsed / estimated));
            thread::sleep(Duration::from_millis(200));
        }
    });
    let q = gen_large_prime();
    t.join().unwrap();

    let n = p.clone() * q.clone();
    let lambda_n = carmichaels_totient_function(&p, &q);
    let e = 65_537.to_bigint().unwrap();
    let ExtendedGcd { y: mut d, .. } = lambda_n.to_bigint().unwrap().extended_gcd(&e);
    if d < BigInt::zero() {
        d += lambda_n.to_bigint().unwrap();
    }

    let pub_key = serde_json::to_string(&PublicKey {
        n: n.to_biguint().unwrap(),
        e: e.to_biguint().unwrap(),
    })
    .unwrap();

    let priv_key = serde_json::to_string(&PrivateKey {
        n: n.to_biguint().unwrap(),
        e: e.to_biguint().unwrap(),
        d: d.to_biguint().unwrap(),
    })
    .unwrap();

    fs::write(public_key_path, pub_key).unwrap();
    println!("Wrote public key to {}", public_key_path);

    fs::write(private_key_path, priv_key).unwrap();
    println!("Wrote private key to {}", private_key_path);
}

#[derive(serde::Serialize, serde::Deserialize)]
enum Message {
    Ciphertext(Vec<u8>),
    Plaintext(Vec<u8>),
}

impl Message {
    fn encrypt(&mut self, key: &PublicKey) {
        if let Self::Plaintext(plaintext) = self {
            let padded = BigUint::from_bytes_be(&plaintext);
            let ciphertext = padded.modpow(&key.e, &key.n);

            *self = Self::Ciphertext(ciphertext.to_bytes_be());
        }
    }

    fn decrypt(&mut self, key: &PrivateKey) {
        if let Self::Ciphertext(ciphertext) = self {
            let ciphertext = BigUint::from_bytes_be(&ciphertext);
            let padded = ciphertext.modpow(&key.d, &key.n);

            *self = Self::Plaintext(padded.to_bytes_be());
        }
    }
}

pub fn encrypt(source: &str, target: &str, public_key_path: &str) {
    let pub_key: PublicKey =
        serde_json::from_str(&fs::read_to_string(public_key_path).unwrap()).unwrap();

    let msg = fs::read_to_string(source).unwrap();
    let mut msg = Message::Plaintext(msg.into_bytes());
    msg.encrypt(&pub_key);

    fs::write(target, serde_json::to_string(&msg).unwrap()).unwrap();
}

pub fn decrypt(source: &str, private_key_path: &str) {
    let priv_key: PrivateKey =
        serde_json::from_str(&fs::read_to_string(private_key_path).unwrap()).unwrap();

    let mut msg: Message = serde_json::from_str(&fs::read_to_string(source).unwrap()).unwrap();
    msg.decrypt(&priv_key);

    if let Message::Plaintext(msg) = msg {
        let msg = String::from_utf8(msg).unwrap();
        println!("{}", msg);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_probable_prime_test() {
        let num = BigUint::new(vec![7919]);
        assert!(is_probable_prime(&num));
    }
}
