use directories::{BaseDirs, ProjectDirs, UserDirs};
use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::sign;
use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;

mod replicant;
use replicant::{create_crdt, get_random_id, Applyable, Nat};

use ansi_term::Colour::Red;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() >= 2 {
        let _ = ansi_term::enable_ansi_support();

        let UserInfo { pk, sk } = get_keypair();

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
    } else {
        println!("Input the name of the project");
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
struct UserInfo {
    pk: sign::ed25519::PublicKey,
    sk: sign::ed25519::SecretKey,
}

fn get_keypair() -> UserInfo {
    if let Some(proj_dirs) = ProjectDirs::from("com", "PennySoftware", "Replicant") {
        let config_dir = proj_dirs.config_dir();
        fs::create_dir_all(config_dir).unwrap();
        let keys_path = config_dir.join(std::path::Path::new("keys.json"));
        match File::open(&keys_path) {
            Ok(mut file) => {
                let mut contents = String::new();
                file.read_to_string(&mut contents).unwrap();
                serde_json::from_str(&contents).unwrap()
            }
            Err(_) => {
                let (pk, sk) = sign::gen_keypair();
                let mut file = File::create(keys_path).unwrap();
                write!(
                    file,
                    "{}",
                    serde_json::to_string(&UserInfo { pk, sk }).unwrap()
                )
                .unwrap();
                get_keypair()
            }
        }
    } else {
        panic!("couldn't get the project directory!")
    }
}
