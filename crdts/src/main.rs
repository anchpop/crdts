use directories::{BaseDirs, ProjectDirs, UserDirs};
use sodiumoxide::crypto::sign;
use std::io;
use std::io::Write;

mod replicant;
use replicant::{create_crdt, get_random_id, Applyable, Nat};

use ansi_term::Colour::Red;

fn main() {
    let _ = ansi_term::enable_ansi_support();

    let (pk, sk): (sign::ed25519::PublicKey, sign::ed25519::SecretKey) = sign::gen_keypair();
    let mut crdt = create_crdt(Nat::from(0), pk, sk, get_random_id());

    println!("Testing the {} CRDT", Nat::NAME);
    loop {
        println!(
            "Current value: {}",
            Red.paint(format!("{}", crdt.value.value))
        );
        print!("Increment: ");
        let _ = io::stdout().flush();
        let mut increment = String::new();
        let _ = io::stdin().read_line(&mut increment);
        match increment.trim().parse() {
            Ok(increment) => {
                crdt = crdt.apply_desc(increment);
            }
            _ => break,
        }
    }
}
