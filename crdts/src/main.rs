use base64::{CharacterSet, Config};
use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use sodiumoxide::crypto::hash;
use sodiumoxide::crypto::sign;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

mod replicant;
use replicant::{
    create_account, create_crdt, create_crdt_info, get_random_id, Account, Applyable, CRDTInfo,
    Counter, Nat, Operation, Signature, UserPubKey, UserSecKey, CRDT,
};

use ansi_term::Colour::Red;

fn base64Config() -> Config {
    Config::new(CharacterSet::UrlSafe, false)
}

fn main() {
    let _ = ansi_term::enable_ansi_support();
    let args: Vec<String> = env::args().collect();

    if args.len() >= 2 {
        let project_name: &str = &args[1];
        let project_basedir_str = format!("{}/", project_name);
        let project_file_str = format!("project.penny");
        let project_basedir = std::path::Path::new(&project_basedir_str);
        let pennyfile_dir = project_basedir.join(std::path::Path::new(&project_file_str));

        match File::open(&pennyfile_dir) {
            Ok(mut file) => {
                println!("Looking for a project at {:?}.", pennyfile_dir);
                let mut contents = vec![];
                file.read_to_end(&mut contents).unwrap();
                let project_info: CRDTInfo<Nat> = bincode::deserialize(&contents).unwrap();
                // @todo: read the id and use it to create the CRDT

                let DirectoryLevelUserInfo { pk, sk, .. } = get_keypair(&pennyfile_dir);
                let account = create_account(pk, sk);

                let crdt = create_crdt(project_info);

                println!("Testing the {} CRDT", Nat::NAME);
                run(crdt, account, project_basedir);
            }
            Err(_) => {
                print!(
                    "Couldn't open '{}'! Do you want me to create it? ",
                    project_name
                );
                io::stdout().flush().unwrap();
                let mut contents = String::new();
                io::stdin().read_line(&mut contents).unwrap();
                if contents.trim() == "y" {
                    let info: CRDTInfo<Nat> = create_crdt_info(Nat::from(0), get_random_id());
                    let info =
                        bincode::serialize(&info).expect("somehow there was a serialization error");
                    let _test: CRDTInfo<Nat> = bincode::deserialize(&info).unwrap();
                    fs::create_dir_all(project_basedir).unwrap();
                    {
                        let mut project_file = File::create(&pennyfile_dir).unwrap();
                        project_file.write_all(&info).unwrap();
                    }
                    println!("I created a new project at {:?}.", pennyfile_dir);
                }
            }
        }
    } else {
        println!("Input the name of the project");
    }
}

fn run(mut crdt: CRDT<Nat>, mut account: Account, project_basedir: &Path) {
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
    write::<Nat>(crdt.flush(), project_basedir);
}

fn write<T>(mut operations: HashMap<Counter, Operation<T::Description>>, project_basedir: &Path)
where
    T: Applyable + Serialize,
    T::Description: Serialize,
{
    for (counter, operation) in operations.drain() {
        let toWriteDir = {
            let relativeDir = format!(
                "operations/{}",
                base64::encode_config(operation.user_pub_key, base64Config())
            );
            project_basedir.join(std::path::Path::new(&relativeDir))
        };
        fs::create_dir_all(&toWriteDir).expect("Failed to create directory to store operations");
        let toWriteFilePath =
            toWriteDir.join(std::path::Path::new(&format!("{}.pennyop", counter)));
        let mut file = OpenOptions::new()
            .read(false)
            .write(true)
            .create(true)
            .open(toWriteFilePath)
            .unwrap();
        file.write_all(
            &bincode::serialize(&operation).expect("somehow there was a serialization error"),
        )
        .expect("Failed to write operation");
    }
}

// This testimony is proof two different private keys are controlled by the same person.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
struct Testimony {
    parent_pk: UserPubKey,
    signature: Signature,
}

// This contains the information needed to create new operations on the CRDT.
// It is NOT needed to read the operations. It should stay private.
// Opening the same project in two different directories will result in different UserInfos.
// But they will each have a testomony that proves they belong to the same person.
#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
struct DirectoryLevelUserInfo {
    pk: UserPubKey,
    sk: UserSecKey,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
struct ComputerLevelUserInfo {
    computer_pk: UserPubKey,
    computer_sk: UserSecKey,
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
struct SavedKeys {
    computer_level_user_info: ComputerLevelUserInfo,
    dir_level_keys: HashMap<String, DirectoryLevelUserInfo>,
}

fn get_keypair(pennyfile_dir: &PathBuf) -> DirectoryLevelUserInfo {
    let pennyfile_dir_hash_string = {
        let pennyfile_dir_canonicalized = fs::canonicalize(pennyfile_dir).unwrap();
        let pennyfile_dir_bytes = pennyfile_dir_canonicalized
            .to_str()
            .expect(
                "The path the penny file is on isn't valid unicode, that is a requirement for now.",
            )
            .as_bytes();
        let pennyfile_dir_hash = hash::hash(pennyfile_dir_bytes);
        base64::encode_config(pennyfile_dir_hash, base64Config())
    };

    let mut keys = get_all_saved_keypairs();
    let dir_keypair = keys
        .dir_level_keys
        .entry(pennyfile_dir_hash_string)
        .or_insert_with(|| {
            let (pk, sk) = sign::gen_keypair();
            DirectoryLevelUserInfo { pk, sk }
        });
    let dir_keypair = dir_keypair.clone(); // I feel like there should be a way not to have to clone here
    set_all_saved_keypairs(&keys);
    dir_keypair
}

fn get_all_saved_keypairs() -> SavedKeys {
    // @todo: generate different keypairs for different directories
    if let Some(proj_dirs) = ProjectDirs::from("com", "PennySoftware", "Replicant") {
        let config_dir = proj_dirs.config_dir();
        println!("Config directory is {:?}", &config_dir);

        fs::create_dir_all(config_dir).expect("Failed to create configuration directory");
        let keys_path = config_dir.join(std::path::Path::new("keys.json"));
        match File::open(&keys_path) {
            Ok(mut file) => {
                let mut contents = String::new();
                file.read_to_string(&mut contents).unwrap();
                let keys: SavedKeys = serde_json::from_str(&contents).unwrap();
                keys
            }
            Err(_) => {
                let (pk, sk) = sign::gen_keypair();
                let keys = SavedKeys {
                    computer_level_user_info: ComputerLevelUserInfo {
                        computer_pk: pk,
                        computer_sk: sk,
                    },
                    dir_level_keys: HashMap::new(),
                };

                let mut file = File::create(keys_path).unwrap();
                write!(file, "{}", serde_json::to_string(&keys).unwrap()).unwrap();
                keys
            }
        }
    } else {
        panic!("couldn't get the project directory!")
    }
}

fn set_all_saved_keypairs(keys: &SavedKeys) {
    // @todo: generate different keypairs for different directories
    if let Some(proj_dirs) = ProjectDirs::from("com", "PennySoftware", "Replicant") {
        let config_dir = proj_dirs.config_dir();
        println!("Config directory is {:?}", &config_dir);

        fs::create_dir_all(config_dir).expect("Failed to create configuration directory");
        let keys_path = config_dir.join(std::path::Path::new("keys.json"));

        let mut file = OpenOptions::new()
            .read(false)
            .write(true)
            .create(true)
            .open(keys_path)
            .unwrap();

        write!(file, "{}", serde_json::to_string(keys).unwrap()).unwrap();
    } else {
        panic!("couldn't get the project directory!")
    };
}
