use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::sign;
use std::env;
use std::fs;
use std::fs::File;
use std::io;
use std::io::Read;
use std::io::Write;

mod replicant;
use replicant::{
    create_account, create_crdt, get_random_id, Applyable, Id, Nat, UserPubKey, UserSecKey,
};

use ansi_term::Colour::Red;

fn main() {
    let _ = ansi_term::enable_ansi_support();
    let args: Vec<String> = env::args().collect();

    if args.len() >= 2 {
        let project_name: &str = &args[1];
        let project_basedir_str = format!("{}/", project_name);
        let project_file_str = format!("{}.penny", project_name);
        let project_basedir = std::path::Path::new(&project_basedir_str);
        let project_path = project_basedir.join(std::path::Path::new(&project_file_str));

        match File::open(&project_path) {
            Ok(mut file) => {
                let mut contents = String::new();
                file.read_to_string(&mut contents).unwrap();
                let _root: ProjectRoot = serde_json::from_str(&contents).unwrap();
                // @todo: read the id and use it to create the CRDT

                let UserInfo { pk, sk } = get_keypair();
                let mut account = create_account(pk, sk);

                let mut crdt = create_crdt(Nat::from(0), get_random_id());

                println!("Testing the {} CRDT", Nat::NAME);
                loop {
                    println!(
                        "Current value: {}",
                        Red.paint(format!("{}", crdt.value.value))
                    );
                    print!("Increment: ");
                    io::stdout().flush().unwrap();
                    let mut increment = String::new();
                    io::stdin().read_line(&mut increment).unwrap();
                    match increment.trim().parse() {
                        Ok(increment) => {
                            crdt = crdt.apply_desc(&mut account, increment);
                        }
                        _ => break,
                    }
                }
            }
            Err(_) => {
                print!(
                    "Couldn't open '{}'! Do you want to create it? ",
                    project_name
                );
                io::stdout().flush().unwrap();
                let mut contents = String::new();
                io::stdin().read_line(&mut contents).unwrap();
                if contents.trim() == "y" {
                    let UserInfo { pk, sk } = get_keypair();
                    let _account = create_account(pk, sk);
                    let initial = create_crdt(Nat::from(0), get_random_id());
                    let initial = bincode::serialize(&initial)
                        .expect("somehow there was a serialization error");
                    println!("{:?}", initial);
                }
            }
        }
    } else {
        println!("Input the name of the project");
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
struct UserInfo {
    pk: UserPubKey,
    sk: UserSecKey,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
struct ProjectRoot {
    id: Id,
}

fn get_keypair() -> UserInfo {
    // @todo: generate different keypairs for different directories
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
